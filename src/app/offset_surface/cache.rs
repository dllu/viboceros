//! Two prior valid previews; prepared candidates still validate their sources.
use super::*;

pub(super) type Key = (Options, BTreeSet<ObjectId>);

pub(super) fn key(options: Options, flips: &BTreeSet<ObjectId>) -> Key {
    (
        Options {
            delete_input: false,
            ..options
        },
        flips.clone(),
    )
}

#[derive(Default)]
pub(super) struct Cache(Vec<(Key, Prepared)>);

impl Cache {
    pub(super) fn insert(&mut self, key: Key, prepared: Prepared) {
        self.0.retain(|(k, _)| k != &key);
        self.0.push((key, prepared));
        if self.0.len() > 2 {
            self.0.remove(0);
        }
    }
    pub(super) fn take(&mut self, key: &Key, document: &Document) -> Option<Prepared> {
        self.0.retain(|(_, p)| p.is_current(document));
        let index = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(index).1)
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
}
