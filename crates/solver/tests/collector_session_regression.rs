use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
#[test]
fn captured_mapping_is_not_recomputed_and_handles_do_not_alias() {
    type Session = CollectorSession<(u64, i32), ListCollector<u64, i32>>;
    let mut first = Session::new();
    let mut second = Session::new();
    let mut source = 7;
    let a = first.insert_with(|id| (id, source)).unwrap();
    let b = first.insert_with(|id| (id, source)).unwrap();
    let foreign = second.insert_with(|id| (id, 99)).unwrap();
    let saved = first.finish();
    source = 20;
    assert_eq!(
        first.remove(&foreign),
        Err(CollectorSessionError::ForeignHandle)
    );
    assert_eq!(first.remove(&a), Ok((a.ordinal(), 7)));
    assert_eq!(
        first.remove(&a.clone()),
        Err(CollectorSessionError::MissingHandle)
    );
    assert_eq!(first.finish().iter().copied().collect::<Vec<_>>(), [7]);
    assert_eq!(
        first.replace_with(&b, |id| (id, source)),
        Ok((b.ordinal(), 7))
    );
    assert_eq!(first.finish().iter().copied().collect::<Vec<_>>(), [20]);
    assert_eq!(saved.iter().copied().collect::<Vec<_>>(), [7, 7]);
    first.clear();
    assert!(first.is_empty());
    assert_eq!(
        first.contribution(&b),
        Err(CollectorSessionError::MissingHandle)
    );
    let c = first.insert_with(|id| (id, 30)).unwrap();
    assert!(c.ordinal() > b.ordinal());
    assert_eq!(first.remove(&b), Err(CollectorSessionError::MissingHandle));
    assert_eq!(first.finish().iter().copied().collect::<Vec<_>>(), [30]);
}
#[derive(Clone, Default)]
struct Failing(i32);
impl ReversibleCollector<i32> for Failing {
    type Output = i32;
    fn insert(&mut self, v: &i32) {
        self.0 += v;
        assert!(*v != 99, "delegate failure");
    }
    fn remove(&mut self, v: &i32) -> bool {
        self.0 -= v;
        *v != 13
    }
    fn finish(&self) -> i32 {
        self.0
    }
}
#[test]
fn failures_leave_captured_values_and_collector_together() {
    let mut c = CollectorSession::<i32, Failing>::new();
    let a = c.insert(4).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(99))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert_with(|_| panic!("mapping")))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace_with(&a, |_| 99))).is_err());
    assert_eq!(c.finish(), 4);
    assert_eq!(c.len(), 1);
    assert_eq!(c.contribution(&a), Ok(&4));
    let b = c.insert(13).unwrap();
    assert_eq!(b.ordinal(), a.ordinal() + 1);
    assert_eq!(
        c.remove(&b),
        Err(CollectorSessionError::RejectedContribution)
    );
    assert_eq!(
        c.replace_with(&b, |_| 5),
        Err(CollectorSessionError::RejectedContribution)
    );
    assert_eq!(c.finish(), 17);
    assert_eq!(c.len(), 2);
    assert_eq!(c.contribution(&b), Ok(&13));
    assert_eq!(c.remove(&a), Ok(4));
    assert_eq!(c.finish(), 13);
}
#[test]
fn contributions_need_no_clone_and_rejected_handles_skip_mapping() {
    struct Payload(i32);
    let mut c = CollectorSession::<Payload, CountCollector>::new();
    let a = c.insert(Payload(3)).unwrap();
    assert_eq!(c.contribution(&a).unwrap().0, 3);
    assert_eq!(c.replace_with(&a, |_| Payload(4)).unwrap().0, 3);
    assert_eq!(c.finish(), 1);
    assert_eq!(c.remove(&a).unwrap().0, 4);
    assert!(matches!(
        c.replace_with(&a, |_| panic!("must not map missing handle")),
        Err(CollectorSessionError::MissingHandle)
    ));
    assert_eq!(c.finish(), 0);
}
