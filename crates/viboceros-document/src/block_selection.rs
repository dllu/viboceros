//! Direct block-definition selection does not expand grouped peers or nested members.
use super::*;

impl Document {
    /// Add selectable instances matching a definition-name wildcard without
    /// expanding groups, traversing nested members, or recording model history.
    pub fn select_block_instances_by_name_pattern(&mut self, pattern: &str) -> usize {
        let pattern = CaseInsensitiveWildcard::new(pattern);
        let definitions = self
            .block_definitions()
            .filter(|definition| pattern.matches(definition.name()))
            .map(BlockDefinition::id)
            .collect::<BTreeSet<_>>();
        let matches = self
            .selectable_objects()
            .filter_map(|object| match object.geometry() {
                Geometry::BlockInstance(instance)
                    if definitions.contains(&instance.reference().definition()) =>
                {
                    Some(object.id())
                }
                _ => None,
            })
            .collect();
        self.apply_selection_mode(matches, SelectionMode::Add)
    }
}
