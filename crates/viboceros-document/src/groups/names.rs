use super::*;

/// Ephemeral allocator for one uninterrupted sequence of group insertions.
/// Initialize lazily so ungrouped copies do not scan or clone group names.
#[derive(Default)]
pub(crate) struct GroupNames {
    used: Option<BTreeSet<String>>,
    last_number: u64,
}

impl GroupNames {
    pub fn next(&mut self, document: &Document) -> String {
        let used = self.used.get_or_insert_with(|| {
            document
                .groups
                .iter()
                .filter_map(|group| group.name.clone())
                .collect()
        });
        while let Some(number) = self.last_number.checked_add(1) {
            self.last_number = number;
            let candidate = format!("Group{number:02}");
            if used.insert(candidate.clone()) {
                return candidate;
            }
        }
        loop {
            let candidate = format!("Group{}", GroupId::new());
            if used.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_names_match_first_unused_live_name_with_holes_and_case_variants() {
        let mut document = Document::default();
        for name in [
            "Group01", "Group03", "Group05", "group02", "Group2", "Group002", "Other",
        ] {
            document.add_empty_group(Some(name.into())).unwrap();
        }
        document.add_empty_group(None).unwrap();
        let mut names = GroupNames::default();
        for _ in 0..128 {
            let expected = (1_u64..)
                .map(|number| format!("Group{number:02}"))
                .find(|name| document.group_by_name(name).is_none())
                .unwrap();
            assert_eq!(document.next_unused_group_name(), expected);
            assert_eq!(names.next(&document), expected);
            document.add_empty_group(Some(expected)).unwrap();
        }
    }
}
