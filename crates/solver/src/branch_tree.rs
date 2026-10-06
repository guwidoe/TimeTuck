//! Private finite trees shared by bounded search; children are cloned only on visit.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListPosition, ListProblem, MixedAssignment,
    MixedProblem,
};

pub(crate) trait BranchTree {
    type Assignment;
    type Choice;
    fn depth(&self) -> usize;
    fn choices(
        &self,
        assignment: &Self::Assignment,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice>;
    fn child(
        &self,
        assignment: &Self::Assignment,
        depth: usize,
        choice: Self::Choice,
    ) -> Self::Assignment;
}

pub(crate) struct BasicTree<'a, V> {
    problem: &'a FiniteProblem<V>,
    remaining: Vec<usize>,
}
impl<'a, V: Eq + Clone> BasicTree<'a, V> {
    pub(crate) fn new(problem: &'a FiniteProblem<V>) -> (Self, Option<Assignment<V>>) {
        let mut tree = Self {
            problem,
            remaining: Vec::new(),
        };
        if problem.search_space_size() == Some(0) {
            return (tree, None);
        }
        tree.remaining = problem
            .variables()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (!v.is_pinned()).then_some(i))
            .collect();
        let mut root = problem.initial_assignment();
        for &i in &tree.remaining {
            root.values[i] = None;
        }
        (tree, Some(root))
    }
    pub(crate) fn remaining(&self) -> &[usize] {
        &self.remaining
    }
}
impl<V: Eq + Clone> BranchTree for BasicTree<'_, V> {
    type Assignment = Assignment<V>;
    type Choice = usize;
    fn depth(&self) -> usize {
        self.remaining.len()
    }
    fn choices(&self, _: &Assignment<V>, depth: usize) -> impl ExactSizeIterator<Item = usize> {
        let v = &self.problem.variables()[self.remaining[depth]];
        0..v.domain().len() + usize::from(v.allows_unassigned())
    }
    fn child(&self, a: &Assignment<V>, depth: usize, choice: usize) -> Assignment<V> {
        let variable = self.remaining[depth];
        let mut child = a.clone();
        child.values[variable] = self.problem.variables()[variable]
            .domain()
            .get(choice)
            .cloned();
        child
    }
}

pub(crate) struct ListTree<'a, V> {
    problem: &'a ListProblem<V>,
    remaining: Vec<usize>,
}
impl<'a, V: Eq + Clone> ListTree<'a, V> {
    pub(crate) fn new(problem: &'a ListProblem<V>) -> (Self, Option<ListAssignment<V>>) {
        let root = ListAssignment::new(
            problem
                .entities()
                .iter()
                .map(|e| {
                    if e.is_pinned() {
                        e.initial().to_vec()
                    } else {
                        e.initial()[..e.pinned_prefix()].to_vec()
                    }
                })
                .collect(),
        );
        let mut tree = Self {
            problem,
            remaining: Vec::new(),
        };
        if !problem.can_complete(&root)
            || (problem.dependencies.is_some() && problem.validate(&root).is_err())
        {
            return (tree, None);
        }
        tree.remaining = problem
            .values()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| root.location(v).is_none().then_some(i))
            .collect();
        (tree, Some(root))
    }
    pub(crate) fn remaining(&self) -> &[usize] {
        &self.remaining
    }
}
impl<V: Eq + Clone> BranchTree for ListTree<'_, V> {
    type Assignment = ListAssignment<V>;
    type Choice = Option<ListPosition>;
    fn depth(&self) -> usize {
        self.remaining.len()
    }
    fn choices(
        &self,
        a: &ListAssignment<V>,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice> {
        let value = &self.problem.values()[self.remaining[depth]];
        let mut choices = Vec::new();
        for (entity, e) in self.problem.entities().iter().enumerate() {
            if !e.is_pinned() && self.problem.accepts(entity, value) {
                for index in e.pinned_prefix()..=a.lists()[entity].len() {
                    choices.push(Some(ListPosition { entity, index }));
                }
            }
        }
        if self.problem.allows_unassigned() {
            choices.push(None);
        }
        if self.problem.dependencies.is_some() {
            choices.retain(|&choice| self.problem.validate(&self.child(a, depth, choice)).is_ok());
        }
        choices.into_iter()
    }
    fn child(
        &self,
        a: &ListAssignment<V>,
        depth: usize,
        choice: Self::Choice,
    ) -> ListAssignment<V> {
        let mut child = a.clone();
        if let Some(position) = choice {
            child.lists[position.entity].insert(
                position.index,
                self.problem.values()[self.remaining[depth]].clone(),
            );
        }
        child
    }
}

pub(crate) struct MixedTree<'a, B, L> {
    basic: BasicTree<'a, B>,
    lists: ListTree<'a, L>,
}
pub(crate) enum MixedChoice {
    Basic(usize),
    List(Option<ListPosition>),
}
impl<'a, B: Eq + Clone, L: Eq + Clone> MixedTree<'a, B, L> {
    pub(crate) fn new(p: &'a MixedProblem<B, L>) -> (Self, Option<MixedAssignment<B, L>>) {
        let (basic, b) = BasicTree::new(p.basic());
        let (lists, l) = ListTree::new(p.lists());
        (
            Self { basic, lists },
            b.zip(l).map(|(b, l)| MixedAssignment::new(b, l)),
        )
    }
    pub(crate) fn remaining(&self, depth: usize) -> (&[usize], &[usize]) {
        if depth < self.lists.depth() {
            (self.basic.remaining(), &self.lists.remaining()[depth..])
        } else {
            (&self.basic.remaining()[depth - self.lists.depth()..], &[])
        }
    }
}
impl<B: Eq + Clone, L: Eq + Clone> BranchTree for MixedTree<'_, B, L> {
    type Assignment = MixedAssignment<B, L>;
    type Choice = MixedChoice;
    fn depth(&self) -> usize {
        self.lists.depth() + self.basic.depth()
    }
    fn choices(
        &self,
        a: &Self::Assignment,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice> {
        let choices: Vec<_> = if depth < self.lists.depth() {
            self.lists
                .choices(a.lists(), depth)
                .map(MixedChoice::List)
                .collect()
        } else {
            self.basic
                .choices(a.basic(), depth - self.lists.depth())
                .map(MixedChoice::Basic)
                .collect()
        };
        choices.into_iter()
    }
    fn child(&self, a: &Self::Assignment, depth: usize, choice: Self::Choice) -> Self::Assignment {
        match choice {
            MixedChoice::List(c) => {
                MixedAssignment::new(a.basic().clone(), self.lists.child(a.lists(), depth, c))
            }
            MixedChoice::Basic(c) => MixedAssignment::new(
                self.basic.child(a.basic(), depth - self.lists.depth(), c),
                a.lists().clone(),
            ),
        }
    }
}
