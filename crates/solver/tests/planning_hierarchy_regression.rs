#[path = "fixtures/planning_hierarchy_fixture.rs"]
mod fixture;
use fixture::solve;
use fixture::*;
use timetuck::*;
#[test]
fn multilevel_composition_reopens_live_ranges_and_writes_parent_storage() {
    struct Top {
        middle: ChildEntity,
    }
    struct Model {
        range: Vec<String>,
        rows: Vec<Top>,
    }
    let child = child_binding(Some("value2"));
    let top = PlanningEntityBinding::default()
        .inherit(child, |e: &Top| &e.middle, |e| &mut e.middle)
        .unwrap();
    let b = PlanningSolutionBinding::default().collection(
        top,
        |s: &Model| &s.rows,
        |s| &mut s.rows,
        |s, _| Some(s.range.clone()),
    );
    let mut s = Model {
        range: vec!["first".into()],
        rows: vec![Top {
            middle: ChildEntity::default(),
        }],
    };
    assert_eq!(
        b.schema()
            .unwrap()
            .genuine_entity::<Top>()
            .unwrap()
            .genuine_variables()
            .count(),
        2
    );
    b.commit(
        &mut s,
        &Assignment::new(vec![Some("first".into()), Some("first".into())]),
    )
    .unwrap();
    assert_eq!(s.rows[0].middle.base.value.as_deref(), Some("first"));
    s.range.push("later".into());
    assert_eq!(b.open(&s).unwrap().search_space_size(), Some(4));
    b.commit(
        &mut s,
        &Assignment::new(vec![Some("later".into()), Some("later".into())]),
    )
    .unwrap();
    assert_eq!(s.rows[0].middle.extra.as_deref(), Some("later"));
}
#[test]
fn late_invalid_value_and_changed_length_do_not_partially_write() {
    let b = solution_binding(Some("value2"));
    let mut s = solution(2, 2, "Value2");
    let first = s.values[0].clone();
    let second = s.extra_values[0].clone();
    let invalid = Assignment::new(vec![
        Some(first.clone()),
        Some(second.clone()),
        Some(first.clone()),
        Some("bad".into()),
    ]);
    assert!(b.commit(&mut s, &invalid).is_err());
    assert!(
        s.entities
            .iter()
            .all(|e| e.base.value.is_none() && e.extra.is_none())
    );
    let model = b.open(&s).unwrap();
    s.entities.push(ChildEntity::default());
    assert!(b.commit(&mut s, &model.initial_assignment()).is_err());
    assert!(s.entities.iter().all(|e| e.base.value.is_none()));
}
#[test]
fn trait_accessors_and_parent_solution_projection_execute_in_independent_managers() {
    for n in [0, 1, 3, 7] {
        let output = solve(
            interface_solution(2, n),
            || interface_solution_binding(true),
            (0..2 * n).collect(),
            1,
        );
        assert_eq!(
            output.score.unwrap().score(),
            interface_score(&output, true)
        );
        assert_eq!(interface_score(&output, true), (2 * n) as i64);
        assert!(
            output
                .entities
                .iter()
                .all(|e| output.values.contains(&e.value().unwrap())
                    && output.extra_values.contains(e.extra.as_ref().unwrap()))
        );
    }
    let input = ExtendedSolution {
        base: solution(2, 5, "Value2"),
        ..Default::default()
    };
    let output = solve(
        input,
        || extended_binding(1, Some("value2")),
        (0..10).collect(),
        1,
    );
    assert_eq!(
        output.base.score.unwrap().score(),
        class_score(&output.base, true)
    );
    assert_eq!(class_score(&output.base, true), 10);
}
#[test]
fn missing_range_is_preflighted_before_any_owned_solve() {
    let binding = PlanningEntityBinding::default()
        .member(
            "value",
            "absent",
            |e: &BaseEntity| e.value.clone(),
            |e, v| e.value = v,
        )
        .unwrap();
    struct S(Vec<BaseEntity>);
    let b = PlanningSolutionBinding::default().collection(
        binding,
        |s: &S| &s.0,
        |s| &mut s.0,
        |_, _| None,
    );
    let s = S(vec![BaseEntity::default()]);
    assert!(
        b.open(&s)
            .unwrap_err()
            .to_string()
            .contains("missing value range (absent) for member (value)")
    );
    assert!(s.0[0].value.is_none());
}

#[test]
fn stream_backend_executes_and_commits_inherited_members() {
    let b = solution_binding(Some("value2"));
    let mut s = solution(3, 3, "Value2");
    let model = b.open(&s).unwrap();
    let mut plan = SolverSequence::default();
    plan.limits.best_score_limit = Some(SimpleScore::ZERO);
    for phase in &mut plan.phases {
        if let SolverPhase::LocalSearch(c) = phase {
            c.max_steps = 40;
            c.max_evaluations = 80;
        }
    }
    let graph = || {
        let mut g = StreamGraphBuilder::new();
        let source = g.source::<usize, (usize, Option<String>)>();
        g.score(
            "each assigned inherited or child variable",
            source.port(),
            |(_, v)| SimpleScore::new(i64::from(v.is_some())),
        )?;
        Ok((g.build(), source))
    };
    let r = solve_stream_with_extensions(&model, graph, plan, SolverExtensions::default()).unwrap();
    assert!(r.complete);
    assert!(r.work.evaluations > 0 && r.work.evaluations <= 80);
    b.commit(&mut s, &r.assignment).unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(class_score(&s, true))));
    assert_eq!(class_score(&s, true), 6);
}

#[test]
fn later_mutable_projection_mismatch_preflights_all_collections_before_setters() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Parent {
        read: Vec<BaseEntity>,
        wrong_write: Vec<BaseEntity>,
    }
    struct Owned {
        first: Vec<BaseEntity>,
        parent: Parent,
    }
    for wrong_length in [0, 2] {
        let writes = Arc::new(AtomicUsize::new(0));
        let member = |writes: Arc<AtomicUsize>| {
            PlanningEntityBinding::default()
                .member(
                    "value",
                    "range",
                    |e: &BaseEntity| e.value.clone(),
                    move |e, v| {
                        writes.fetch_add(1, Ordering::SeqCst);
                        e.value = v;
                    },
                )
                .unwrap()
        };
        let parent = PlanningSolutionBinding::default().collection(
            member(writes.clone()),
            |s: &Parent| &s.read,
            |s| &mut s.wrong_write,
            |_, _| Some(vec!["ok".to_string()]),
        );
        let binding = PlanningSolutionBinding::default()
            .collection(
                member(writes.clone()),
                |s: &Owned| &s.first,
                |s| &mut s.first,
                |_, _| Some(vec!["ok".to_string()]),
            )
            .inherit(parent, |s| &s.parent, |s| &mut s.parent);
        let mut input = Owned {
            first: vec![BaseEntity::default()],
            parent: Parent {
                read: vec![BaseEntity::default()],
                wrong_write: vec![BaseEntity::default(); wrong_length],
            },
        };
        let assignment = Assignment::new(vec![Some("ok".into()), Some("ok".into())]);
        assert!(binding.open(&input).unwrap().validate(&assignment).is_ok());
        let error = binding.commit(&mut input, &assignment).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("collection (1) read/write slot count mismatch: read 1, write {wrong_length}")
        );
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        assert!(
            input
                .first
                .iter()
                .chain(&input.parent.read)
                .chain(&input.parent.wrong_write)
                .all(|e| e.value.is_none())
        );
    }
}
