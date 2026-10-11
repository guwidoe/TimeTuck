// Adapted from pinned Community DefaultSolverTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/assembled_factory_fixture.rs"]
mod basic;
#[path = "fixtures/observed_list_factory_fixture.rs"]
mod lists;
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
#[test]
fn ensure_list_variable_state_is_reused_across_phases() {
    for stream in [false, true] {
        let domain = lists::Domain::new(10, 4, false);
        let factory = lists::prepare_with_phases(
            &domain,
            stream,
            Rc::new(basic::Observations::default()),
            vec![],
            vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    ..Default::default()
                }),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    ..Default::default()
                }),
            ],
        );
        let states = RefCell::new(Vec::<RetainedListIndex>::new());
        let mut ended = |event, view: lists::View<'_>| {
            if matches!(event, FactoryRunEvent::PhaseCompleted { .. }) {
                let state = view
                    .read_runtime_view(|r| match &r.engine {
                        lists::Engine::Dense(d) => d.retained_list_index(),
                        lists::Engine::Stream(d) => d.retained_list_index(),
                    })
                    .unwrap();
                states.borrow_mut().push(state);
            }
            Ok(())
        };
        factory
            .solve_observed_assembled_with(
                domain.model.initial_assignment(),
                &mut FactoryRunCallbacks {
                    lifecycle: Some(&mut ended),
                    failure: None,
                },
                |r, phase| r.phase(phase, false),
            )
            .unwrap();
        let states = states.borrow();
        assert_eq!(states.len(), 3);
        assert!(states.iter().all(|state| state.same_instance(&states[0])));
    }
}
