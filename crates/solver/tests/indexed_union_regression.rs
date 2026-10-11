//! Original large provider-union checks; no upstream test credit.
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
struct Provider {
    id: usize,
    log: Rc<RefCell<Vec<(usize, u64)>>>,
}
impl MoveProvider<(), (), Result<usize, usize>> for Provider {
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        _: &'a (),
        step: u64,
    ) -> Box<dyn Iterator<Item = Result<usize, usize>> + 'a>
    where
        Result<usize, usize>: 'a,
    {
        self.log.borrow_mut().push((self.id, step));
        Box::new((0..self.id % 4).map(|item| if item == 1 { Err(self.id) } else { Ok(self.id) }))
    }
}
#[test]
fn large_provider_union_opens_only_selected_children_once_and_preserves_errors() {
    let log = Rc::new(RefCell::new(vec![]));
    let mut providers: Vec<_> = (0..96)
        .map(|id| Provider {
            id,
            log: log.clone(),
        })
        .collect();
    let children = providers.iter_mut().map(|p| {
        let weight = if p.id % 9 == 0 { 0 } else { u32::MAX };
        (
            p as &mut dyn MoveProvider<(), (), Result<usize, usize>>,
            weight,
        )
    });
    let mut union = ProviderUnion::new(children, UnionOrder::Random { seed: 9 }).unwrap();
    let mut cursor = union.moves(&(), &(), 6);
    assert!(log.borrow().is_empty());
    let first = cursor.next();
    assert!(first.is_some());
    // Some selected children may be empty, but no unselected provider is opened.
    let reference = MoveUnion::new(
        (0..96).map(|id| {
            (
                (0..id % 4).map(move |item| if item == 1 { Err(id) } else { Ok(id) }),
                if id % 9 == 0 { 0 } else { u32::MAX },
            )
        }),
        UnionOrder::Random { seed: 15 },
    )
    .collect::<Vec<_>>();
    let mut actual = vec![first.unwrap()];
    actual.extend(cursor.by_ref());
    assert_eq!(actual, reference);
    assert!(actual.iter().any(Result::is_err));
    assert_eq!(cursor.next(), None);
    drop(cursor);
    let mut opened = log.borrow().clone();
    opened.sort();
    assert_eq!(
        opened,
        (0..96)
            .filter(|id| id % 9 != 0)
            .map(|id| (id, 6))
            .collect::<Vec<_>>()
    );
    log.borrow_mut().clear();
    assert_eq!(union.moves(&(), &(), 6).collect::<Vec<_>>(), actual);
}
