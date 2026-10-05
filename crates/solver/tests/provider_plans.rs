//! Original full-plan provider integration across models and scoring backends.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3, 4]).with_initial(0)]).unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![7],
        (0..5)
            .map(|i| ListEntity::new(if i == 0 { vec![7] } else { vec![] }))
            .collect(),
    )
    .unwrap()
}
fn bm(_: &Assignment<i32>, v: i32) -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(v),
    }
}
fn lm(a: &ListAssignment<i32>, v: i32) -> ListMove<i32> {
    ListMove::Change {
        source: a.location(&7).unwrap(),
        destination: ListPosition {
            entity: v as usize,
            index: 0,
        },
    }
}
fn mm(a: &MixedAssignment<i32, i32>, v: i32) -> MixedMove<i32, i32> {
    MixedMove {
        basic: bm(a.basic(), v),
        lists: lm(a.lists(), v),
    }
}
macro_rules! exercise {
    ($P:ty,$A:ty,$M:ty,$E:ty,$p:expr,$objective:expr,$solve:ident,$Plan:ident,$phase:expr,$construction:expr,$custom:expr,$state:expr,$movement:expr,$factor:expr,$calls:expr) => {{
        let p=$p; let phase=$phase; let state=$state; let movement=$movement;
        // Combine construction, two separately bound providers, a worsening
        // custom command and an unbound built-in local phase.
        for observe in [false,true] {
            let mut first_seen=vec![];let mut second_seen=vec![];let mut commands=vec![];
            let mut first=|_:&$P,a:&$A,step|{first_seen.push((step,state(a)));vec![movement(a,2)]};
            let mut second=|_:&$P,a:&$A,step|{second_seen.push((step,state(a)));vec![movement(a,3)]};
            let mut command=|i,ctx:&mut CustomPhaseContext<'_,$A,$M,SimpleScore,$E>|{
                commands.push((i,state(ctx.assignment())));ctx.execute(&movement(ctx.assignment(),1))?;Ok(())
            };
            let mut best=vec![];let mut initialized=vec![];
            let mut events=|event:SolverEvent<'_,$A,SimpleScore>|match event{
                SolverEvent::Best(u)=>best.push((u.phase,u.score)),
                SolverEvent::FirstInitialized{initialization,complete,..}=>initialized.push((initialization.phase,complete)),
            };
            let custom_config=LocalSearchConfig{max_steps:1,neighborhood:Neighborhood::MassUnassign(MassAssignmentSize::all()),assert_scores:true,..Default::default()};
            let plan=$Plan{phases:vec![$construction,phase(custom_config),$custom,phase(custom_config),phase(LocalSearchConfig{max_steps:2,assert_scores:true,..Default::default()})],..Default::default()};
            let mut extensions=SolverExtensions::default();
            extensions.move_providers.bind(1,&mut first);extensions.move_providers.bind(3,&mut second);
            extensions.command=Some(&mut command);if observe{extensions.events=Some(&mut events);}
            let r=$solve(&p,$objective,plan,extensions).unwrap();
            assert_eq!(r.score,Some(s(4*$factor)));assert!(r.complete);assert_eq!(state(&r.assignment),4);
            assert_eq!(r.termination,SolverTermination::PlanCompleted);assert_eq!(r.phases.len(),5);
            assert_eq!(first_seen,vec![(0,0)]);assert_eq!(second_seen,vec![(0,2)]);assert_eq!(commands,vec![(2,2)]);
            if observe{
                assert_eq!(initialized,vec![(0,true)]);
                assert!(best.contains(&(1,s(2*$factor))));assert!(best.contains(&(3,s(3*$factor))));
                assert_eq!(best.last(),Some(&(4,s(4*$factor))));
                assert!(best.windows(2).all(|w|w[0].1<w[1].1));
            }
        }
        // Shared limits and event-driven cancellation stop later providers.
        for mode in 0..3 {
            let token=CancellationToken::new();let condition=SearchTermination::Condition(SearchLimit::Steps(1));
            let mut first_calls=0;
            let mut first=|_:&$P,a:&$A,_|{first_calls+=1;vec![movement(a,if state(a)==0 {2} else {(state(a)+1).min(4)})]};
            let mut later=|_:&$P,_:&$A,_|->Vec<$M>{panic!("later provider ran after shared stop")};
            let mut event=|event:SolverEvent<'_,$A,SimpleScore>|{
                if let SolverEvent::Best(u)=event && u.score>s(0) {token.cancel();}
            };
            let plan=$Plan{phases:vec![phase(LocalSearchConfig{max_steps:3,..Default::default()}),phase(LocalSearchConfig::default())],limits:SolverLimits{
                max_move_evaluations:(mode==0).then_some(1),termination:(mode==1).then_some(&condition),cancellation:Some(&token),..Default::default()
            }};
            let mut extensions=SolverExtensions::default();extensions.move_providers.bind(0,&mut first);extensions.move_providers.bind(1,&mut later);
            if mode==2{extensions.events=Some(&mut event);}
            let r=$solve(&p,$objective,plan,extensions).unwrap();
            assert_eq!(r.score,Some(s(2*$factor)));assert_eq!(r.work.move_evaluations,1);assert_eq!(r.phases.len(),1);assert_eq!(first_calls,if mode==0 {2} else {1},"provider calls for mode {mode}");
            assert_eq!(r.termination,match mode{0=>SolverTermination::MoveEvaluationLimit,1=>SolverTermination::Condition,_=>SolverTermination::Cancelled});
        }
        // Binding one phase must not disable validation of other phases.
        {
            let phase = $phase;
            $calls.set(0);
            let children = [(Neighborhood::MassUnassign(MassAssignmentSize::all()), 1),
                (Neighborhood::MassListUnassign(ListSampleSize::all()), 1)];
            let union = NeighborhoodUnion {children: &children, order: UnionOrder::Sequential};
            let plan = $Plan {phases: vec![phase(LocalSearchConfig {neighborhood_union: Some(&union), ..Default::default()}),
                phase(LocalSearchConfig::default())], ..Default::default()};
            let mut provider = |_: &$P, _: &$A, _| -> Vec<$M> {panic!("validation invoked provider")};
            let mut extensions = SolverExtensions::default();extensions.move_providers.bind(1, &mut provider);
            assert!(matches!($solve(&p, $objective, plan, extensions), Err(SolverError::Phase {phase: 0, ..})));
            assert_eq!($calls.get(), 0);
        }
        // Binding validation precedes scoring, graph factories and callbacks.
        for index in [0,3] {
            $calls.set(0);
            let mut provider=|_:&$P,_:&$A,_|->Vec<$M>{panic!("invalid binding invoked provider")};
            let mut events=|_:SolverEvent<'_,$A,SimpleScore>|panic!("invalid binding invoked events");
            let mut extensions=SolverExtensions::default();extensions.move_providers.bind(index,&mut provider);extensions.events=Some(&mut events);
            let plan=$Plan{phases:vec![$construction,phase(LocalSearchConfig::default())],..Default::default()};
            assert!(matches!($solve(&p,$objective,plan,extensions),Err(SolverError::InvalidMoveProviderPhase{phase}) if phase==index));
            assert_eq!($calls.get(),0);
        }
    }};
}

#[test]
fn basic_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.set(calls.get() + 1);
        s(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    exercise!(
        FiniteProblem<i32>,
        Assignment<i32>,
        Move<i32>,
        ScoringError,
        basic(),
        &n,
        solve_with_extensions,
        SolverSequence,
        SolverPhase::LocalSearch,
        SolverPhase::Construction(ConstructionConfig::default()),
        SolverPhase::Custom(CustomPhaseConfig::default()),
        |a: &Assignment<i32>| a.values()[0].unwrap(),
        bm,
        1,
        calls
    );
}

#[test]
fn basic_stream_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut graph = || {
        calls.set(calls.get() + 1);
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, (usize, Option<i32>)>();
        b.score("value", source.port(), |(_, v)| s(i64::from(v.unwrap())))?;
        Ok((b.build(), source))
    };
    exercise!(
        FiniteProblem<i32>,
        Assignment<i32>,
        Move<i32>,
        StreamScoringError,
        basic(),
        &mut graph,
        solve_stream_with_extensions,
        SolverSequence,
        SolverPhase::LocalSearch,
        SolverPhase::Construction(ConstructionConfig::default()),
        SolverPhase::Custom(CustomPhaseConfig::default()),
        |a: &Assignment<i32>| a.values()[0].unwrap(),
        bm,
        1,
        calls
    );
}

#[test]
fn list_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut n = ListConstraintNetwork::new(5, 1);
    n.add_constraint("owner", vec![ListScope::Value(0)], |v| {
        calls.set(calls.get() + 1);
        let ListFact::Value { element, .. } = &v[0] else {
            unreachable!()
        };
        s(element.as_ref().unwrap().position.entity as i64)
    })
    .unwrap();
    exercise!(
        ListProblem<i32>,
        ListAssignment<i32>,
        ListMove<i32>,
        ListScoringError,
        lists(),
        &n,
        solve_list_with_extensions,
        SolverSequence,
        SolverPhase::LocalSearch,
        SolverPhase::Construction(ConstructionConfig::default()),
        SolverPhase::Custom(CustomPhaseConfig::default()),
        |a: &ListAssignment<i32>| a.location(&7).unwrap().entity as i32,
        lm,
        1,
        calls
    );
}

#[test]
fn list_stream_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut graph = || {
        calls.set(calls.get() + 1);
        let mut b = StreamGraphBuilder::new();
        let source = ListStreamSources::<i32>::new(&mut b);
        b.score("owner", source.values.port(), |v| {
            s(v.position.unwrap().entity as i64)
        })?;
        Ok((b.build(), source))
    };
    exercise!(
        ListProblem<i32>,
        ListAssignment<i32>,
        ListMove<i32>,
        ListStreamScoringError,
        lists(),
        &mut graph,
        solve_list_stream_with_extensions,
        SolverSequence,
        SolverPhase::LocalSearch,
        SolverPhase::Construction(ConstructionConfig::default()),
        SolverPhase::Custom(CustomPhaseConfig::default()),
        |a: &ListAssignment<i32>| a.location(&7).unwrap().entity as i32,
        lm,
        1,
        calls
    );
}

#[test]
fn mixed_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut n = MixedConstraintNetwork::new(1, 5, 1);
    n.add_constraint(
        "both",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |v| {
            calls.set(calls.get() + 1);
            let (MixedFact::Basic(value), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
            else {
                unreachable!()
            };
            s(i64::from(*value.unwrap()) + element.as_ref().unwrap().position.entity as i64)
        },
    )
    .unwrap();
    exercise!(MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove<i32,i32>,MixedScoringError,MixedProblem::new(basic(),lists()),&n,solve_mixed_with_extensions,MixedSolverSequence,|search|MixedSolverPhase::LocalSearch(MixedLocalSearchConfig{search,joint_moves:false}),MixedSolverPhase::Construction(MixedConstructionConfig::default()),MixedSolverPhase::Custom(CustomPhaseConfig::default()),|a:&MixedAssignment<i32,i32>|a.basic().values()[0].unwrap(),mm,2,calls);
}

#[test]
fn mixed_stream_provider_plans() {
    let calls = Rc::new(Cell::new(0));
    let mut graph = || {
        calls.set(calls.get() + 1);
        let mut b = StreamGraphBuilder::new();
        let source = MixedStreamSources::<i32, i32>::new(&mut b);
        b.score("value", source.basic.port(), |(_, v)| {
            s(i64::from(v.unwrap()))
        })?;
        b.score("owner", source.lists.values.port(), |v| {
            s(v.position.unwrap().entity as i64)
        })?;
        Ok((b.build(), source))
    };
    exercise!(MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove<i32,i32>,MixedStreamScoringError,MixedProblem::new(basic(),lists()),&mut graph,solve_mixed_stream_with_extensions,MixedSolverSequence,|search|MixedSolverPhase::LocalSearch(MixedLocalSearchConfig{search,joint_moves:false}),MixedSolverPhase::Construction(MixedConstructionConfig::default()),MixedSolverPhase::Custom(CustomPhaseConfig::default()),|a:&MixedAssignment<i32,i32>|a.basic().values()[0].unwrap(),mm,2,calls);
}
