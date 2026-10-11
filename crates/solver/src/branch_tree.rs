//! Private finite trees shared by bounded search; children are cloned only on visit.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListPosition, ListProblem, MixedAssignment,
    MixedProblem,
};

pub(crate) trait BranchTree {
    type Assignment;
    type Choice;
    fn depth(&self) -> usize;
    fn choices<'b>(
        &'b self,
        assignment: &'b Self::Assignment,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice> + 'b;
    fn child(
        &self,
        assignment: &Self::Assignment,
        depth: usize,
        choice: Self::Choice,
    ) -> Self::Assignment;
    /// Consume an expanded parent after its final choice. Implementations must
    /// produce the same logical child as `child` without duplicating the parent.
    fn child_owned(
        &self,
        assignment: Self::Assignment,
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
    /// The caller has validated the phase handoff. Assigned representatives are
    /// preserved verbatim, including payloads whose equality is identity-only.
    pub(crate) fn for_phase(
        problem: &'a FiniteProblem<V>,
        initial: Option<&Assignment<V>>,
    ) -> (Self, Option<Assignment<V>>) {
        let Some(initial) = initial else {
            return Self::new(problem);
        };
        if problem.search_space_size() == Some(0) {
            return (
                Self {
                    problem,
                    remaining: Vec::new(),
                },
                None,
            );
        }
        let remaining = problem
            .variables()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (!v.is_pinned() && initial.values()[i].is_none()).then_some(i))
            .collect();
        (Self { problem, remaining }, Some(initial.clone()))
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
    fn choices<'b>(
        &'b self,
        _: &'b Assignment<V>,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = usize> + 'b {
        let v = &self.problem.variables()[self.remaining[depth]];
        0..v.domain().len() + usize::from(v.allows_unassigned())
    }
    fn child(&self, a: &Assignment<V>, depth: usize, choice: usize) -> Assignment<V> {
        self.child_owned(a.clone(), depth, choice)
    }
    fn child_owned(&self, mut child: Assignment<V>, depth: usize, choice: usize) -> Assignment<V> {
        let variable = self.remaining[depth];
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
        Self::from_root(problem, root)
    }
    pub(crate) fn for_phase(
        problem: &'a ListProblem<V>,
        initial: Option<&ListAssignment<V>>,
    ) -> (Self, Option<ListAssignment<V>>) {
        match initial {
            Some(initial) => Self::from_root(problem, initial.clone()),
            None => Self::new(problem),
        }
    }
    fn from_root(
        problem: &'a ListProblem<V>,
        root: ListAssignment<V>,
    ) -> (Self, Option<ListAssignment<V>>) {
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
    fn choices<'b>(
        &'b self,
        a: &'b ListAssignment<V>,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice> + 'b {
        let value = &self.problem.values()[self.remaining[depth]];
        let choices = InsertionChoices::new(self.problem, a, value);
        if let Some(graph) = &self.problem.dependencies {
            // Parents are validated and choices already enforce ranges and pins.
            // Resolve stable identities once; testing insertion topology requires
            // no owned child, planning-value clones, or repeated domain checks.
            let rows: Vec<Vec<usize>> = a
                .lists()
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|value| {
                            self.problem
                                .value_index(value)
                                .expect("registered exact-search value")
                        })
                        .collect()
                })
                .collect();
            let incoming = self.remaining[depth];
            let mut edges = Vec::with_capacity(rows.iter().map(Vec::len).sum::<usize>() + 1);
            let valid: Vec<_> = choices
                .filter(|position| {
                    let Some(position) = position else {
                        return true;
                    };
                    edges.clear();
                    for (entity, row) in rows.iter().enumerate() {
                        let split = if entity == position.entity {
                            position.index
                        } else {
                            row.len()
                        };
                        let inserted = (entity == position.entity).then_some(incoming);
                        let chain = row[..split]
                            .iter()
                            .copied()
                            .chain(inserted)
                            .chain(row[split..].iter().copied());
                        let mut previous = None;
                        for id in chain {
                            if let Some(parent) = previous {
                                edges.push((id, parent));
                            }
                            previous = Some(id);
                        }
                    }
                    // Fixed edges are acyclic, and every dynamic edge joins assigned
                    // values. Therefore any new cycle contains an assigned value and
                    // is exactly a structural failure under ListProblem::validate.
                    graph
                        .evaluate(&edges, |_, _| Some(()))
                        .expect("known registry IDs")
                        .values()
                        .all(|v| !v.inconsistent)
                })
                .collect();
            ChoiceIter::Filtered(valid.into_iter())
        } else {
            ChoiceIter::Direct(choices)
        }
    }
    fn child(
        &self,
        a: &ListAssignment<V>,
        depth: usize,
        choice: Self::Choice,
    ) -> ListAssignment<V> {
        self.child_owned(a.clone(), depth, choice)
    }
    fn child_owned(
        &self,
        mut child: ListAssignment<V>,
        depth: usize,
        choice: Self::Choice,
    ) -> ListAssignment<V> {
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
    pub(crate) fn for_phase(
        problem: &'a MixedProblem<B, L>,
        initial: Option<&MixedAssignment<B, L>>,
    ) -> (Self, Option<MixedAssignment<B, L>>) {
        let Some(initial) = initial else {
            return Self::new(problem);
        };
        let (basic, b) = BasicTree::for_phase(problem.basic(), Some(initial.basic()));
        let (lists, l) = ListTree::for_phase(problem.lists(), Some(initial.lists()));
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
    fn choices<'b>(
        &'b self,
        a: &'b Self::Assignment,
        depth: usize,
    ) -> impl ExactSizeIterator<Item = Self::Choice> + 'b {
        if depth < self.lists.depth() {
            ChoiceIter::Direct(self.lists.choices(a.lists(), depth).map(MixedChoice::List))
        } else {
            ChoiceIter::Filtered(
                self.basic
                    .choices(a.basic(), depth - self.lists.depth())
                    .map(MixedChoice::Basic),
            )
        }
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
    fn child_owned(
        &self,
        a: Self::Assignment,
        depth: usize,
        choice: Self::Choice,
    ) -> Self::Assignment {
        let (basic, lists) = a.into_parts();
        match choice {
            MixedChoice::List(c) => {
                MixedAssignment::new(basic, self.lists.child_owned(lists, depth, c))
            }
            MixedChoice::Basic(c) => MixedAssignment::new(
                self.basic.child_owned(basic, depth - self.lists.depth(), c),
                lists,
            ),
        }
    }
}

// Two exact-size iterator representations; no allocation or dynamic dispatch is
// needed merely to choose between list/basic branches or direct/filtered spans.
enum ChoiceIter<A, B> {
    Direct(A),
    Filtered(B),
}
impl<T, A: Iterator<Item = T>, B: Iterator<Item = T>> Iterator for ChoiceIter<A, B> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self {
            Self::Direct(a) => a.next(),
            Self::Filtered(b) => b.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Direct(a) => a.size_hint(),
            Self::Filtered(b) => b.size_hint(),
        }
    }
}
impl<T, A: ExactSizeIterator<Item = T>, B: ExactSizeIterator<Item = T>> ExactSizeIterator
    for ChoiceIter<A, B>
{
}

// The number of insertion slots is known without storing each coordinate. Visit
// eligible rows and positions in exactly the old entity/index order, then omission.
struct InsertionChoices<'a, V> {
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    value: &'a V,
    entity: usize,
    index: Option<usize>,
    optional: bool,
    remaining: usize,
}
impl<'a, V: Eq + Clone> InsertionChoices<'a, V> {
    fn new(problem: &'a ListProblem<V>, assignment: &'a ListAssignment<V>, value: &'a V) -> Self {
        let optional = problem.allows_unassigned();
        let remaining = problem
            .entities()
            .iter()
            .enumerate()
            .filter(|(entity, e)| !e.is_pinned() && problem.accepts(*entity, value))
            .map(|(entity, e)| assignment.lists()[entity].len() - e.pinned_prefix() + 1)
            .sum::<usize>()
            + usize::from(optional);
        Self {
            problem,
            assignment,
            value,
            entity: 0,
            index: None,
            optional,
            remaining,
        }
    }
}
impl<V: Eq + Clone> Iterator for InsertionChoices<'_, V> {
    type Item = Option<ListPosition>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        while let Some(meta) = self.problem.entities().get(self.entity) {
            if self.index.is_none() {
                if meta.is_pinned() || !self.problem.accepts(self.entity, self.value) {
                    self.entity += 1;
                    continue;
                }
                self.index = Some(meta.pinned_prefix());
            }
            let index = self.index.unwrap();
            let position = ListPosition {
                entity: self.entity,
                index,
            };
            if index == self.assignment.lists()[self.entity].len() {
                self.entity += 1;
                self.index = None;
            } else {
                self.index = Some(index + 1);
            }
            self.remaining -= 1;
            return Some(Some(position));
        }
        if self.optional {
            self.optional = false;
            self.remaining -= 1;
            Some(None)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<V: Eq + Clone> ExactSizeIterator for InsertionChoices<'_, V> {}

#[cfg(test)]
mod choice_tests {
    use super::*;
    use crate::ListEntity;
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Value(usize, Rc<Cell<usize>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for Value {
        fn eq(&self, b: &Self) -> bool {
            self.0 == b.0
        }
    }
    impl Eq for Value {}
    #[test]
    fn dependency_choices_match_owned_children_without_cloning_parent_payloads() {
        let clones = Rc::new(Cell::new(0));
        let v = |id| Value(id, clones.clone());
        let p = ListProblem::new(
            (0..10).map(v).collect(),
            vec![
                ListEntity::new((0..8).map(v).collect()),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap()
        .allowing_unassigned()
        .with_dependencies(
            (0..10)
                .map(|i| if i == 0 { vec![] } else { vec![i - 1] })
                .collect(),
        )
        .unwrap();
        let (tree, _) = ListTree::new(&p);
        let a = p.initial_assignment();
        let proposals: Vec<_> = InsertionChoices::new(&p, &a, &p.values()[8]).collect();
        let expected: Vec<_> = proposals
            .iter()
            .copied()
            .filter(|choice| p.validate(&tree.child(&a, 8, *choice)).is_ok())
            .collect();
        assert!(expected.len() < proposals.len());
        clones.set(0);
        let actual: Vec<_> = tree.choices(&a, 8).collect();
        assert_eq!(actual, expected);
        assert_eq!(
            clones.get(),
            0,
            "dependency filtering only inspects integer value identities"
        );
    }
    #[test]
    fn direct_choices_keep_exact_remaining_lengths_and_order_with_pins_and_ranges() {
        for optional in [false, true] {
            let mut p = ListProblem::new(
                vec![0, 1, 2, 3],
                vec![
                    ListEntity::new(vec![0]).with_pinned(true),
                    ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
                    ListEntity::new(vec![]).with_value_range(vec![0, 1, 2]),
                    ListEntity::new(vec![]),
                ],
            )
            .unwrap();
            if optional {
                p = p.allowing_unassigned();
            }
            let a = p.initial_assignment();
            let expected = vec![
                Some(ListPosition {
                    entity: 1,
                    index: 1,
                }),
                Some(ListPosition {
                    entity: 1,
                    index: 2,
                }),
                Some(ListPosition {
                    entity: 3,
                    index: 0,
                }),
            ];
            let mut expected = expected;
            if optional {
                expected.push(None);
            }
            let mut cursor = InsertionChoices::new(&p, &a, &3);
            for (i, item) in expected.iter().enumerate() {
                assert_eq!(cursor.len(), expected.len() - i);
                assert_eq!(
                    cursor.size_hint(),
                    (expected.len() - i, Some(expected.len() - i))
                );
                assert_eq!(cursor.next(), Some(*item));
            }
            assert_eq!(cursor.len(), 0);
            assert_eq!(cursor.next(), None);
            assert_eq!(cursor.next(), None);
        }
    }
}
