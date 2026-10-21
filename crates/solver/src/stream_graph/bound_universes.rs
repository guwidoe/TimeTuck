//! Propagate checked occurrence namespaces, independently of source storage.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Universe {
    Ordinals(usize),
    Pairs(usize, usize),
}

impl<S: AdditiveScore> StreamGraph<'_, S> {
    pub(super) fn bind_derived_universes(&mut self, source: usize, count: usize) {
        let mut pending = VecDeque::from([(source, Universe::Ordinals(count))]);
        let mut outputs = vec![None; self.nodes.len()];
        let mut join_inputs = vec![[None; 2]; self.nodes.len()];
        outputs[source] = Some(Universe::Ordinals(count));
        while let Some((parent, universe)) = pending.pop_front() {
            for edge in &self.edges[parent] {
                let node = &mut self.nodes[edge.node];
                match universe {
                    Universe::Ordinals(count) => node.bind_fixed_input(edge.right, count),
                    Universe::Pairs(left, right) => {
                        node.bind_fixed_pair_input(edge.right, left, right);
                    }
                }
                self.join_output_nodes[edge.node] = node.has_join_output();
                let output = if node.joins_input_ids() {
                    join_inputs[edge.node][usize::from(edge.right)] = Some(universe);
                    match join_inputs[edge.node] {
                        [
                            Some(Universe::Ordinals(left)),
                            Some(Universe::Ordinals(right)),
                        ] => Some(Universe::Pairs(left, right)),
                        _ => None,
                    }
                } else if !edge.right && node.preserves_input_ids() {
                    Some(universe)
                } else {
                    // Group keys, expansions and concatenations have their own
                    // namespaces. Matching Rust types do not establish bounds.
                    None
                };
                if let Some(output) = output
                    && outputs[edge.node] != Some(output)
                {
                    outputs[edge.node] = Some(output);
                    pending.push_back((edge.node, output));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Probe {
        received: Rc<RefCell<Vec<Universe>>>,
        preserves: bool,
        joins: bool,
    }
    impl Node<crate::SimpleScore> for Probe {
        fn bind_fixed_input(&mut self, _: bool, count: usize) {
            self.received.borrow_mut().push(Universe::Ordinals(count));
        }
        fn bind_fixed_pair_input(&mut self, _: bool, left: usize, right: usize) {
            self.received
                .borrow_mut()
                .push(Universe::Pairs(left, right));
        }
        fn preserves_input_ids(&self) -> bool {
            self.preserves
        }
        fn joins_input_ids(&self) -> bool {
            self.joins
        }
        fn checkpoint(&self) -> Box<dyn Any> {
            Box::new(())
        }
        fn restore(&mut self, _: Box<dyn Any>) {}
        fn clear(&mut self) {}
        fn apply(&mut self, _: bool, _: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
            Ok(Outputs::new())
        }
    }

    #[test]
    fn pair_bounds_follow_both_join_inputs_and_stop_at_new_namespaces() {
        let mut builder = StreamGraphBuilder::<crate::SimpleScore>::new();
        let source = builder.source::<usize, usize>();
        let mapped = builder.map(source.port(), |&v| v).unwrap();
        let joined = builder
            .join_equal(&mapped, &mapped, |&v| v, |&v| v)
            .unwrap();
        let filtered = builder.filter(&joined, |_| true).unwrap();
        let grouped = builder
            .group_collect(&filtered, |_| (usize::MAX, 1))
            .unwrap();
        let after = builder.map(&grouped, |v| *v).unwrap();
        let mut graph = builder.build();
        let observations: Vec<_> = (0..graph.nodes.len())
            .map(|_| Rc::new(RefCell::new(Vec::new())))
            .collect();
        for (id, received) in observations.iter().enumerate() {
            graph.nodes[id] = Box::new(Probe {
                received: received.clone(),
                preserves: id == mapped.node || id == filtered.node || id == after.node,
                joins: id == joined.node,
            });
        }
        graph.bind_derived_universes(source.0.node, 573);
        assert_eq!(
            *observations[mapped.node].borrow(),
            [Universe::Ordinals(573)]
        );
        assert_eq!(
            *observations[joined.node].borrow(),
            [Universe::Ordinals(573); 2]
        );
        assert_eq!(
            *observations[filtered.node].borrow(),
            [Universe::Pairs(573, 573)]
        );
        assert_eq!(
            *observations[grouped.node].borrow(),
            [Universe::Pairs(573, 573)]
        );
        assert!(observations[after.node].borrow().is_empty());
    }

    #[test]
    fn one_known_join_input_does_not_imply_a_cartesian_universe() {
        let mut builder = StreamGraphBuilder::<crate::SimpleScore>::new();
        let source = builder.source::<usize, usize>();
        let facts = builder.source::<usize, usize>();
        let joined = builder
            .join_equal(source.port(), facts.port(), |&v| v, |&v| v)
            .unwrap();
        let filtered = builder.filter(&joined, |_| true).unwrap();
        let mut graph = builder.build();
        let received = Rc::new(RefCell::new(Vec::new()));
        graph.nodes[filtered.node] = Box::new(Probe {
            received: received.clone(),
            preserves: true,
            joins: false,
        });
        graph.bind_derived_universes(source.0.node, 320);
        assert!(received.borrow().is_empty());
    }
}
