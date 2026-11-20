//! Preserve the pick order of whole objects and temporary portions.
use std::collections::BTreeSet;
use viboceros_document::ObjectId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum OrderedInput {
    Object(ObjectId),
    Range(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::app) struct InputOrder {
    inputs: Vec<OrderedInput>,
    wholes: BTreeSet<ObjectId>,
}

impl InputOrder {
    pub(in crate::app) fn sync(&mut self, selected: impl IntoIterator<Item = ObjectId>) {
        let selected = selected.into_iter().collect::<Vec<_>>();
        let present = selected.iter().copied().collect::<BTreeSet<_>>();
        if present == self.wholes {
            return;
        }
        // Only removals need to scan the accumulated portion inputs.
        if self.wholes.difference(&present).next().is_some() {
            self.inputs.retain(|input| match input {
                OrderedInput::Object(id) => present.contains(id),
                OrderedInput::Range(_) => true,
            });
            self.wholes.retain(|id| present.contains(id));
        }
        for id in selected {
            if self.wholes.insert(id) {
                self.inputs.push(OrderedInput::Object(id));
            }
        }
    }

    pub(in crate::app) fn push_range(
        &mut self,
        index: usize,
        selected: impl IntoIterator<Item = ObjectId>,
    ) {
        self.sync(selected);
        self.inputs.push(OrderedInput::Range(index));
    }

    pub(in crate::app) fn ordered(&self) -> impl Iterator<Item = OrderedInput> + '_ {
        self.inputs.iter().copied()
    }
}
