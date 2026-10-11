// Adapted from Timefold Community DefaultProblemChangeDirectorTest and Lavish
// test fixtures, Apache-2.0. See the separate provenance ledger for exact paths.
// Original native failure/identity tests below are not upstream adaptations.
use ProblemObjectKind::{Entity, Fact};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    CalculatorSession, IncrementalCalculator, MatchRegistry, MatchRegistryError, ProblemEdit,
    ProblemEditError, ProblemLookup, ProblemObjectKind, SimpleScore,
};

type Edit = ProblemEdit<String, &'static str>;
type Session = CalculatorSession<Problem, Calculator>;
type Lookup = fn(&Problem, ProblemObjectKind, &String) -> ProblemLookup<usize>;
const ADDED: &str = "newly added entity";
const REMOVED: &str = "entity to remove";
const CHANGED: &str = "changed entity";
const ADDED_FACT: &str = "newly added fact";
const REMOVED_FACT: &str = "fact to remove";
const CHANGED_VALUE: &str = "changed entity value";

#[derive(Clone, Debug)]
struct Item {
    key: String,
    group: Option<String>,
    value: Option<String>,
    long: i64,
}
impl Item {
    fn entity(key: &str) -> Self {
        Self {
            key: key.into(),
            group: Some("entityGroupOne".into()),
            value: None,
            long: 1,
        }
    }
    fn fact(key: &str) -> Self {
        Self {
            key: key.into(),
            group: Some("valueGroupOne".into()),
            value: None,
            long: 1,
        }
    }
}
// Intentionally not Clone. Pools own the named objects supplied by the upstream
// mocked lookup even when they are absent from solution membership collections.
struct Problem {
    entities: Vec<Item>,
    facts: Vec<Item>,
    entity_members: Vec<String>,
    fact_members: Vec<String>,
    derived: i64,
}
impl Problem {
    fn generated(include_named: bool) -> Self {
        let facts: Vec<_> = (0..5)
            .map(|i| Item {
                key: format!("Generated Value {i}"),
                group: Some(format!("Generated ValueGroup {}", i % 2)),
                value: None,
                long: 1,
            })
            .collect();
        let entities: Vec<_> = (0..7)
            .map(|i| Item {
                key: format!("Generated Entity {i}"),
                group: Some(format!("Generated EntityGroup {}", i % 3)),
                value: Some(facts[i % 5].key.clone()),
                long: i as i64 * 100_000,
            })
            .collect();
        let mut p = Self {
            entity_members: entities.iter().map(|i| i.key.clone()).collect(),
            fact_members: facts.iter().map(|i| i.key.clone()).collect(),
            entities,
            facts,
            derived: -1,
        };
        p.entities
            .extend([Item::entity(REMOVED), Item::entity(CHANGED)]);
        p.facts
            .extend([Item::fact(REMOVED_FACT), Item::fact(CHANGED_VALUE)]);
        if include_named {
            p.entity_members.extend([REMOVED.into(), CHANGED.into()]);
            p.fact_members
                .extend([REMOVED_FACT.into(), CHANGED_VALUE.into()]);
        }
        p
    }
    fn total(&self) -> i64 {
        self.entities
            .iter()
            .filter(|e| self.entity_members.contains(&e.key))
            .map(|e| i64::from(e.value.is_some()) + i64::from(e.group.is_some()))
            .sum::<i64>()
            + self.fact_members.len() as i64
    }
}
fn lookup(p: &Problem, kind: ProblemObjectKind, key: &String) -> ProblemLookup<usize> {
    let pool = if kind == Entity {
        &p.entities
    } else {
        &p.facts
    };
    let mut positions = pool
        .iter()
        .enumerate()
        .filter(|(_, item)| &item.key == key)
        .map(|(i, _)| i);
    match (positions.next(), positions.next()) {
        (None, _) => ProblemLookup::Missing,
        (Some(i), None) => ProblemLookup::Unique(i),
        _ => ProblemLookup::Duplicate,
    }
}
#[derive(Default)]
struct Calculator {
    notifications: Vec<(bool, Edit)>,
    resets: usize,
    flushes: usize,
    scores: usize,
    cached: i64,
    old: i64,
    fail: Rc<Cell<u8>>,
}
impl Calculator {
    fn stage(&self, stage: u8) -> Result<(), MatchRegistryError> {
        if self.fail.get() == stage {
            return Err(MatchRegistryError::Calculator(format!("stage {stage}")));
        }
        assert_ne!(self.fail.get(), stage + 10, "stage panic {stage}");
        Ok(())
    }
}
impl IncrementalCalculator<Problem> for Calculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Edit;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        p: &Problem,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.stage(1)?;
        self.resets += 1;
        self.cached = p.total();
        Ok(())
    }
    fn before_change(
        &mut self,
        p: &Problem,
        edit: &Edit,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.stage(2)?;
        self.old = p.total();
        self.notifications.push((true, edit.clone()));
        Ok(())
    }
    fn after_change(
        &mut self,
        p: &Problem,
        edit: &Edit,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.stage(3)?;
        self.cached += p.total() - self.old;
        self.notifications.push((false, edit.clone()));
        Ok(())
    }
    fn flush_changes(
        &mut self,
        p: &mut Problem,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.stage(4)?;
        self.flushes += 1;
        p.derived = self.cached;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.scores += 1;
        SimpleScore::new(self.cached)
    }
}
fn session(include_named: bool) -> Session {
    CalculatorSession::new(
        Problem::generated(include_named),
        Calculator::default(),
        true,
    )
    .unwrap()
}
type Editor<'a> = timetuck::ProblemEdits<
    'a,
    Problem,
    Calculator,
    String,
    &'static str,
    usize,
    fn(&Problem, ProblemObjectKind, &String) -> ProblemLookup<usize>,
>;
fn entity_edits(e: &mut Editor<'_>) -> Result<(), ProblemEditError> {
    e.add_entity(ADDED.into(), |p| {
        p.entities.push(Item::entity(ADDED));
        p.entity_members.push(ADDED.into());
    })?;
    e.remove_entity(REMOVED.into(), |p, i| {
        p.entity_members.retain(|k| k != &p.entities[i].key);
    })
}
fn other_edits(e: &mut Editor<'_>) -> Result<(), ProblemEditError> {
    e.change_variable(CHANGED.into(), "value", |p, i| {
        p.entities[i].value = Some(CHANGED_VALUE.into())
    })?;
    e.change_property(Entity, CHANGED.into(), |p, i| p.entities[i].group = None)?;
    e.add_fact(ADDED_FACT.into(), |p| {
        p.facts.push(Item::fact(ADDED_FACT));
        p.fact_members.push(ADDED_FACT.into());
    })?;
    e.remove_fact(REMOVED_FACT.into(), |p, i| {
        p.fact_members.retain(|k| k != &p.facts[i].key)
    })
}
fn assert_score(s: &mut Session) {
    assert_eq!(s.score().unwrap(), SimpleScore::new(s.solution().total()));
}

#[test]
fn complex_problem_change_correctly_notifies_score_director() {
    let mut s = session(true);
    s.edit_problem(lookup as Lookup, |e| {
        entity_edits(e)?;
        other_edits(e)
    })
    .unwrap();
    let expected = [
        Edit::EntityAdded(ADDED.into()),
        Edit::EntityRemoved(REMOVED.into()),
        Edit::VariableChanged {
            entity: CHANGED.into(),
            variable: "value",
        },
        Edit::PropertyChanged {
            kind: Entity,
            key: CHANGED.into(),
        },
        Edit::FactAdded(ADDED_FACT.into()),
        Edit::FactRemoved(REMOVED_FACT.into()),
    ];
    for edit in &expected {
        for before in [true, false] {
            assert_eq!(
                s.calculator()
                    .notifications
                    .iter()
                    .filter(|n| n == &&(before, edit.clone()))
                    .count(),
                1
            );
        }
    }
    assert_eq!(s.calculator().notifications.len(), 12);
    assert_eq!(s.calculator().scores, 0);
    assert_score(&mut s);
}
#[test]
fn verify_no_reset_solution_if_no_entities_added_or_removed() {
    let mut s = session(false);
    s.edit_problem(lookup as Lookup, |e| {
        other_edits(e)?;
        e.flush()
    })
    .unwrap();
    assert_eq!(s.calculator().resets - 1, 0); // Exclude session initialization.
    assert_eq!(s.calculator().flushes, 1);
    assert_eq!(s.solution().derived, s.solution().total());
    assert_score(&mut s);
}
#[test]
fn verify_reset_solution_if_entities_added_or_removed() {
    let mut s = session(false);
    s.edit_problem(lookup, |e| {
        e.add_entity(ADDED.into(), |p| {
            p.entities.push(Item::entity(ADDED));
            p.entity_members.push(ADDED.into());
        })?;
        e.flush()
    })
    .unwrap();
    assert_eq!(s.calculator().resets - 1, 1);
    assert_eq!(s.calculator().flushes, 0);
    // The Java test installs a fresh generated fixture before removal. Owning
    // sessions install it explicitly; that fixture reset is excluded below.
    s.reset(Problem::generated(false)).unwrap();
    s.edit_problem(lookup, |e| {
        e.remove_entity(REMOVED.into(), |p, i| {
            p.entity_members.retain(|k| k != &p.entities[i].key)
        })?;
        e.flush()
    })
    .unwrap();
    assert_eq!(s.calculator().resets - 2, 2);
    assert_eq!(s.calculator().flushes, 0);
    assert_score(&mut s);
}
#[test]
fn verify_reset_solution_once_if_entities_added_or_removed_then_variables_changed() {
    let mut s = session(true);
    s.edit_problem(lookup as Lookup, |e| {
        entity_edits(e)?;
        e.flush()?;
        other_edits(e)?;
        e.flush()
    })
    .unwrap();
    assert_eq!(s.calculator().resets - 1, 1);
    assert_eq!(s.calculator().flushes, 1);
    assert_eq!(s.solution().derived, s.solution().total());
    assert_score(&mut s);
}

// The remaining tests are original native regression coverage.
#[test]
fn stable_lookup_rejects_missing_duplicate_and_existing_add_before_callbacks() {
    for mode in 0..4 {
        let mut s = session(true);
        let result = s.edit_problem(
            |p: &Problem, k, id: &String| {
                if mode == 1 {
                    ProblemLookup::Duplicate
                } else {
                    lookup(p, k, id)
                }
            },
            |e| {
                let result = if mode == 2 {
                    e.add_entity(CHANGED.into(), |_| panic!("mutation"))
                } else {
                    e.remove_entity(if mode == 0 { "absent" } else { CHANGED }.into(), |_, _| {
                        if mode != 3 {
                            panic!("mutation");
                        }
                    })
                };
                if mode == 3 {
                    return Err(MatchRegistryError::Calculator("outer".into()).into());
                }
                assert!(matches!(
                    result,
                    Err(ProblemEditError::Missing(_)) | Err(ProblemEditError::Duplicate(_))
                ));
                assert_eq!(e.flush(), Err(MatchRegistryError::NeedsReset.into()));
                Ok(()) // A caught operation error cannot hide session invalidation.
            },
        );
        assert!(result.is_err());
        if mode != 3 {
            assert!(s.calculator().notifications.is_empty());
        }
        assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
        s.reset_current().unwrap();
        assert_score(&mut s);
    }
}
#[test]
fn lookup_resolves_shifted_handles_and_mutates_owned_payload_only() {
    let mut s = session(true);
    let detached = s.solution().entities[1].clone();
    s.edit_problem(lookup, |e| {
        e.remove_entity("Generated Entity 0".into(), |p, i| {
            let item = p.entities.remove(i);
            p.entity_members.retain(|k| k != &item.key);
        })?;
        e.change_variable(detached.key.clone(), "value", |p, i| {
            assert_eq!(i, 0);
            p.entities[i].value = None;
        })
    })
    .unwrap();
    assert!(detached.value.is_some());
    assert!(s.solution().entities[0].value.is_none());
    assert_eq!(s.calculator().resets, 2);
    assert_eq!(s.calculator().scores, 0);
    assert_score(&mut s);
}
#[test]
fn callback_errors_and_panics_require_explicit_same_instance_reset() {
    for stage in 1..=4 {
        for panic in [false, true] {
            let mut s = session(true);
            let identity = s.calculator().fail.clone();
            identity.set(stage + if panic { 10 } else { 0 });
            let result = catch_unwind(AssertUnwindSafe(|| {
                s.edit_problem(lookup, |e| {
                    if stage == 1 {
                        e.remove_entity(REMOVED.into(), |p, i| {
                            p.entities.remove(i);
                        })
                    } else {
                        e.change_variable(CHANGED.into(), "value", |p, i| {
                            p.entities[i].value = None
                        })
                    }
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
            assert_eq!(s.calculator().scores, 0);
            identity.set(0);
            s.reset_current().unwrap();
            assert!(Rc::ptr_eq(&identity, &s.calculator().fail));
            assert_score(&mut s);
        }
    }
}
#[test]
fn caught_lookup_and_mutation_panics_still_poison_batch() {
    for lookup_panics in [false, true] {
        let mut s = session(true);
        let result = s.edit_problem(
            |p: &Problem, k, id: &String| {
                assert!(!lookup_panics, "lookup panic");
                lookup(p, k, id)
            },
            |e| {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| e.change_variable(
                        CHANGED.into(),
                        "value",
                        |p, i| {
                            p.entities[i].value = None;
                            panic!("mutation panic");
                        }
                    )))
                    .is_err()
                );
                Ok(())
            },
        );
        assert_eq!(result, Err(MatchRegistryError::NeedsReset.into()));
        assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
        s.reset_current().unwrap();
        assert_score(&mut s);
    }
}
#[test]
fn flush_is_once_no_scoring_and_preserves_matches_until_membership_reset() {
    let mut s = session(true);
    let retained = s
        .registry_mut()
        .unwrap()
        .register("external", SimpleScore::new(2), ())
        .unwrap();
    s.edit_problem(lookup, |e| {
        e.flush()?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        (
            s.calculator().resets,
            s.calculator().flushes,
            s.calculator().scores
        ),
        (1, 0, 0)
    );
    s.edit_problem(lookup, |e| {
        e.change_property(Fact, CHANGED_VALUE.into(), |p, i| p.facts[i].group = None)?;
        e.change_variable(CHANGED.into(), "value", |p, i| p.entities[i].value = None)?;
        e.flush()?;
        e.flush()
    })
    .unwrap();
    assert_eq!(
        (
            s.calculator().resets,
            s.calculator().flushes,
            s.calculator().scores
        ),
        (1, 1, 0)
    );
    retained.cancel(s.registry_mut().unwrap()).unwrap();
    let expired = s
        .registry_mut()
        .unwrap()
        .register("old", SimpleScore::new(1), ())
        .unwrap();
    s.edit_problem(lookup as Lookup, entity_edits).unwrap();
    assert_eq!(
        expired.cancel(s.registry_mut().unwrap()),
        Err(MatchRegistryError::ForeignOrExpiredRegistration)
    );
    assert_eq!(
        (
            s.calculator().resets,
            s.calculator().flushes,
            s.calculator().scores
        ),
        (2, 1, 0)
    );
    assert_score(&mut s);
}
#[test]
fn generated_fixture_keeps_source_groups_and_initial_values() {
    let p = Problem::generated(false);
    assert_eq!((p.entity_members.len(), p.fact_members.len()), (7, 5));
    for (i, item) in p.entities[..7].iter().enumerate() {
        assert_eq!(item.group, Some(format!("Generated EntityGroup {}", i % 3)));
        assert_eq!(item.value, Some(format!("Generated Value {}", i % 5)));
        assert!((0..=1_000_000).contains(&item.long));
    }
    for (i, item) in p.facts[..5].iter().enumerate() {
        assert_eq!(item.group, Some(format!("Generated ValueGroup {}", i % 2)));
    }
}
