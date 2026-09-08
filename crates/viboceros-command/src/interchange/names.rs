//! Import-local collision allocation; names are only added during an import.
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct ImportNames {
    used: BTreeSet<String>,
    next_suffix: BTreeMap<String, u64>,
    ascii_insensitive: bool,
    fallback: &'static str,
}

impl ImportNames {
    pub(super) fn new<'a>(
        names: impl Iterator<Item = &'a str>,
        ascii_insensitive: bool,
        fallback: &'static str,
    ) -> Self {
        let mut result = Self {
            used: BTreeSet::new(),
            next_suffix: BTreeMap::new(),
            ascii_insensitive,
            fallback,
        };
        result.used = names.map(|name| result.key(name)).collect();
        result
    }

    fn key(&self, name: &str) -> String {
        if self.ascii_insensitive {
            name.to_ascii_lowercase()
        } else {
            name.to_owned()
        }
    }

    pub(super) fn allocate(&mut self, source: &str) -> String {
        let base = if source.trim().is_empty() {
            self.fallback
        } else {
            source.trim()
        };
        let key = self.key(base);
        if self.used.insert(key.clone()) {
            return base.to_owned();
        }
        let mut suffix = *self.next_suffix.get(&key).unwrap_or(&1);
        loop {
            let candidate = format!("{base} (Imported {suffix})");
            suffix = suffix
                .checked_add(1)
                .expect("finite import cannot exhaust u64 names");
            if self.used.insert(self.key(&candidate)) {
                self.next_suffix.insert(key, suffix);
                return candidate;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_allocation_matches_independent_full_scan_policy() {
        for insensitive in [false, true] {
            let mut used = vec![
                "Part".to_owned(),
                "Part (Imported 2)".into(),
                "Imported Layer".into(),
            ];
            let mut allocator = ImportNames::new(
                used.iter().map(String::as_str),
                insensitive,
                "Imported Layer",
            );
            let sources = [
                "Part",
                "part",
                " Part ",
                "Part (Imported 1)",
                "",
                "\t",
                "Ä",
                "ä",
            ];
            for i in 0..512 {
                let source = sources[(i * 17 + i / 7) % sources.len()];
                let base = if source.trim().is_empty() {
                    "Imported Layer"
                } else {
                    source.trim()
                };
                let contains = |name: &str| {
                    used.iter().any(|existing| {
                        if insensitive {
                            existing.eq_ignore_ascii_case(name)
                        } else {
                            existing == name
                        }
                    })
                };
                let expected = if !contains(base) {
                    base.to_owned()
                } else {
                    (1..)
                        .map(|n| format!("{base} (Imported {n})"))
                        .find(|candidate| !contains(candidate))
                        .unwrap()
                };
                assert_eq!(allocator.allocate(source), expected);
                used.push(expected);
            }
        }
    }

    #[test]
    fn repeated_names_reuse_the_suffix_cursor() {
        let reserved = (1..=1000)
            .map(|i| format!("Part (Imported {i})"))
            .chain(["Part".into()])
            .collect::<Vec<_>>();
        let mut names =
            ImportNames::new(reserved.iter().map(String::as_str), true, "Imported Layer");
        for i in 1001..=2000 {
            assert_eq!(names.allocate("part"), format!("part (Imported {i})"));
            assert_eq!(names.next_suffix["part"], i + 1);
        }
        assert_eq!(names.used.len(), 2001);
    }
}
