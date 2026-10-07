// Adapted from Timefold Community IncrementalScoreDirectorTest, Apache-2.0.
// Source identities, fixtures and native substitutions: compat/provenance.json.
use timetuck::*;
type Registration = MatchRegistration<SimpleScore, DefaultJustification<SimpleScore, ()>>;
struct Calculator {
    aware: bool,
    entries: Vec<(&'static str, i64)>,
    registrations: Vec<Registration>,
    resets: usize,
    first_reset_only: bool,
    score: i64,
}
impl IncrementalCalculator<()> for Calculator {
    type Score = SimpleScore;
    type Justification = DefaultJustification<SimpleScore, ()>;
    type Change = ();
    fn supports_matches(&self) -> bool {
        self.aware
    }
    fn reset(
        &mut self,
        _: &(),
        registry: &mut MatchRegistry<Self::Score, Self::Justification>,
    ) -> Result<(), MatchRegistryError> {
        self.resets += 1;
        self.registrations.clear();
        if registry.is_enabled() && (!self.first_reset_only || self.resets == 1) {
            for &(name, value) in &self.entries {
                let score = SimpleScore::new(value);
                self.registrations.push(registry.register(
                    name,
                    score,
                    DefaultJustification {
                        score,
                        facts: vec![],
                    },
                )?);
            }
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.score)
    }
}
fn session(
    aware: bool,
    enabled: bool,
    entries: Vec<(&'static str, i64)>,
    first_reset_only: bool,
) -> CalculatorSession<(), Calculator> {
    let score = if first_reset_only {
        0
    } else {
        entries.iter().map(|(_, s)| s).sum()
    };
    CalculatorSession::new(
        (),
        Calculator {
            aware,
            entries,
            registrations: vec![],
            resets: 0,
            first_reset_only,
            score,
        },
        enabled,
    )
    .unwrap()
}
#[test]
fn illegal_state_exception_thrown_when_constraint_match_not_enabled() {
    let s = session(false, false, vec![], false);
    assert_eq!(
        s.registry().unwrap().constraints().unwrap_err(),
        MatchRegistryError::Disabled
    );
    assert!(
        s.registry()
            .unwrap()
            .constraints()
            .unwrap_err()
            .to_string()
            .contains("DISABLED")
    );
}
#[test]
fn constraint_match_totals_never_null() {
    assert!(
        session(true, true, vec![], false)
            .registry()
            .unwrap()
            .constraints()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn constraint_match_is_not_enabled_when_score_calculator_not_constraint_match_aware() {
    assert!(
        !session(false, true, vec![], false)
            .registry()
            .unwrap()
            .is_enabled()
    );
}
#[test]
fn register_constraint_match_throws_when_constraint_matching_disabled() {
    let mut s = session(false, false, vec![], false);
    let error = s
        .registry_mut()
        .unwrap()
        .register(
            "constraintA",
            SimpleScore::new(-1),
            DefaultJustification {
                score: SimpleScore::new(-1),
                facts: vec![],
            },
        )
        .unwrap_err();
    assert_eq!(error, MatchRegistryError::Disabled);
    assert!(error.to_string().contains("DISABLED"));
}
#[test]
fn register_constraint_match_updates_total_score_and_map() {
    let s = session(true, true, vec![("constraintA", -5)], false);
    let registry = s.registry().unwrap();
    assert_eq!(*registry.total().unwrap(), SimpleScore::new(-5));
    let a = &registry.constraints().unwrap()["constraintA"];
    assert_eq!(*a.score(), SimpleScore::new(-5));
    assert_eq!(a.matches().len(), 1);
}
#[test]
fn cancel_registration_updates_total_score_and_map() {
    let mut s = session(true, true, vec![("constraintA", -3)], false);
    assert_eq!(
        *s.registry().unwrap().total().unwrap(),
        SimpleScore::new(-3)
    );
    let registration = s.calculator().registrations[0].clone();
    registration.cancel(s.registry_mut().unwrap()).unwrap();
    let registry = s.registry().unwrap();
    assert_eq!(*registry.total().unwrap(), SimpleScore::ZERO);
    assert!(
        registry.constraints().unwrap()["constraintA"]
            .matches()
            .is_empty()
    );
}
#[test]
fn cancel_registration_twice_throws() {
    let mut s = session(true, true, vec![("constraintA", -1)], false);
    let registration = s.calculator().registrations[0].clone();
    registration.cancel(s.registry_mut().unwrap()).unwrap();
    let error = registration.cancel(s.registry_mut().unwrap()).unwrap_err();
    assert_eq!(error, MatchRegistryError::AlreadyCanceled);
    assert!(error.to_string().contains("canceled once"));
}
#[test]
fn multiple_constraint_matches_across_different_constraints() {
    let s = session(
        true,
        true,
        vec![
            ("constraintA", -2),
            ("constraintA", -3),
            ("constraintB", -5),
        ],
        false,
    );
    let registry = s.registry().unwrap();
    assert_eq!(*registry.total().unwrap(), SimpleScore::new(-10));
    let totals = registry.constraints().unwrap();
    assert_eq!(totals["constraintA"].matches().len(), 2);
    assert_eq!(*totals["constraintA"].score(), SimpleScore::new(-5));
    assert_eq!(totals["constraintB"].matches().len(), 1);
    assert_eq!(*totals["constraintB"].score(), SimpleScore::new(-5));
}
#[test]
fn reset_clears_constraint_match_map_and_resets_score() {
    let mut s = session(true, true, vec![("constraintA", -7)], true);
    assert_eq!(
        *s.registry().unwrap().total().unwrap(),
        SimpleScore::new(-7)
    );
    assert!(
        s.registry()
            .unwrap()
            .constraints()
            .unwrap()
            .contains_key("constraintA")
    );
    s.reset(()).unwrap();
    assert_eq!(*s.registry().unwrap().total().unwrap(), SimpleScore::ZERO);
    assert!(
        !s.registry()
            .unwrap()
            .constraints()
            .unwrap()
            .contains_key("constraintA")
    );
}
#[test]
fn registration_constraint_ref_and_score_are_accessible() {
    let s = session(true, true, vec![("constraintA", -4)], false);
    let record = s.calculator().registrations[0].record();
    assert_eq!(record.constraint, "constraintA");
    assert_eq!(record.score, SimpleScore::new(-4));
    let _: &DefaultJustification<SimpleScore, ()> = &record.justification;
}
