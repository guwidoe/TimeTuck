// Adapted from pinned Community KOptListMoveIteratorTest TEST helpers, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md. No production source.
use timetuck::*;

pub const JAVA_PARTNERS: [usize; 17] = [0, 8, 3, 2, 5, 4, 7, 6, 1, 12, 11, 10, 9, 15, 16, 13, 14];

pub struct NonsequentialFixture {
    pub problem: ListProblem<String>,
    pub origin: String,
    pub values: Vec<String>,
    pub repair_cut: ListPosition,
    pub expected_removed: Vec<String>,
}

pub fn nonsequential_fixture(owners: &[usize; 5]) -> NonsequentialFixture {
    let entities = owners.iter().copied().max().unwrap() + 1;
    let mut offset = 0;
    let rows: Vec<Vec<String>> = (0..entities)
        .map(|entity| {
            let count = 2 * owners.iter().filter(|&&owner| owner == entity).count();
            let mut row = vec![format!("e{}-start", entity + 1)];
            for i in 0..count {
                row.push(format!("v{}", offset + i));
                row.push(format!("e{}-extra-{i}", entity + 1));
            }
            row.push(format!("e{}-end", entity + 1));
            offset += count;
            row
        })
        .collect();
    let origin = rows[owners[0]][1].clone();
    let mut offsets = vec![1; entities];
    offsets[owners[0]] += 3;
    let mut values = vec![];
    let mut expected_tail = vec![];
    for (i, &owner) in owners[1..].iter().enumerate() {
        let index = offsets[owner];
        values.insert(0, rows[owner][index].clone());
        let edge = if i == 0 {
            ["e1-extra-0".to_owned(), "v1".to_owned()]
        } else {
            [rows[owner][index].clone(), rows[owner][index + 1].clone()]
        };
        expected_tail.insert(0, edge);
        offsets[owner] += 3;
    }
    let final_owner = owners[4];
    let repair_cut = ListPosition {
        entity: final_owner,
        index: offsets[final_owner],
    };
    let mut expected_removed = vec![origin.clone(), rows[owners[0]][2].clone()];
    expected_removed.extend(expected_tail.into_iter().flatten());
    expected_removed.extend([
        rows[repair_cut.entity][repair_cut.index].clone(),
        rows[repair_cut.entity][repair_cut.index + 1].clone(),
    ]);
    if entities > 1 {
        // Exact explicit multi-owner expectation from the original TEST helper.
        expected_removed = [
            "v0",
            "e1-extra-0",
            "v3",
            "e1-extra-3",
            "e2-extra-1",
            "v8",
            "e1-extra-1",
            "v2",
            "v8",
            "e2-extra-2",
            "e1-extra-4",
            "v5",
        ]
        .map(str::to_owned)
        .to_vec();
    }
    let domain = rows.iter().flatten().cloned().collect();
    let problem =
        ListProblem::new(domain, rows.into_iter().map(ListEntity::new).collect()).unwrap();
    NonsequentialFixture {
        problem,
        origin,
        values,
        repair_cut,
        expected_removed,
    }
}

/// Translate the one-based Java descriptor to the entire active occurrence domain.
/// Keep the raw source array above, including its four out-of-domain entries.
/// A live port referencing spare storage is invalid, never silently truncated.
pub fn native_partners(raw: &[usize], k: usize) -> Result<Vec<usize>, &'static str> {
    let active = k.checked_mul(2).ok_or("port count overflow")?;
    if raw.first() != Some(&0) || raw.len() <= active {
        return Err("missing sentinel or active ports");
    }
    for i in 1..=active {
        let partner = raw[i];
        if partner == 0 || partner > active || partner == i || raw[partner] != i {
            return Err("invalid active matching");
        }
    }
    Ok(raw[1..=active].iter().map(|index| index - 1).collect())
}
