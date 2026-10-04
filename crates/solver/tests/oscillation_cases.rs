// Adapted Apache-2.0 pinned Community StrategicOscillationByLevelFinalistPodiumTest.
// All candidate scores, singleton identities and step/reference updates preserved.
// Explicit immutable score references replace Java mocked move/phase scopes.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

fn step<S: AdditiveScore>(
    ranking: FinalistRanking,
    current: S,
    best: S,
    rows: &[S],
    winner: usize,
) -> S {
    let mut f = MoveForager::new(ForagerConfig {
        ranking,
        current_score: current,
        best_score: best,
        tie_breaking: TieBreaking::Random { seed: 0 },
        ..Default::default()
    });
    for (id, score) in rows.iter().enumerate() {
        f.add(score.clone(), true, id);
    }
    assert_eq!(f.finalist_count(), 1);
    assert_eq!(
        f.pick_with(|_| panic!("singleton has no draw")).unwrap(),
        Some(winner)
    );
    rows[winner].clone()
}
fn hs(h: i64, s: i64) -> HardSoftScore {
    HardSoftScore::new(h, s)
}
fn hms(h: i64, m: i64, s: i64) -> HardMediumSoftScore {
    HardMediumSoftScore::new(h, m, s)
}

#[test]
fn reference_last_step_score() {
    let mode = FinalistRanking::StrategicOscillationLastStep;
    let best = hs(-200, -5000);
    let last = step(
        mode,
        best,
        best,
        &[
            hs(-150, -2000),
            hs(-100, -7000),
            hs(-100, -7100),
            hs(-200, -1000),
        ],
        1,
    );
    let last = step(
        mode,
        last,
        best,
        &[
            hs(-100, -8000),
            hs(-100, -7000),
            hs(-150, -3000),
            hs(-120, -4000),
            hs(-150, -2000),
            hs(-200, -1000),
        ],
        3,
    );
    step(
        mode,
        last,
        best,
        &[
            hs(-120, -4000),
            hs(-120, -5000),
            hs(-150, -3000),
            hs(-150, -1000),
            hs(-150, -2000),
            hs(-160, -500),
        ],
        3,
    );
}
#[test]
fn reference_best_score() {
    let mode = FinalistRanking::StrategicOscillationBest;
    let best = hs(-200, -5000);
    let last = step(
        mode,
        best,
        best,
        &[
            hs(-150, -2000),
            hs(-100, -7000),
            hs(-100, -7100),
            hs(-200, -1000),
        ],
        1,
    );
    let best = last;
    let last = step(
        mode,
        last,
        best,
        &[
            hs(-100, -8000),
            hs(-100, -7000),
            hs(-150, -3000),
            hs(-120, -4000),
            hs(-150, -2000),
            hs(-200, -1000),
        ],
        3,
    );
    step(
        mode,
        last,
        best,
        &[
            hs(-110, -8000),
            hs(-150, -3000),
            hs(-150, -1000),
            hs(-110, -6000),
            hs(-150, -2000),
            hs(-160, -500),
        ],
        3,
    );
}
#[test]
fn reference_last_step_score_three_levels() {
    let mode = FinalistRanking::StrategicOscillationLastStep;
    let best = hms(-200, -5000, -10);
    let last = step(
        mode,
        best,
        best,
        &[
            hms(-150, -2000, -10),
            hms(-100, -7000, -20),
            hms(-100, -7100, -5),
            hms(-200, -1000, -10),
        ],
        1,
    );
    let last = step(
        mode,
        last,
        best,
        &[
            hms(-100, -8000, -10),
            hms(-100, -7000, -30),
            hms(-150, -3000, -10),
            hms(-120, -4000, -40),
            hms(-150, -2000, -10),
            hms(-200, -1000, -10),
        ],
        3,
    );
    step(
        mode,
        last,
        best,
        &[
            hms(-120, -4000, -50),
            hms(-120, -5000, -10),
            hms(-150, -3000, -10),
            hms(-150, -1000, -20),
            hms(-150, -2000, -10),
            hms(-160, -500, -10),
        ],
        3,
    );
}
#[test]
fn always_pick_improving_move() {
    let reference = hms(0, -2, -3);
    step(
        FinalistRanking::StrategicOscillationLastStep,
        reference,
        reference,
        &[hms(-1, -1, -3), hms(0, -2, -1)],
        1,
    );
}
