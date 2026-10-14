// Original service ownership/runtime controls; no upstream method credit.
#[path = "fixtures/constraint_services_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use timetuck::*;
#[test]
fn jobs_solution_operations_metadata_and_verifier_share_the_consumed_resource() {
    for backend in [Backend::Dense, Backend::Stream] {
        let recipe = resource();
        let consumed = Arc::new(AtomicUsize::new(0));
        let expected = recipe.clone();
        let calls = consumed.clone();
        let f = SharedConstraintSolverFactory::new(
            recipe,
            SolverConfig {
                local_search: vec![LocalSearchConfig {
                    max_steps: 10,
                    ..Default::default()
                }],
                ..Default::default()
            },
            move |actual, solution, configuration, progress| {
                assert!(Arc::ptr_eq(actual, &expected));
                calls.fetch_add(1, Ordering::Relaxed);
                run(actual, solution, configuration, progress, backend)
            },
        )
        .unwrap();
        let verifier = f.constraint_verifier();
        let operations = f.solution_manager();
        assert!(Arc::ptr_eq(f.score_factory(), verifier.score_factory()));
        assert!(Arc::ptr_eq(f.score_factory(), operations.score_factory()));
        let mut manager = f.build_manager(2).unwrap();
        assert!(Arc::ptr_eq(manager.factory(), &f));
        let jobs: Vec<_> = (0..2)
            .map(|id| {
                manager
                    .job_builder(id)
                    .with_problem(source())
                    .run()
                    .unwrap()
            })
            .collect();
        for job in jobs {
            let result = job.wait();
            let solved = result.as_ref().as_ref().unwrap();
            assert!(solved.entities.iter().all(Option::is_some));
            assert_eq!(solved.score, Some(SimpleScore::ZERO));
            assert_eq!(solved.score, Some(independent_score(solved)));
            let p = model(solved);
            verifier
                .verify_score(&p, &p.initial_assignment(), solved.score.as_ref().unwrap())
                .unwrap();
            let mut owned = ManagedSolution::new(p.initial_assignment(), ());
            assert_eq!(
                operations
                    .update(&p, &mut owned, SolutionUpdatePolicy::Everything)
                    .unwrap(),
                solved.score
            );
            let analysis = operations.analyze(&p, &owned.assignment).unwrap();
            assert!(Arc::ptr_eq(&analysis.meta_model, f.meta_model()));
            assert_eq!(
                analysis.constraint_scores,
                vec![(NAME.into(), SimpleScore::ZERO)]
            );
        }
        manager.shutdown().unwrap();
        assert_eq!(consumed.load(Ordering::Relaxed), 2);
    }
}
#[test]
fn updates_publish_actual_value_changes_and_preserve_metadata_identity() {
    let f = factory(Backend::Stream);
    let operations = f.solution_manager();
    let mut s = source();
    s.entities = vec![Some("v1".into()), Some("v1".into())];
    let p = model(&s);
    let mut owned = ManagedSolution::new(p.initial_assignment(), ());
    assert_eq!(
        operations
            .update(&p, &mut owned, SolutionUpdatePolicy::Everything)
            .unwrap(),
        Some(SimpleScore::new(-2))
    );
    owned.assignment = Assignment::new(vec![Some("v1".into()), Some("v2".into())]);
    assert_eq!(
        operations
            .update(&p, &mut owned, SolutionUpdatePolicy::ShadowsOnly)
            .unwrap(),
        Some(SimpleScore::new(-2))
    );
    assert_eq!(
        operations
            .update(&p, &mut owned, SolutionUpdatePolicy::ScoreOnly)
            .unwrap(),
        Some(SimpleScore::ZERO)
    );
    let report = operations.analyze(&p, &owned.assignment).unwrap();
    assert_eq!(report.score, SimpleScore::ZERO);
    assert!(Arc::ptr_eq(&report.meta_model, f.meta_model()));
    assert_eq!(f.meta_model().constraints()[0].weight, SimpleScore::new(1));
    owned.assignment = Assignment::new(vec![Some("invalid".into()), Some("v1".into())]);
    let before = owned.clone();
    assert!(
        operations
            .update(&p, &mut owned, SolutionUpdatePolicy::Everything)
            .is_err()
    );
    assert_eq!(owned, before);
}
#[test]
fn compilation_creates_independent_mutable_directors() {
    let resource = resource();
    let mut s = source();
    s.entities = vec![Some("v1".into()), Some("v1".into())];
    let p = model(&s);
    let a = resource.compile().unwrap();
    let b = resource.compile().unwrap();
    let mut first =
        StreamScoreDirector::new(&p, a.graph, a.source, p.initial_assignment()).unwrap();
    let second = StreamScoreDirector::new(&p, b.graph, b.source, p.initial_assignment()).unwrap();
    first
        .apply(&Move::Change {
            variable: 1,
            value: Some("v2".into()),
        })
        .unwrap()
        .commit();
    assert_eq!(first.score(), SimpleScore::ZERO);
    assert_eq!(second.score(), SimpleScore::new(-2));
    first.assert_consistent().unwrap();
    assert_eq!(
        resource.meta_model().constraints()[0].weight,
        SimpleScore::new(1)
    );
}
struct ChangingProvider(Arc<AtomicBool>);
impl ReadOnlyConstraintProvider<String, SimpleScore> for ChangingProvider {
    fn compile(&self) -> Result<CompiledConstraintGraph<String, SimpleScore>, GraphError> {
        let mut compiled = PairProvider.compile()?;
        if self.0.load(Ordering::Relaxed) {
            compiled.graph.apply_weight_overrides(
                &[(NAME.into(), SimpleScore::new(2))]
                    .into_iter()
                    .collect::<ConstraintWeightOverrides<_>>(),
            )?;
        }
        Ok(compiled)
    }
}
#[test]
fn changed_recipe_rejects_before_solution_publication() {
    let change = Arc::new(AtomicBool::new(false));
    let resource =
        Arc::new(ScoreFactoryResource::new(Arc::new(ChangingProvider(change.clone()))).unwrap());
    let operations = ConstraintSolutionManager::new(resource.clone());
    let p = model(&source());
    let mut owned = ManagedSolution::new(p.initial_assignment(), ());
    owned.score = Some(SimpleScore::new(99));
    let before = owned.clone();
    change.store(true, Ordering::Relaxed);
    assert!(matches!(
        operations.update(&p, &mut owned, SolutionUpdatePolicy::Everything),
        Err(ConstraintServiceError::MetadataChanged)
    ));
    assert_eq!(owned, before);
    assert_eq!(
        resource.meta_model().constraints()[0].weight,
        SimpleScore::new(1)
    );
    change.store(false, Ordering::Relaxed);
    assert_eq!(
        operations
            .update(&p, &mut owned, SolutionUpdatePolicy::Everything)
            .unwrap(),
        Some(SimpleScore::ZERO.with_structural(-2))
    );
}
#[test]
fn verifier_recomputes_all_ordered_pairs_and_reports_mismatches() {
    let f = factory(Backend::Stream);
    let verifier = f.constraint_verifier();
    for count in 0..=4 {
        for bits in 0..(1usize << count) {
            let mut s = source();
            s.entities = (0..count)
                .map(|i| Some(format!("v{}", 1 + ((bits >> i) & 1))))
                .collect();
            let p = model(&s);
            let expected = independent_score(&s);
            verifier
                .verify_score(&p, &p.initial_assignment(), &expected)
                .unwrap();
            let report = f
                .solution_manager()
                .analyze(&p, &p.initial_assignment())
                .unwrap();
            assert_eq!(report.constraint_scores, vec![(NAME.into(), expected)]);
        }
    }
    let mut s = source();
    s.entities = vec![Some("v1".into()), Some("v1".into())];
    let p = model(&s);
    assert!(matches!(
        verifier.verify_score(&p, &p.initial_assignment(), &SimpleScore::ZERO),
        Err(ConstraintServiceError::ScoreMismatch { .. })
    ));
}
#[test]
fn manager_retains_the_canonical_factory_until_drop() {
    let f = factory(Backend::Stream);
    let weak = Arc::downgrade(&f);
    let mut manager = f.build_manager(1).unwrap();
    let verifier = f.constraint_verifier();
    let meta = f.meta_model().clone();
    drop(f);
    assert!(Arc::ptr_eq(&weak.upgrade().unwrap(), manager.factory()));
    let job = manager.job_builder(7).with_problem(source()).run().unwrap();
    let result = job.wait();
    let solved = result.as_ref().as_ref().unwrap();
    assert_eq!(solved.score, Some(independent_score(solved)));
    manager.shutdown().unwrap();
    drop(manager);
    assert!(weak.upgrade().is_none());
    let p = model(solved);
    verifier
        .verify_score(&p, &p.initial_assignment(), &SimpleScore::ZERO)
        .unwrap();
    assert!(Arc::ptr_eq(&meta, verifier.score_factory().meta_model()));
}
struct InvalidProvider(bool);
impl ReadOnlyConstraintProvider<String, SimpleScore> for InvalidProvider {
    fn compile(&self) -> Result<CompiledConstraintGraph<String, SimpleScore>, GraphError> {
        let mut builder = StreamGraphBuilder::new();
        let source = builder.source::<usize, (usize, Option<String>)>();
        if self.0 {
            builder.score("unweighted", source.port(), |_| SimpleScore::ZERO)?;
        } else {
            builder.score_weighted(NAME, source.port(), SimpleScore::new(1), |_| -1)?;
            let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
            return Ok(CompiledConstraintGraph {
                graph: builder.build(),
                source: foreign.source::<usize, (usize, Option<String>)>(),
            });
        }
        Ok(CompiledConstraintGraph {
            graph: builder.build(),
            source,
        })
    }
}
#[test]
fn incompatible_preparations_fail_without_fake_metadata() {
    assert!(matches!(
        ScoreFactoryResource::new(Arc::new(InvalidProvider(true))),
        Err(ConstraintServiceError::UnweightedTerminal)
    ));
    assert!(matches!(
        ScoreFactoryResource::new(Arc::new(InvalidProvider(false))),
        Err(ConstraintServiceError::Graph(GraphError::ForeignPort))
    ));
}
#[test]
fn identical_names_do_not_substitute_for_resource_identity() {
    let first = resource();
    let second = resource();
    assert_eq!(first.meta_model(), second.meta_model());
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(!Arc::ptr_eq(first.meta_model(), second.meta_model()));
}
