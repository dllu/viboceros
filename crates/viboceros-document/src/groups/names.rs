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
                .used_group_names
                .iter()
                .cloned()
                .chain(
                    document
                        .groups
                        .iter()
                        .filter_map(|group| group.name.clone()),
                )
                .map(|name| name.to_ascii_lowercase())
                .collect()
        });
        while let Some(number) = self.last_number.checked_add(1) {
            self.last_number = number;
            let candidate = format!("Group{number:02}");
            if used.insert(candidate.to_ascii_lowercase()) {
                return candidate;
            }
        }
        loop {
            let candidate = format!("Group{}", GroupId::new());
            if used.insert(candidate.to_ascii_lowercase()) {
                return candidate;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_names_reserve_case_variants_and_fill_unused_holes() {
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
                .find(|name| {
                    !document
                        .used_group_names()
                        .any(|used| used.eq_ignore_ascii_case(name))
                })
                .unwrap();
            assert_eq!(document.next_unused_group_name(), expected);
            assert_eq!(names.next(&document), expected);
            document.add_empty_group(Some(expected)).unwrap();
        }
    }
}

#[cfg(test)]
mod reservation_tests {
    use super::*;
    #[test]
    fn removed_automatic_names_stay_reserved_after_undo_and_history_clear() {
        let mut doc = Document::default();
        let point = doc
            .add_geometry(Geometry::Point(
                viboceros_geometry::Point3::try_from([0.; 3]).unwrap(),
            ))
            .unwrap();
        doc.add_group(Some("Pattern".into()), [point]).unwrap();
        doc.copy_objects_with_transforms(
            [point],
            &[viboceros_geometry::AffineTransform3::identity(); 3],
        )
        .unwrap();
        let automatic = doc
            .groups()
            .filter(|g| g.name().is_some_and(|n| n.starts_with("Group")))
            .map(|g| g.id())
            .collect::<Vec<_>>();
        assert_eq!(automatic.len(), 3);
        for group in automatic {
            doc.remove_group(group).unwrap();
        }
        assert_eq!(doc.next_unused_group_name(), "Group04");
        doc.undo().unwrap();
        doc.redo().unwrap();
        doc.clear_history().unwrap();
        assert_eq!(doc.next_unused_group_name(), "Group04");
        assert_eq!(
            doc.used_group_names().collect::<Vec<_>>(),
            ["Group01", "Group02", "Group03", "Pattern"]
        );
        doc.add_empty_group(Some("Group01".into())).unwrap();
        assert_eq!(doc.next_unused_group_name(), "Group04");
    }
}
