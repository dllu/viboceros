use super::*;

mod parts;
mod summary;
use summary::{ExplodeSummary, PartKind};

#[cfg(test)]
mod tests;

pub(super) struct ExplodeCommand;

impl Command for ExplodeCommand {
    fn name(&self) -> &'static str {
        "Explode"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["X"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, "Explode")?;
        let locked_layers = document
            .layers()
            .filter(|layer| layer.is_locked())
            .map(|layer| layer.id())
            .collect::<BTreeSet<_>>();
        let selected = document
            .selected_objects()
            .map(|object| {
                (
                    object.id(),
                    object.geometry(),
                    object.attributes().is_visible()
                        && !object.attributes().is_locked()
                        && !locked_layers.contains(&object.attributes().layer_id()),
                )
            })
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut exploded = Vec::new();
        let mut summary = ExplodeSummary::default();
        let mut unchanged_ids = Vec::new();
        let mut deleted_sources = Vec::new();
        for (id, geometry, delete_source) in &selected {
            if let Some(count) = parts::known_output_count(geometry)? {
                summary.check_add(count)?;
            }
            let parts = parts::decompose(geometry, document.tolerance(), summary.remaining())?;
            let Some(parts) = parts else {
                unchanged_ids.push(*id);
                continue;
            };
            let (kind, count) = parts.report();
            summary.record(kind, count)?;
            if *delete_source {
                deleted_sources.push(*id);
            }
            exploded.push((*id, parts));
        }
        if exploded.is_empty() {
            return Err(CommandError::NoExplodableObjects);
        }
        let unchanged_count = unchanged_ids.len();
        let pieces = exploded.into_iter().flat_map(|(source, parts)| {
            parts
                .into_geometries()
                .into_iter()
                .map(move |geometry| (source, geometry))
        });
        // Copy while restricted sources remain selected/editable, then consume
        // source selection before recording their deletion (Rhino's Explode
        // history policy). Fresh pieces inherit attributes and ordered groups.
        let selected_result_ids = document.copy_object_pieces_into_source_groups(pieces)?;
        document.select_command_results(unchanged_ids.iter().copied())?;
        document.delete_objects(deleted_sources)?;
        // Retained restricted sources stay unselected, and overlapping groups
        // must not pull untouched peers into the output selection.
        document.select_command_results(unchanged_ids.into_iter().chain(selected_result_ids))?;
        Ok(summary.message(unchanged_count))
    }
}
