//! Independent Cartesian state-space, laziness and solver integration regressions.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
#[test]
fn products_match_independent_state_space_with_pins_ranges_nulls_and_subsets() {
    for code in 0..27 {
        for optional in [0, 3, 7] {
            for pinned in [0, 2, 7] {
                let mut state = code;
                let vars = (0..3)
                    .map(|i| {
                        let initial = match state % 3 {
                            0 => None,
                            1 => Some(0),
                            _ => Some(i + 1),
                        };
                        state /= 3;
                        let mut v = Variable::new(vec![0, i + 1]);
                        if let Some(value) = initial {
                            v = v.with_initial(value);
                        }
                        if optional & (1 << i) != 0 {
                            v = v.allowing_unassigned();
                        }
                        v.with_pinned(pinned & (1 << i) != 0)
                    })
                    .collect();
                let Ok(p) = FiniteProblem::new(vars) else {
                    continue;
                };
                let a = p.initial_assignment();
                let eligible: Vec<_> = (0..3).filter(|i| pinned & (1 << i) == 0).collect();
                for selection in [
                    SubPillarSelection::combinations(1, 3).unwrap(),
                    SubPillarSelection::combinations(2, 2).unwrap(),
                    SubPillarSelection::consecutive(2, 3).unwrap(),
                    SubPillarSelection::combinations(4, 4).unwrap(),
                ] {
                    let mut expected = BTreeSet::new();
                    for end in 0..27 {
                        let mut number = end;
                        let values: Vec<_> = (0..3)
                            .map(|i| {
                                let v = match number % 3 {
                                    0 => None,
                                    1 => Some(0),
                                    _ => Some(i + 1),
                                };
                                number /= 3;
                                v
                            })
                            .collect();
                        let changed: Vec<_> =
                            (0..3).filter(|&i| values[i] != a.values()[i]).collect();
                        if changed.len() < selection.minimum()
                            || changed.len() > selection.maximum()
                        {
                            continue;
                        }
                        if changed.iter().any(|&i| {
                            pinned & (1 << i) != 0
                                || (values[i].is_none() && optional & (1 << i) == 0)
                        }) {
                            continue;
                        }
                        if selection.is_consecutive()
                            && !eligible.windows(changed.len()).any(|w| w == changed)
                        {
                            continue;
                        }
                        expected.insert(values);
                    }
                    let mut actual = BTreeSet::new();
                    let mut count = 0;
                    let mut scratch = a.clone();
                    for m in change_product_moves(&p, &a, selection).unwrap() {
                        let trial = p.apply(&mut scratch, &m).unwrap();
                        assert_eq!(
                            m.touched_variables().len(),
                            trial
                                .values()
                                .iter()
                                .zip(a.values())
                                .filter(|(x, y)| x != y)
                                .count()
                        );
                        actual.insert(trial.values().to_vec());
                        count += 1;
                    }
                    assert_eq!(scratch, a);
                    assert_eq!(actual.len(), count, "duplicate successor");
                    assert_eq!(
                        actual, expected,
                        "code={code} optional={optional} pinned={pinned} selection={selection:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn generic_products_are_lazy_restart_suffixes_and_fuse_exhaustion() {
    let factories = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let polls = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let mut product = MoveProduct::new(3, false, |i| {
        factories[i].set(factories[i].get() + 1);
        let counter = &polls[i];
        (0..2).inspect(move |_| counter.set(counter.get() + 1))
    });
    assert_eq!(factories.each_ref().map(Cell::get), [0, 0, 0]);
    assert_eq!(product.next(), Some(vec![0, 0, 0]));
    assert_eq!(polls.each_ref().map(Cell::get), [1, 1, 1]);
    assert_eq!(product.next(), Some(vec![0, 0, 1]));
    assert_eq!(polls.each_ref().map(Cell::get), [1, 1, 2]);
    assert_eq!(product.count(), 6);
    assert_eq!(factories.each_ref().map(Cell::get), [1, 2, 4]);
    let mut product = MoveProduct::new(0, false, |_| 0..1);
    assert_eq!(product.next(), None);
    assert_eq!(product.next(), None);
    let mut product = MoveProduct::new(1, false, |_| {
        [Some(4), None, Some(8)].into_iter().scan((), |_, v| v)
    });
    assert_eq!(product.next(), Some(vec![4]));
    assert_eq!(product.next(), None);
    assert_eq!(product.next(), None);
    let mut infinite = MoveProduct::new(2, false, |_| 0..);
    assert_eq!(
        infinite.by_ref().take(3).collect::<Vec<_>>(),
        vec![vec![0, 0], vec![0, 1], vec![0, 2]]
    );
}
#[derive(Debug)]
struct Counted {
    value: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Counted {}
#[test]
fn huge_move_products_do_not_materialize_domains_or_combinations() {
    let clones = Rc::new(Cell::new(0));
    let value = |n| Counted {
        value: n,
        clones: clones.clone(),
    };
    let p = FiniteProblem::new(
        (0..40)
            .map(|_| Variable::new((0..50).map(value).collect()).with_initial(value(0)))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let mut moves =
        change_product_moves(&p, &a, SubPillarSelection::combinations(25, 25).unwrap()).unwrap();
    assert_eq!(clones.get(), 0);
    for _ in 0..3 {
        assert_eq!(moves.next().unwrap().touched_variables().len(), 25);
    }
    assert!(clones.get() < 150, "{} clones", clones.get());
}
#[test]
fn immutable_slots_are_excluded_and_incomplete_required_slots_can_be_filled() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]),
        Variable::new(vec![2]).with_initial(2).allowing_unassigned(),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> =
        change_product_moves(&p, &a, SubPillarSelection::combinations(2, 2).unwrap())
            .unwrap()
            .collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].touched_variables(), vec![1, 2]);
    let mut scratch = a.clone();
    p.apply(&mut scratch, &moves[0]).unwrap().commit();
    assert_eq!(scratch.values(), &[Some(0), Some(1), None]);
    assert!(p.is_complete(&scratch));
}
