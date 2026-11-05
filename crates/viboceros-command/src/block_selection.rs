//! Select instances by their catalog definition rather than object attributes.
use super::*;

pub(super) struct SelBlockInstanceNamedCommand;
impl Command for SelBlockInstanceNamedCommand {
    fn name(&self) -> &'static str {
        "SelBlockInstanceNamed"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn parse_arguments<'a>(&self, input: &'a str) -> Result<Vec<&'a str>, CommandError> {
        Ok(selection_commands::attribute_pattern_arguments(input))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let pattern = selection_commands::parse_attribute_pattern(
            arguments,
            "SelBlockInstanceNamed block-name",
        )?;
        let count = document.select_block_instances_by_name_pattern(&pattern);
        Ok(format!("Selected {count} object(s)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::{BlockContent, BlockMember, BlockReference};
    use viboceros_geometry::{AffineTransform3, Vector3};

    fn definition(doc: &mut Document, name: &str) -> viboceros_document::BlockDefinitionId {
        doc.add_block_definition(
            name,
            vec![BlockMember::new(
                BlockContent::Geometry(
                    Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()).into(),
                ),
                ObjectAttributes::on_layer(doc.current_layer_id()),
            )],
        )
        .unwrap()
    }
    #[test]
    fn block_names_preserve_whitespace_unicode_and_failed_input_selection() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let mut ids = Vec::new();
        for name in ["Bolt A", "Bolt  A", "部品 A"] {
            let index = definition(&mut doc, name);
            ids.push(
                doc.add_block_instance(
                    BlockReference::try_new(index, AffineTransform3::identity()).unwrap(),
                )
                .unwrap(),
            );
        }
        let original = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Point 1,2,3").unwrap();
        registry.execute(&mut doc, "Undo").unwrap();
        for (name, expected) in [
            ("Bolt  A", vec![ids[1]]),
            ("部品 A", vec![ids[2]]),
            ("bolt*", ids[..2].to_vec()),
        ] {
            doc.clear_selection();
            registry
                .execute(&mut doc, &format!("_-SelBlockInstanceNamed \"{name}\""))
                .unwrap();
            assert_eq!(
                doc.selected_object_ids().collect::<BTreeSet<_>>(),
                expected.into_iter().collect()
            );
        }
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        for input in ["SelBlockInstanceNamed", "SelBlockInstanceNamed \"Bolt A"] {
            assert!(registry.execute(&mut doc, input).is_err());
            assert_eq!(doc.selected_object_ids().collect::<Vec<_>>(), selected);
        }
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(doc.redo_label(), Some("Point"));
    }

    #[test]
    fn mirrored_blocks_use_exact_transform_orientation_and_replace_selection() {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let index = definition(&mut doc, "Origin");
        let mut odd = Vec::new();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
            .unwrap();
        for scale in [2f64.powi(-900), 1., 2f64.powi(900)] {
            for y in [scale, -scale] {
                let t = AffineTransform3::try_new(
                    [[-scale, 0., 0.], [0., y, 0.], [0., 0., scale]],
                    Vector3::try_new(0., 0., 0.).unwrap(),
                )
                .unwrap();
                let id = doc
                    .add_block_instance(BlockReference::try_new(index, t).unwrap())
                    .unwrap();
                if y > 0. {
                    odd.push(id);
                }
            }
        }
        doc.select_object(point, SelectionMode::Replace).unwrap();
        registry.execute(&mut doc, "SelMirroredBlocks").unwrap();
        assert_eq!(
            doc.selected_object_ids().collect::<BTreeSet<_>>(),
            odd.into_iter().collect()
        );
    }
}
