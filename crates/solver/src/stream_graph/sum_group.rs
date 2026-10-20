//! Explicit production sum-only grouping, with optional final batch publication.
use super::*;
use crate::SumGroupNode;

struct SumGroup<'a, I, T, K> {
    node: SumGroupNode<I, K>,
    mapping: Box<GroupMapping<'a, T, K>>,
    batched: bool,
    final_inputs: super::final_inputs::FinalInputs,
}

impl<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static, S: AdditiveScore>
    Node<S> for SumGroup<'_, I, T, K>
{
    fn bind_fixed_input(&mut self, right: bool, count: usize) {
        if !right {
            self.node.bind_fixed_inputs(count);
        }
    }

    fn prefers_batch(&self) -> bool {
        self.batched
    }

    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }

    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.node.begin_checkpoint()
    }

    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.node.commit_checkpoint(saved);
    }

    fn restore(&mut self, saved: Box<dyn Any>) {
        self.node.restore(saved);
    }

    fn clear(&mut self) {
        self.node.clear();
    }

    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, T>(event);
        let mapped = TupleChange {
            id: change.id.clone(),
            value: change.value.as_ref().map(|row| (self.mapping)(row)),
        };
        Ok(erase(self.node.apply(mapped).into_iter().map(|change| {
            let key = change.id.clone();
            change.map(|sum| (key, sum))
        })))
    }

    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        if !self.batched {
            let mut output = Vec::new();
            for (side, event) in inputs {
                output.extend(<Self as Node<S>>::apply(self, *side, event, arithmetic)?);
            }
            return Ok(output);
        }
        if self.final_inputs.prepare::<I, T>(inputs) {
            let mapped = self
                .final_inputs
                .positions()
                .iter()
                .map(|&(_, position)| {
                    let change = typed::<I, T>(&inputs[position].1);
                    (
                        change.id.clone(),
                        change.value.as_ref().map(|row| (self.mapping)(row)),
                    )
                })
                .collect();
            let changes = self
                .node
                .try_apply_sorted_batch(mapped)
                .expect("sum group overflow");
            return Ok(erase(changes.into_iter().map(|change| {
                let key = change.id.clone();
                change.map(|sum| (key, sum))
            })));
        }
        let mut latest = BTreeMap::new();
        for (_, event) in inputs {
            let change = typed::<I, T>(event);
            latest.insert(change.id.clone(), change);
        }
        let mapped = latest
            .into_iter()
            .map(|(id, change)| (id, change.value.as_ref().map(|row| (self.mapping)(row))))
            .collect();
        let changes = self
            .node
            .try_apply_sorted_batch(mapped)
            .expect("sum group overflow");
        Ok(erase(changes.into_iter().map(|change| {
            let key = change.id.clone();
            change.map(|sum| (key, sum))
        })))
    }
}

impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Keyed checked i64 sum with immediate per-event publication.
    ///
    /// Captures every input ID and mapped value; zero totals retain nonempty
    /// groups. Same-key replacement checks new-old, then total+difference.
    /// Arithmetic overflow panics and an enclosing graph transaction rolls back.
    /// Output is `(key, sum)` in key order. Retractions do not invoke mapping.
    pub fn group_sum<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, i64)>, GraphError> {
        self.add_sum_group(input, mapping, false, false)
    }

    /// Checked i64 sum with one final publication per affected group in a batch.
    ///
    /// Only each input ID's final put is mapped, in sorted ID order. Retractions
    /// use captured values. Existing group representatives survive temporary
    /// emptiness; absent-to-absent groups emit nothing. Nonempty equal/zero totals
    /// still emit puts. Intermediate checked arithmetic follows final-ID order,
    /// not the original event order; same-key replacement checks its difference.
    /// This explicit boundary enables batched score-director moves. Direct
    /// transaction apply remains immediate. Pure mapping/stable keys are required.
    pub fn group_sum_batched<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, i64)>, GraphError> {
        self.add_sum_group(input, mapping, false, true)
    }

    /// Keyed modular signed-i64 sum with immediate per-event publication.
    ///
    /// Arithmetic matches IntSummary's wrapping sum; the output contains only
    /// `(key, sum)`. Membership, representative and rollback rules match group_sum.
    pub fn group_sum_wrapping<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, i64)>, GraphError> {
        self.add_sum_group(input, mapping, true, false)
    }

    /// Modular signed-i64 sum with group_sum_batched's final-publication rules.
    pub fn group_sum_wrapping_batched<
        I: Ord + Clone + 'static,
        T: Clone + 'static,
        K: Ord + Clone + 'static,
    >(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
    ) -> Result<StreamPort<K, (K, i64)>, GraphError> {
        self.add_sum_group(input, mapping, true, true)
    }

    fn add_sum_group<I: Ord + Clone + 'static, T: Clone + 'static, K: Ord + Clone + 'static>(
        &mut self,
        input: &StreamPort<I, T>,
        mapping: impl Fn(&T) -> (K, i64) + 'a,
        wrapping: bool,
        batched: bool,
    ) -> Result<StreamPort<K, (K, i64)>, GraphError> {
        self.validate(input)?;
        let out = self.push(SumGroup {
            node: if wrapping {
                SumGroupNode::<I, K>::wrapping()
            } else {
                SumGroupNode::<I, K>::new()
            },
            mapping: Box::new(mapping),
            batched,
            final_inputs: Default::default(),
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}

#[cfg(test)]
mod final_input_tests {
    use super::*;
    use crate::SimpleScore;
    use std::cell::Cell;

    #[test]
    fn ordinal_mapping_panic_precedes_state_changes_and_retry_uses_fresh_positions() {
        let fail = Cell::new(true);
        let mut group = SumGroup {
            node: SumGroupNode::<usize, usize>::new(),
            mapping: Box::new(|row: &i64| {
                assert!(!fail.get() || *row != 9, "mapping failure");
                (0usize, *row)
            }),
            batched: true,
            final_inputs: Default::default(),
        };
        let events = |rows: &[(usize, Option<i64>)]| {
            rows.iter()
                .map(|&(id, value)| (false, Rc::new(TupleChange { id, value }) as Event))
                .collect::<Vec<_>>()
        };
        let initial = events(&[(0, Some(2))]);
        <_ as Node<SimpleScore>>::apply_batch(
            &mut group,
            &initial,
            NodeArithmetic {
                impact: crate::ScoringArithmetic::Checked,
                totals: crate::ScoringArithmetic::Checked,
            },
        )
        .unwrap();
        let failed = events(&[(0, Some(7)), (usize::MAX, Some(9))]);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                <_ as Node<SimpleScore>>::apply_batch(
                    &mut group,
                    &failed,
                    NodeArithmetic {
                        impact: crate::ScoringArithmetic::Checked,
                        totals: crate::ScoringArithmetic::Checked,
                    },
                )
                .unwrap();
            }))
            .is_err()
        );
        fail.set(false);
        let retry = events(&[(usize::MAX, Some(4)), (usize::MAX, None), (19, Some(3))]);
        let output = <_ as Node<SimpleScore>>::apply_batch(
            &mut group,
            &retry,
            NodeArithmetic {
                impact: crate::ScoringArithmetic::Checked,
                totals: crate::ScoringArithmetic::Checked,
            },
        )
        .unwrap();
        assert_eq!(typed::<usize, (usize, i64)>(&output[0]).value, Some((0, 5)));
    }
}
