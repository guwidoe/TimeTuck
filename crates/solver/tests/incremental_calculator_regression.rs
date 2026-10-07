use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;

#[test]
fn registration_identity_reset_and_atomic_overflow() {
    let mut a = MatchRegistry::<SimpleScore, ()>::new(true);
    let mut b = MatchRegistry::<SimpleScore, ()>::new(true);
    let old = a.register("a", SimpleScore::new(1), ()).unwrap();
    b.register("a", SimpleScore::new(1), ()).unwrap();
    assert_eq!(
        old.cancel(&mut b),
        Err(MatchRegistryError::ForeignOrExpiredRegistration)
    );
    a.reset();
    let huge = a.register("a", SimpleScore::new(i64::MAX), ()).unwrap();
    assert_eq!(old.id(), huge.id());
    assert_eq!(
        old.cancel(&mut a),
        Err(MatchRegistryError::ForeignOrExpiredRegistration)
    );
    assert!(matches!(
        a.register("b", SimpleScore::new(1), ()),
        Err(MatchRegistryError::ScoreOverflow)
    ));
    assert!(!a.constraints().unwrap().contains_key("b"));
    let negative = a.register("b", SimpleScore::new(-1), ()).unwrap();
    // The grand total would fit but constraint a would overflow.
    assert!(matches!(
        a.register("a", SimpleScore::new(1), ()),
        Err(MatchRegistryError::ScoreOverflow)
    ));
    assert_eq!(*a.total().unwrap(), SimpleScore::new(i64::MAX - 1));
    let plus = a.register("b", SimpleScore::new(1), ()).unwrap();
    // Retracting a negative match can overflow; nothing is removed on failure.
    assert_eq!(
        negative.cancel(&mut a),
        Err(MatchRegistryError::ScoreOverflow)
    );
    assert_eq!(a.constraints().unwrap()["b"].matches().len(), 2);
    plus.cancel(&mut a).unwrap();
    negative.cancel(&mut a).unwrap();
    huge.cancel(&mut a).unwrap();
    assert_eq!(*a.total().unwrap(), SimpleScore::ZERO);
    assert_eq!(
        huge.clone().cancel(&mut a),
        Err(MatchRegistryError::AlreadyCanceled)
    );
    assert_eq!(old.record().score, SimpleScore::new(1));
    assert!(matches!(
        a.register("s", SimpleScore::new(1).with_structural(-1), ()),
        Err(MatchRegistryError::StructuralContribution)
    ));
}

#[derive(Default)]
struct SumCalculator {
    total: i64,
    registrations: Vec<MatchRegistration<SimpleScore, usize>>,
    resets: usize,
    before: usize,
    after: usize,
    fail_before: bool,
    fail_after: bool,
}
impl IncrementalCalculator<Vec<i64>> for SumCalculator {
    type Score = SimpleScore;
    type Justification = usize;
    type Change = usize;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        solution: &Vec<i64>,
        registry: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.resets += 1;
        self.total = solution.iter().sum();
        self.registrations.clear();
        self.fail_before = false;
        self.fail_after = false;
        if registry.is_enabled() {
            for (index, value) in solution.iter().enumerate() {
                self.registrations.push(registry.register(
                    "sum",
                    SimpleScore::new(*value),
                    index,
                )?);
            }
        }
        Ok(())
    }
    fn before_change(
        &mut self,
        solution: &Vec<i64>,
        index: &usize,
        registry: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.before += 1;
        if self.fail_before {
            return Err(MatchRegistryError::Calculator("before".into()));
        }
        self.total -= solution[*index];
        if registry.is_enabled() {
            self.registrations[*index].cancel(registry)?;
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        solution: &Vec<i64>,
        index: &usize,
        registry: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.after += 1;
        self.total += solution[*index];
        if registry.is_enabled() {
            self.registrations[*index] =
                registry.register("sum", SimpleScore::new(solution[*index]), *index)?;
        }
        if solution[*index] == 999 {
            self.fail_before = true;
        }
        if solution[*index] == 998 {
            self.fail_after = true;
        }
        if self.fail_after {
            return Err(MatchRegistryError::Calculator("after".into()));
        }
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.total)
    }
}

#[test]
fn incremental_updates_and_rejected_trials_match_full_recomputation() {
    for enabled in [false, true] {
        let mut s = CalculatorSession::new(vec![0; 8], SumCalculator::default(), enabled).unwrap();
        for step in 0..400 {
            let index = step % 8;
            let old = s.solution()[index];
            let candidate = ((step * 17) % 31) as i64 - 15;
            let previous = s.score().unwrap();
            s.change(&index, |a| a[index] = candidate).unwrap();
            assert_eq!(
                s.score().unwrap(),
                SimpleScore::new(s.solution().iter().sum())
            );
            if s.score().unwrap() < previous {
                s.change(&index, |a| a[index] = old).unwrap();
            }
            assert!(s.score().unwrap() >= previous);
            if enabled {
                let calculated = s.score().unwrap();
                assert_eq!(s.registry().unwrap().total().unwrap(), &calculated);
                let matches = s.registry().unwrap().constraints().unwrap()["sum"].matches();
                assert_eq!(matches.len(), 8);
                for record in matches.values() {
                    assert_eq!(
                        record.score,
                        SimpleScore::new(s.solution()[record.justification])
                    );
                }
            }
        }
        assert_eq!(s.calculator().resets, 1);
        assert_eq!(s.calculator().before, s.calculator().after);
        assert!(s.calculator().before >= 400);
        s.reset(vec![3, -4]).unwrap();
        assert_eq!(s.score().unwrap(), SimpleScore::new(-1));
        assert_eq!(s.calculator().resets, 2);
    }
}

#[test]
fn failed_callbacks_and_panics_require_reset() {
    let mut s = CalculatorSession::new(vec![0], SumCalculator::default(), true).unwrap();
    s.change(&0, |a| a[0] = 999).unwrap();
    assert!(matches!(
        s.change(&0, |_| panic!("must not run")),
        Err(MatchRegistryError::Calculator(_))
    ));
    assert_eq!(s.solution(), &[999]);
    assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
    assert!(matches!(s.registry(), Err(MatchRegistryError::NeedsReset)));
    s.reset(vec![2]).unwrap();
    assert!(matches!(
        s.change(&0, |a| a[0] = 998),
        Err(MatchRegistryError::Calculator(_))
    ));
    assert_eq!(s.solution(), &[998]);
    assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
    s.reset(vec![2]).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| s.change(&0, |a| {
            a[0] = 10;
            panic!("failed mutation")
        })))
        .is_err()
    );
    assert_eq!(s.score(), Err(MatchRegistryError::NeedsReset));
    assert_eq!(s.change(&0, |_| ()), Err(MatchRegistryError::NeedsReset));
    s.reset(vec![7]).unwrap();
    assert_eq!(s.score().unwrap(), SimpleScore::new(7));
    assert_eq!(*s.registry().unwrap().total().unwrap(), SimpleScore::new(7));
}
