// Original bounded sampling and independent cyclic successor references.
use std::collections::BTreeSet;
use timetuck::*;
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn visit(n: usize, row: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if row.len() == n {
            out.push(row.clone());
            return;
        }
        for i in 1..n {
            if !row.contains(&i) {
                row.push(i);
                visit(n, row, out);
                row.pop();
            }
        }
    }
    let mut out = vec![];
    visit(n, &mut vec![0], &mut out);
    out
}
fn edges(row: &[usize]) -> BTreeSet<(usize, usize)> {
    (0..row.len())
        .map(|i| {
            let a = row[i];
            let b = row[(i + 1) % row.len()];
            (a.min(b), a.max(b))
        })
        .collect()
}
fn config(min: usize, max: usize, seed: u64, attempts: usize) -> ListKOptSampling {
    ListKOptSampling::new(ListKOptConfig::new(min, max).unwrap(), seed, attempts)
}
#[test]
fn samples_preserve_exact_edge_counts_and_reach_independent_small_tours() {
    let mut compared = 0;
    for n in 4..=7 {
        let p =
            ListProblem::new((0..n).collect(), vec![ListEntity::new((0..n).collect())]).unwrap();
        let a = p.initial_assignment();
        let original = edges(&a.lists()[0]);
        let reference = permutations(n);
        for k in 2..=n {
            let expected: BTreeSet<_> = reference
                .iter()
                .filter(|r| original.difference(&edges(r)).count() == k)
                .cloned()
                .collect();
            let mut observed = BTreeSet::new();
            for seed in 0..16 {
                for movement in sample_list_k_opt_moves(&p, &a, config(k, k, seed, 2000)).unwrap() {
                    let mut trial = a.clone();
                    {
                        let changed = p.apply(&mut trial, &movement).unwrap();
                        assert!(expected.contains(&changed.lists()[0]), "n={n}, k={k}");
                        observed.insert(changed.lists()[0].clone());
                        compared += 1;
                    }
                    assert_eq!(trial, a);
                }
            }
            if n <= 6 {
                assert_eq!(observed, expected, "n={n}, k={k}");
            } else {
                assert_eq!(observed.is_empty(), expected.is_empty());
            }
        }
    }
    assert!(compared > 10000);
}
#[test]
fn multi_entity_sampling_preserves_prefixes_ranges_missing_values_and_full_pins() {
    let p = ListProblem::new(
        (0..20).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3, 4, 5])
                .with_pinned_prefix(2)
                .with_value_range((0..6).collect()),
            ListEntity::new(vec![6, 7, 8, 9, 10]).with_value_range((6..11).collect()),
            ListEntity::new((11..19).collect()).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let cfg = config(2, usize::MAX, 23, 4000);
    let moves: Vec<_> = sample_list_k_opt_moves(&p, &a, cfg.clone())
        .unwrap()
        .collect();
    assert!(!moves.is_empty() && moves.len() <= 4000);
    assert_eq!(
        moves,
        list_moves(&p, &a, Neighborhood::SampledKOpt(cfg))
            .unwrap()
            .collect::<Vec<_>>()
    );
    let mut owners = BTreeSet::new();
    let mut ks = BTreeSet::new();
    for m in moves {
        let mut trial = a.clone();
        let changed = p.apply(&mut trial, &m).unwrap();
        assert_eq!(changed.location(&19), None);
        assert_eq!(changed.lists()[2], a.lists()[2]);
        let different: Vec<_> = (0..4)
            .filter(|&i| a.lists()[i] != changed.lists()[i])
            .collect();
        assert_eq!(different.len(), 1);
        let e = different[0];
        owners.insert(e);
        let prefix = p.entities()[e].pinned_prefix();
        assert_eq!(&changed.lists()[e][..=prefix], &a.lists()[e][..=prefix]);
        let before = &a.lists()[e][prefix..];
        let after = &changed.lists()[e][prefix..];
        let k = edges(before).difference(&edges(after)).count();
        ks.insert(k);
        assert!([2, 3, 5].contains(&k));
        assert_eq!(
            before.iter().collect::<BTreeSet<_>>(),
            after.iter().collect()
        );
    }
    assert_eq!(owners, BTreeSet::from([0, 1]));
    assert_eq!(ks, BTreeSet::from([2, 3, 5]));
}
#[test]
fn seeds_attempt_prefixes_step_derivation_and_repeats_are_explicit() {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new((0..6).collect())]).unwrap();
    let a = p.initial_assignment();
    let mut traces = BTreeSet::new();
    for seed in 0..16 {
        let run = |cfg| {
            sample_list_k_opt_moves(&p, &a, cfg)
                .unwrap()
                .map(|m| {
                    let mut trial = a.clone();
                    let changed = p.apply(&mut trial, &m).unwrap();
                    changed.lists()[0].clone()
                })
                .collect::<Vec<_>>()
        };
        let cfg = config(2, 6, seed, 300);
        let full = run(cfg.clone());
        assert_eq!(full, run(cfg.clone()));
        assert!(full.starts_with(&run(config(2, 6, seed, 7))));
        assert_eq!(cfg.for_step(0), cfg);
        assert_ne!(cfg.for_step(1).seed(), cfg.seed());
        assert_eq!(cfg.for_step(u64::MAX), cfg.for_step(u64::MAX));
        assert!(run(config(2, 6, seed, 0)).is_empty());
        assert!(full.len() <= 300);
        traces.insert(full);
    }
    assert_eq!(traces.len(), 16);
    let p = ListProblem::new((0..4).collect(), vec![ListEntity::new((0..4).collect())]).unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = sample_list_k_opt_moves(&p, &a, config(2, 2, 9, 1000))
        .unwrap()
        .collect();
    assert!(moves.len() > 4); // At most four tours; draws are with replacement.
}
#[test]
fn impossible_and_zero_work_terminate_and_malformed_inputs_still_fail() {
    for (n, min, max, pin) in [
        (0, 2, usize::MAX, false),
        (3, 2, 3, false),
        (4, 3, 4, false),
        (5, 4, 4, false),
        (8, 9, usize::MAX, false),
        (8, 2, 4, true),
    ] {
        let p = ListProblem::new(
            (0..n).collect(),
            vec![ListEntity::new((0..n).collect()).with_pinned(pin)],
        )
        .unwrap();
        let a = p.initial_assignment();
        assert!(
            sample_list_k_opt_moves(&p, &a, config(min, max, 0, usize::MAX))
                .unwrap()
                .next()
                .is_none()
        );
    }
    let p = ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap();
    assert!(sample_list_k_opt_moves(&p, &ListAssignment::new(vec![]), config(2, 3, 0, 0)).is_err());
}
#[test]
fn large_cut_sets_are_generated_on_demand_without_word_sized_limits() {
    let p = ListProblem::new(
        (0..130).collect(),
        vec![ListEntity::new((0..130).collect())],
    )
    .unwrap();
    let a = p.initial_assignment();
    let original = edges(&a.lists()[0]);
    let moves: Vec<_> = sample_list_k_opt_moves(&p, &a, config(65, 65, u64::MAX, usize::MAX))
        .unwrap()
        .take(2)
        .collect();
    assert_eq!(moves.len(), 2);
    for m in moves {
        let mut trial = a.clone();
        let changed = p.apply(&mut trial, &m).unwrap();
        assert_eq!(original.difference(&edges(&changed.lists()[0])).count(), 65);
    }
}
fn rank(row: &[usize]) -> SimpleScore {
    SimpleScore::new(row.iter().enumerate().map(|(i, &v)| (i * v) as i64).sum())
}
#[test]
fn per_step_sampling_matches_manual_first_improvement_across_both_list_backends() {
    let p = ListProblem::new(
        (0..6).collect(),
        vec![ListEntity::new(vec![0, 5, 4, 3, 2, 1])],
    )
    .unwrap();
    let mut net = ListConstraintNetwork::new(1, 6);
    net.add_constraint("rank", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        rank(row)
    })
    .unwrap();
    for seed in 0..32 {
        let sampling = config(2, 4, seed, 400);
        let mut manual = p.initial_assignment();
        let mut calls = 1;
        let mut steps = 0;
        for step in 0..3 {
            let old = manual.clone();
            let before = rank(&old.lists()[0]);
            let mut chosen = None;
            for m in sample_list_k_opt_moves(&p, &old, sampling.for_step(step)).unwrap() {
                calls += 1;
                let trial = p.apply(&mut manual, &m).unwrap();
                if rank(&trial.lists()[0]) > before {
                    chosen = Some(trial.clone());
                    break;
                }
            }
            if let Some(next) = chosen {
                manual = next;
                steps += 1;
            } else {
                break;
            }
        }
        let cfg = LocalSearchConfig {
            neighborhood: Neighborhood::SampledKOpt(sampling),
            max_evaluations: 10000,
            max_steps: 3,
            acceptance: Acceptance::StrictImprovement,
            selection: MoveSelection::FirstAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let a = list_local_search_from(&p, &net, p.initial_assignment(), cfg.clone()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("rank", s.entities.port(), |(_, row)| rank(row))
            .unwrap();
        let c =
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), cfg).unwrap();
        assert_eq!(a, c);
        assert_eq!(a.best.unwrap().assignment, manual);
        assert_eq!(a.evaluations, calls);
        assert_eq!(a.completed_steps, steps);
    }
}
