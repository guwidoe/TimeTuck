// Adapted from pinned Community PhaseLifecycleTest.verifyEventCounts.
// Apache-2.0; see compat/provenance.json.
mod lifecycle_support;
use lifecycle_support::*;
use timetuck::*;
#[test]
fn verify_event_counts() {
    for backend in 0..4 {
        let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 17]).unwrap();
        let mut events = Vec::new();
        let solved = basic_run(&p, backend, config(), Some(&mut |e| events.push(e)));
        counts(&events, 1, 2, 17 + 10);
        let variables = solved
            .assignment
            .values()
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let variable = Variable::new(vec![1, 2]);
                if i == 0 {
                    variable
                } else {
                    variable.with_initial(v.unwrap())
                }
            })
            .collect();
        let p2 = FiniteProblem::new(variables).unwrap();
        events.clear();
        basic_run(&p2, backend, config(), Some(&mut |e| events.push(e)));
        counts(&events, 1, 2, 1 + 10);
        events.clear();
        basic_run(&p, backend, config(), None);
        counts(&events, 0, 0, 0);
    }
}
