//! Borrowed callback arguments for small scopes without heap allocation.

#[inline(always)]
pub(crate) fn with_facts<A, T, R>(
    scopes: &[A],
    mut project: impl FnMut(&A) -> T,
    evaluate: impl FnOnce(&[T]) -> R,
) -> R {
    match scopes {
        [] => evaluate(&[]),
        [a] => evaluate(&[project(a)]),
        [a, b] => evaluate(&[project(a), project(b)]),
        [a, b, c] => evaluate(&[project(a), project(b), project(c)]),
        [a, b, c, d] => evaluate(&[project(a), project(b), project(c), project(d)]),
        _ => evaluate(&scopes.iter().map(project).collect::<Vec<_>>()),
    }
}

#[cfg(test)]
mod tests {
    use super::with_facts;
    use std::cell::Cell;

    struct Fact<'a>(&'a Cell<usize>);
    impl Drop for Fact<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn projections_are_ordered_once_and_owned_facts_drop_on_return_or_unwind() {
        for count in 0..=6 {
            for panic_at in 0..=count + 1 {
                let visits = Cell::new(0);
                let drops = Cell::new(0);
                let scopes: Vec<_> = (0..count).collect();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    with_facts(
                        &scopes,
                        |&scope| {
                            assert_eq!(scope, visits.get());
                            visits.set(visits.get() + 1);
                            assert_ne!(visits.get(), panic_at, "projection panic");
                            Fact(&drops)
                        },
                        |facts| {
                            assert_eq!(facts.len(), count);
                            assert_ne!(panic_at, count + 1, "callback panic");
                        },
                    )
                }));
                if panic_at == 0 {
                    assert!(result.is_ok());
                    assert_eq!((visits.get(), drops.get()), (count, count));
                } else {
                    assert!(result.is_err());
                    let constructed = panic_at.min(count + 1) - 1;
                    assert_eq!(drops.get(), constructed);
                }
            }
        }
    }
}
