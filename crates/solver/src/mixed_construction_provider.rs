//! Compose component placement schedules without duplicating mixed scoring.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListMove, ListProblem, MixedAssignment,
    MixedConstructionOrder, MixedMove, MixedProblem, Move, MoveProvider,
};

#[derive(Default)]
struct ComponentState {
    steps: u64,
    exhausted: bool,
}
fn open<'a, P, A, M: 'a>(
    provider: &'a mut impl MoveProvider<P, A, M>,
    state: &mut ComponentState,
    problem: &'a P,
    assignment: &'a A,
) -> Option<Box<dyn Iterator<Item = M> + 'a>> {
    if state.exhausted {
        return None;
    }
    let mut candidates = provider.moves(problem, assignment, state.steps);
    if let Some(first) = candidates.next() {
        Some(Box::new(std::iter::once(first).chain(candidates)))
    } else {
        state.exhausted = true;
        None
    }
}

/// Run basic and list construction providers in the requested component order.
/// Each placement contains moves from one component, scored against the complete
/// mixed assignment by the existing mixed construction engine. The other component
/// receives an empty composite move. This does not combine two moves into one.
///
/// Owns the providers, which may borrow the bound problem's components. Each child
/// sees its own completed-step count, advanced only by construction completion
/// notifications. Atomic discarded scans retry the same child counter. An empty
/// or fully assigned child, or one that stops after optional rejection, yields to the next one;
/// exhausted children are not revisited until reset. Only one proposal is prefetched
/// to detect an empty child; the rest stay lazy. Callbacks are cooperative.
///
/// Use only for construction. Direct callers must deliver exactly one
/// construction_placement_completed notification per completed nonempty scan.
/// The outer moves counter is intentionally ignored: child state survives a
/// budget-limited solve resumed from its returned assignment, whose counter starts
/// at zero again. Use reset_with before an independent phase or fresh solve; the
/// callback resets child-specific caches/queues explicitly. Match the children's
/// optional policy to the mixed phase. Only the bound problem may be used.
///
/// ```
/// use timetuck::*;
/// let p = MixedProblem::new(
///     FiniteProblem::new(vec![Variable::new(vec![1, 2])])?,
///     ListProblem::new(vec![9], vec![ListEntity::new(vec![])])?);
/// let basic = SortedBasicConstruction::new(p.basic(), ConstructionPlacement::Sequential,
///     OptionalConstruction::Preserve, |a,b| a.cmp(&b), |a,b| b.cmp(a));
/// let lists = SortedListConstruction::new(p.lists(), ConstructionPlacement::ValueQueue,
///     OptionalConstruction::Preserve, |a,b| a.cmp(&b), |a,b| a.cmp(b));
/// let mut provider = MixedConstructionProvider::new(&p, basic, lists,
///     MixedConstructionOrder::BasicThenLists);
/// let n = MixedConstraintNetwork::<_,_,SimpleScore>::new(1,1,1);
/// let r = mixed_construction_from_with_provider(&p,&n,p.initial_assignment(),
///     ProviderConstructionConfig::default(),&mut provider)?;
/// assert_eq!(r.assignment.basic().values(), &[Some(2)]);
/// assert_eq!(r.assignment.lists().lists(), &[vec![9]]);
/// provider.reset_with(|basic,lists| { basic.reset(); lists.reset(); });
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct MixedConstructionProvider<'p, B, L, BP, LP> {
    problem: &'p MixedProblem<B, L>,
    basic: BP,
    lists: LP,
    order: MixedConstructionOrder,
    basic_state: ComponentState,
    list_state: ComponentState,
    active: Option<bool>, // true = basic; only a nonempty scan can receive a notification.
}
impl<'p, B, L, BP, LP> MixedConstructionProvider<'p, B, L, BP, LP> {
    pub fn new(
        problem: &'p MixedProblem<B, L>,
        basic: BP,
        lists: LP,
        order: MixedConstructionOrder,
    ) -> Self {
        Self {
            problem,
            basic,
            lists,
            order,
            basic_state: ComponentState::default(),
            list_state: ComponentState::default(),
            active: None,
        }
    }
    /// Completed nonempty scans, in [basic, list] order, across resumed calls.
    pub fn component_steps(&self) -> [u64; 2] {
        [self.basic_state.steps, self.list_state.steps]
    }
    /// Reset child providers and then scheduling state. The caller knows each
    /// child's reset API (stateless callbacks need no reset). If this callback
    /// panics, scheduling is unchanged; its effects on children are not undone.
    pub fn reset_with(&mut self, reset: impl FnOnce(&mut BP, &mut LP)) {
        reset(&mut self.basic, &mut self.lists);
        self.basic_state = ComponentState::default();
        self.list_state = ComponentState::default();
        self.active = None;
    }
}
impl<B: Eq + Clone, L: Eq + Clone, BP, LP>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for MixedConstructionProvider<'_, B, L, BP, LP>
where
    BP: MoveProvider<FiniteProblem<B>, Assignment<B>, Move<B>>,
    LP: MoveProvider<ListProblem<L>, ListAssignment<L>, ListMove<L>>,
{
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        let Some(basic) = self.active.take() else {
            return false;
        };
        let (state, continue_child) = if basic {
            (
                &mut self.basic_state,
                self.basic.construction_placement_completed(installed),
            )
        } else {
            (
                &mut self.list_state,
                self.lists.construction_placement_completed(installed),
            )
        };
        state.steps = state.steps.saturating_add(1);
        if !installed && !continue_child {
            state.exhausted = true;
        }
        !self.basic_state.exhausted || !self.list_state.exhausted
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.problem),
            "mixed construction provider used with a different problem"
        );
        self.active = None;
        // No component construction move can add an assignment in either case.
        // Avoid invoking step factories after their last possible placement.
        self.basic_state.exhausted |= a.basic().values().iter().all(Option::is_some);
        self.list_state.exhausted |=
            a.lists().lists().iter().map(Vec::len).sum::<usize>() == p.lists().values().len();
        macro_rules! basic {
            () => {
                if let Some(moves) =
                    open(&mut self.basic, &mut self.basic_state, p.basic(), a.basic())
                {
                    self.active = Some(true);
                    return Box::new(moves.map(|basic| MixedMove {
                        basic,
                        lists: ListMove::Composite(vec![]),
                    }));
                }
            };
        }
        macro_rules! lists {
            () => {
                if let Some(moves) =
                    open(&mut self.lists, &mut self.list_state, p.lists(), a.lists())
                {
                    self.active = Some(false);
                    return Box::new(moves.map(|lists| MixedMove {
                        basic: Move::Composite(vec![]),
                        lists,
                    }));
                }
            };
        }
        match self.order {
            MixedConstructionOrder::BasicThenLists => {
                basic!();
                lists!();
            }
            MixedConstructionOrder::ListsThenBasic => {
                lists!();
                basic!();
            }
        }
        Box::new(std::iter::empty())
    }
}
