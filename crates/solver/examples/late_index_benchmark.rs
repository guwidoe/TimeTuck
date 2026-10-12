//! Original paired native probe. Previous implementation is our own pre-change
//! LateAcceptance, not Timefold production. No upstream-method credit.
use std::{hint::black_box, time::Instant};
use timetuck::*;
struct PreviousLate<S> {
    history: ScoreHistory<S>,
    best: S,
    hill_climbing: bool,
}
impl<S: Score> PreviousLate<S> {
    fn previous(initial: S, size: usize) -> Self {
        let history = ScoreHistory::new(size, initial).unwrap();
        Self {
            best: history.current().clone(),
            history,
            hill_climbing: true,
        }
    }
    fn previous_accepts(&self, current: S, candidate: S) -> bool {
        candidate >= *self.history.current() || (self.hill_climbing && candidate >= current)
    }
    fn previous_end(&mut self, score: S) {
        let improves = score > self.best;
        let reset = improves && score.late_acceptance_cmp(&self.best).is_gt();
        let best = improves.then(|| score.clone());
        let baseline = reset.then(|| score.clone());
        if let Some(baseline) = baseline {
            self.history.reset(baseline);
        }
        self.history.update(score);
        if let Some(best) = best {
            self.best = best;
        }
    }
}
fn previous_run<S: Score>(initial: &S, scores: &[S], size: usize) -> (u64, S) {
    let mut a = PreviousLate::previous(initial.clone(), size);
    let mut current = initial.clone();
    let mut count = 0;
    for score in scores {
        count +=
            u64::from(a.previous_accepts(black_box(current.clone()), black_box(score.clone())));
        a.previous_end(black_box(score.clone()));
        current = score.clone();
    }
    (count, a.history.current().clone())
}
fn indexed_run<S: Score>(initial: &S, scores: &[S], size: usize) -> (u64, S) {
    let mut a = LateAcceptance::new(size, true, initial.clone()).unwrap();
    let mut current = initial.clone();
    let mut count = 0;
    for score in scores {
        count += u64::from(a.accepts(black_box(current.clone()), black_box(score.clone())));
        a.step_ended(black_box(score.clone()));
        current = score.clone();
    }
    (count, a.threshold())
}
fn samples<S: Score>(label: &str, initial: S, scores: Vec<S>, size: usize) -> serde_json::Value {
    assert_eq!(
        previous_run(&initial, &scores, size),
        indexed_run(&initial, &scores, size)
    );
    let timed = |indexed: bool| {
        let start = Instant::now();
        for _ in 0..32 {
            let result = if indexed {
                indexed_run(&initial, &scores, size)
            } else {
                previous_run(&initial, &scores, size)
            };
            black_box(result);
        }
        start.elapsed().as_secs_f64() * 1000.0
    };
    for i in 0..3 {
        black_box(timed(i % 2 == 0));
        black_box(timed(i % 2 != 0));
    }
    let mut before = vec![];
    let mut after = vec![];
    for i in 0..15 {
        if i % 2 == 0 {
            before.push(timed(false));
            after.push(timed(true));
        } else {
            after.push(timed(true));
            before.push(timed(false));
        }
    }
    let median = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    serde_json::json!({"name":label,"history_size":size,"steps":scores.len(),"invocations_per_sample":32,
        "before_samples_ms":before,"after_samples_ms":after,"before_over_after":median(&before)/median(&after)})
}
fn main() {
    let raw: Vec<_> = (0..8192)
        .map(|i| {
            (
                (i / 2048) as i64 - 2,
                ((i * 17) % 211) as i64 - 100,
                ((i * 31) % 317) as i64 - 200,
            )
        })
        .collect();
    let reports = vec![
        samples(
            "hard-soft",
            HardSoftScore::new(-3, -1000),
            raw.iter()
                .map(|&(h, _, s)| HardSoftScore::new(h, s))
                .collect(),
            64,
        ),
        samples(
            "hard-medium-soft",
            HardMediumSoftScore::new(-3, -1000, -1000),
            raw.iter()
                .map(|&(h, m, s)| HardMediumSoftScore::new(h, m, s))
                .collect(),
            64,
        ),
        samples(
            "bendable",
            BendableScore::new([-3, -1000], [-1000]),
            raw.iter()
                .map(|&(h, m, s)| BendableScore::new([h, m], [s]))
                .collect(),
            4096,
        ),
    ];
    println!(
        "{}",
        serde_json::json!({"java_comparison":false,"warmups":3,"samples":15,"reports":reports})
    );
}
