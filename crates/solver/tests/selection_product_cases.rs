// Adapted from pinned Community CartesianProductMoveSelectorTest and PlannerAssert.
// Apache-2.0. Native snapshots/scripted child draws replace Java selectors; see provenance.
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Source {
    items: Vec<&'static str>,
    events: Vec<SelectionLifecycleEvent>,
}
impl SelectionSource<(), &'static str> for Source {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.items.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(self.items.iter().copied())
    }
}
fn run(children: &[&[&'static str]], ignore: bool, random: bool, size: u128, expected: &[&str]) {
    let mut product = SelectionProduct::new(
        children
            .iter()
            .map(|items| Source {
                items: items.to_vec(),
                events: vec![],
            })
            .collect(),
        ignore,
        3,
    );
    for event in [SolveStarted, PhaseStarted, StepStarted] {
        product.event(event, &());
    }
    let snapshot = product.snapshot(&()).unwrap();
    assert_eq!(snapshot.cardinality(), size);
    let mut counts = vec![0; children.len()];
    let mut sampled = snapshot.samples_with(move |child, bound| {
        let index = counts[child] % bound;
        counts[child] += 1;
        index
    });
    let mut original = snapshot.iter();
    assert!(!original.is_never_ending());
    assert!(sampled.is_never_ending());
    for &code in expected {
        assert!(if random {
            sampled.has_next()
        } else {
            original.has_next()
        });
        let row = if random {
            sampled.next()
        } else {
            original.next()
        }
        .unwrap();
        assert_eq!(row.iter().map(|s| **s).collect::<Vec<_>>().join("+"), code);
    }
    if random && !expected.is_empty() {
        assert!(sampled.has_next());
    } else if random {
        assert!(!sampled.has_next());
        assert_eq!(sampled.next(), None);
        assert_eq!(sampled.next(), None);
    } else {
        assert!(!original.has_next());
        assert_eq!(original.next(), None);
        assert_eq!(original.next(), None);
    }
    for event in [StepEnded, PhaseEnded, SolveEnded] {
        product.event(event, &());
    }
    for child in product.children() {
        for event in [
            SolveStarted,
            PhaseStarted,
            StepStarted,
            StepEnded,
            PhaseEnded,
            SolveEnded,
        ] {
            assert_eq!(child.events.iter().filter(|&&e| e == event).count(), 1);
        }
    }
}
macro_rules! pair {
    ($no:ident,$yes:ident,$body:expr) => {
        #[test]
        fn $no() {
            ($body)(false);
        }
        #[test]
        fn $yes() {
            ($body)(true);
        }
    };
}
pair!(original_not_ignoring, original_ignoring, |ignore| run(
    &[&["a1", "a2", "a3"], &["b1", "b2"]],
    ignore,
    false,
    6,
    &["a1+b1", "a1+b2", "a2+b1", "a2+b2", "a3+b1", "a3+b2"]
));
fn empty_original(ignore: bool, first: bool, second: bool) {
    let a: &[&str] = if first { &[] } else { &["a1", "a2", "a3"] };
    let b: &[&str] = if second { &[] } else { &["a1", "a2", "a3"] };
    let expected: &[&str] = if ignore && !(first && second) {
        &["a1", "a2", "a3"]
    } else {
        &[]
    };
    run(&[a, b], ignore, false, expected.len() as u128, expected);
}
pair!(empty_first_not_ignoring, empty_first_ignoring, |ignore| {
    empty_original(ignore, true, false)
});
pair!(empty_second_not_ignoring, empty_second_ignoring, |ignore| {
    empty_original(ignore, false, true)
});
pair!(empty_all_not_ignoring, empty_all_ignoring, |ignore| {
    empty_original(ignore, true, true)
});
pair!(
    original_three_not_ignoring,
    original_three_ignoring,
    |ignore| run(
        &[&["a1", "a2"], &["b1", "b2"], &["c1", "c2"]],
        ignore,
        false,
        8,
        &[
            "a1+b1+c1", "a1+b1+c2", "a1+b2+c1", "a1+b2+c2", "a2+b1+c1", "a2+b1+c2", "a2+b2+c1",
            "a2+b2+c2"
        ]
    )
);
pair!(
    empty_original_three_not_ignoring,
    empty_original_three_ignoring,
    |ignore| {
        let expected: &[&str] = if ignore {
            &["a1+c1", "a1+c2", "a2+c1", "a2+c2"]
        } else {
            &[]
        };
        run(
            &[&["a1", "a2"], &[], &["c1", "c2"]],
            ignore,
            false,
            expected.len() as u128,
            expected,
        );
    }
);
pair!(random_not_ignoring, random_ignoring, |ignore| run(
    &[&["a1", "a2", "a3"], &["b1", "b2"]],
    ignore,
    true,
    6,
    &["a1+b1", "a2+b2", "a3+b1", "a1+b2", "a2+b1", "a3+b2"]
));
pair!(empty_random_not_ignoring, empty_random_ignoring, |ignore| {
    let expected: &[&str] = if ignore { &["b1", "b2"] } else { &[] };
    run(
        &[&[], &["b1", "b2"]],
        ignore,
        true,
        expected.len() as u128,
        expected,
    );
});
pair!(random_three_not_ignoring, random_three_ignoring, |ignore| {
    run(
        &[&["a1", "a2"], &["b1", "b2"], &["c1", "c2"]],
        ignore,
        true,
        8,
        &["a1+b1+c1", "a2+b2+c2", "a1+b1+c1"],
    )
});
pair!(
    empty_random_three_not_ignoring,
    empty_random_three_ignoring,
    |ignore| {
        let expected: &[&str] = if ignore {
            &["b1+c1", "b2+c2", "b1+c3"]
        } else {
            &[]
        };
        run(
            &[&[], &["b1", "b2"], &["c1", "c2", "c3"]],
            ignore,
            true,
            if ignore { 6 } else { 0 },
            expected,
        );
    }
);
