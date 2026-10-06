// Adapted from Timefold Community DefaultSolverTest.solveCustomConfigListVariable.
// Apache-2.0; exact source/test fixture attribution: compat/provenance.json.
use timetuck::*;
struct SelectedValue;
impl SelectionSource<usize, usize> for SelectedValue {
    fn size(&mut self, _: &usize) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, key: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(std::iter::once(*key))
    }
}
struct ListFactory {
    recording: RecordingSelection<usize, usize, SelectedValue>,
    published: Vec<usize>,
    replayed: Vec<usize>,
}
impl PlacementMoveFactory<ListProblem<usize>, ListAssignment<usize>, usize, ListMove<usize>>
    for ListFactory
{
    fn candidates<'a>(
        &mut self,
        p: &'a ListProblem<usize>,
        a: &'a ListAssignment<usize>,
        key: &usize,
    ) -> Box<dyn Iterator<Item = ListMove<usize>> + 'a>
    where
        ListMove<usize>: 'a,
    {
        if a.location(key).is_some() {
            return Box::new(std::iter::empty());
        }
        let replay = self.recording.replaying();
        let mut rec = self.recording.open(key);
        let published = rec.next().unwrap();
        self.published.push(*published);
        // A dependent selector consumes the actual replay publication, not the
        // outer queue key, to select which value's insertion moves it produces.
        let mut cursor = replay.cursor();
        let value = cursor.next_replayed().unwrap().unwrap();
        self.replayed.push(*value);
        assert!(std::sync::Arc::ptr_eq(&published, &value));
        assert!(cursor.next_replayed().unwrap().is_none());
        Box::new(
            list_assign_moves(p, a)
                .unwrap()
                .filter(move |m| matches!(m,ListMove::Assign{value:v,..} if *v==*value)),
        )
    }
}
#[test]
fn solve_custom_config_list_variable() {
    for stream in [false, true] {
        let p = ListProblem::new(vec![0usize, 1], vec![ListEntity::new(vec![]); 2]).unwrap();
        let mut q = QueuedMoveProvider::new(
            vec![0, 1],
            PlacementQueueOrder::Once,
            ListFactory {
                recording: RecordingSelection::new(SelectedValue),
                published: vec![],
                replayed: vec![],
            },
        );
        let mut events = 0;
        let mut observer = |e: SolverEvent<'_, ListAssignment<usize>, SimpleScore>| {
            if matches!(e, SolverEvent::Best(_)) {
                events += 1;
            }
        };
        let plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                }),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 16,
                    assert_scores: true,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let r = if stream {
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut q);
            ext.events = Some(&mut observer);
            solve_list_stream_with_extensions(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = ListStreamSources::new(&mut b);
                    b.score("owner size", s.entities.port(), |(_, list)| {
                        SimpleScore::new(if list.len() == 1 { 2 } else { 1 })
                    })?;
                    Ok((b.build(), s))
                },
                plan,
                ext,
            )
            .unwrap()
        } else {
            let mut n = ListConstraintNetwork::new(2, 2);
            for i in 0..2 {
                n.add_constraint(format!("owner{i}"), vec![ListScope::Entity(i)], |f| {
                    let ListFact::Entity(list) = &f[0] else {
                        panic!()
                    };
                    SimpleScore::new(if list.len() == 1 { 2 } else { 1 })
                })
                .unwrap();
            }
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut q);
            ext.events = Some(&mut observer);
            solve_list_with_extensions(&p, &n, plan, ext).unwrap()
        };
        assert!(events > 0);
        assert!(r.assignment.lists().iter().all(|list| !list.is_empty()));
        assert!(r.complete);
        assert_eq!(r.score, Some(SimpleScore::new(4)));
        assert_eq!(r.phases.len(), 2);
        assert_eq!(q.factory().published, vec![0, 1]);
        assert_eq!(q.factory().replayed, q.factory().published);
    }
}
