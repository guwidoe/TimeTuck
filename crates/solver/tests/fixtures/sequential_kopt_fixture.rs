// Adapted KOptListMoveIteratorTest.setupValidOddSequentialKOptMove TEST helper.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
pub struct Fixture {
    pub problem: ListProblem<String>,
    pub origin: String,
    pub values: Vec<String>,
    pub expected: Vec<String>,
}
pub fn fixture(owners: &[usize]) -> Fixture {
    let entities = owners.iter().copied().max().unwrap() + 1;
    let mut offset = 0;
    let rows: Vec<Vec<String>> = (0..entities)
        .map(|e| {
            let count = 2 * owners.iter().filter(|&&owner| owner == e).count();
            let mut row = vec![format!("e{}-start", e + 1)];
            for i in 0..count {
                row.push(format!("v{}", offset + i));
                row.push(format!("e{}-extra-{i}", e + 1));
            }
            row.push(format!("e{}-end", e + 1));
            offset += count;
            row
        })
        .collect();
    let origin = rows[owners[0]][1].clone();
    let mut positions = vec![1; entities];
    positions[owners[0]] += 3;
    let mut selected = vec![];
    let mut successors = vec![];
    for &e in &owners[1..] {
        let p = positions[e];
        selected.insert(0, rows[e][p].clone());
        successors.insert(0, rows[e][p + 1].clone());
        positions[e] += 3;
    }
    let mut expected = vec![origin.clone(), rows[owners[0]][2].clone()];
    for (a, b) in selected.iter().zip(successors) {
        expected.extend([a.clone(), b]);
    }
    let domain = rows.iter().flatten().cloned().collect();
    let problem =
        ListProblem::new(domain, rows.into_iter().map(ListEntity::new).collect()).unwrap();
    Fixture {
        problem,
        origin,
        values: selected,
        expected,
    }
}
/// Native recurrence adapting the TEST helper's computed sequential matching:
/// first/last endpoint connect, then each even endpoint to the following odd one.
/// Source's sentinel0/one-based array is translated in its entirety to native slots.
pub fn expected_partners(k: usize) -> Vec<usize> {
    let mut expected = vec![0; 2 * k];
    expected[0] = 2 * k - 1;
    expected[2 * k - 1] = 0;
    for i in (1..2 * k - 1).step_by(2) {
        expected[i] = i + 1;
        expected[i + 1] = i;
    }
    expected
}
