//! Original conditional graph comparisons with Community; no upstream method credit.
use std::io::{self, BufRead};
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        let (id, encoded) = line.split_once('\t').ok_or("case,operations required")?;
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let s = b.source::<usize, (u8, i64)>();
        let count =
            b.collect_conditionally::<_, _, _, CountCollector>(s.port(), |r| r.1 > 0, |_| ())?;
        let sum = b.group_collect_conditionally::<_, _, _, _, IntCollector>(
            s.port(),
            |r| r.0,
            |r| r.1 > 0,
            |r| r.1,
        )?;
        b.score("selected-count", &count, |n| {
            HardSoftScore::new(*n as i64, 0)
        })?;
        b.score("global-presence", &count, |_| HardSoftScore::new(0, 1))?;
        b.score("keyed-sum", &sum, |(k, s)| {
            HardSoftScore::new(0, *k as i64 * 100 + s.sum)
        })?;
        let mut g = b.build();
        let mut state = 0;
        let emit = |state, score: HardSoftScore| {
            println!("{id}\t{state}\t{}\t{}", score.hard(), score.soft())
        };
        emit(state, g.score());
        state += 1;
        if encoded.is_empty() {
            continue;
        }
        for op in encoded.split(';') {
            let fields = op.split(':').collect::<Vec<_>>();
            if fields.len() != 3 || !matches!(fields[0], "c" | "t") {
                return Err("operation".into());
            }
            let index: usize = fields[1].parse()?;
            if index >= 32 {
                return Err("id".into());
            }
            let value = if fields[2] == "-" {
                None
            } else {
                let (k, v) = fields[2].split_once(',').ok_or("row")?;
                let k: u8 = k.parse()?;
                let v: i64 = v.parse()?;
                if k >= 4 || !(-100..=100).contains(&v) {
                    return Err("bounds".into());
                }
                Some((k, v))
            };
            let mut t = g.transaction();
            t.apply(&s, TupleChange { id: index, value })?;
            emit(state, t.score()?);
            state += 1;
            if fields[0] == "t" {
                drop(t);
                emit(state, g.score());
                state += 1;
            } else {
                t.commit()?;
            }
        }
        g.assert_consistent()?;
    }
    Ok(())
}
