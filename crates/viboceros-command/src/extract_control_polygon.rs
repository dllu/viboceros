//! Control-net extraction uses native geometry conversion and source-group copies.
use crate::*;
use viboceros_geometry::SubDControlNetPriority;

mod options;
pub use options::{Options, OutputLayer};

pub(crate) fn accepts(geometry: &Geometry) -> bool {
    geometry.curve_ref().is_some()
        || matches!(geometry, Geometry::NurbsSurface(_) | Geometry::SubD(_))
        || matches!(geometry, Geometry::Brep(b) if b.faces().len() == 1)
}

pub(super) struct ExtractControlPolygonCommand;
impl Command for ExtractControlPolygonCommand {
    fn name(&self) -> &'static str {
        "ExtractControlPolygon"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(options::parse(args)?.prompt()))
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn cleanup_failed_selection(
        &self,
        document: &mut Document,
        error: &CommandError,
        _postselected: bool,
    ) {
        if matches!(
            error,
            CommandError::UnsupportedExtractControlPolygonGeometry
        ) {
            document.clear_selection();
        }
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = options::parse(args)?;
        let selected = document
            .selected_objects()
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut staged = Vec::new();
        for &id in &selected {
            let source = document.object(id).unwrap();
            if !accepts(source.geometry()) {
                continue;
            }
            let output = match source.geometry() {
                Geometry::SubD(s) => Some(Geometry::Mesh(s.control_net_mesh(options.priority)?)),
                Geometry::NurbsSurface(s) => s
                    .control_polygon_mesh(document.tolerance())?
                    .map(Geometry::Mesh),
                Geometry::Brep(b) => b.faces()[0]
                    .surface()
                    .control_polygon_mesh(document.tolerance())?
                    .map(Geometry::Mesh),
                Geometry::PolyCurve(c) => {
                    Some(Geometry::Polyline(c.control_polygon(document.tolerance())?))
                }
                Geometry::NurbsCurve(c) => {
                    Some(Geometry::Polyline(c.control_polygon(document.tolerance())?))
                }
                geometry => {
                    let curve = geometry
                        .nurbs_curve_representation()?
                        .expect("accepted curve has an exact NURBS form");
                    Some(Geometry::Polyline(
                        curve.control_polygon(document.tolerance())?,
                    ))
                }
            };
            if let Some(output) = output {
                if staged.len() == MAX_SPAN_OUTPUT_OBJECTS {
                    return Err(too_many_span_outputs(self.name()));
                }
                staged.push((id, output));
            }
        }
        if staged.is_empty() {
            return Err(
                if selected
                    .iter()
                    .all(|id| !accepts(document.object(*id).unwrap().geometry()))
                {
                    CommandError::UnsupportedExtractControlPolygonGeometry
                } else {
                    CommandError::NoExtractableControlPolygons
                },
            );
        }
        let count = staged.len();
        let extracted = staged.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
        document.release_command_selection_on_history_replay(selected.iter().copied())?;
        let outputs = document.copy_object_geometries_into_source_groups_in_order(staged)?;
        if options.output_layer == OutputLayer::Current {
            document.set_objects_layer(outputs.iter().copied(), document.current_layer_id())?;
        }
        document.release_command_selection_on_history_replay(outputs.iter().copied())?;
        document.select_command_results(
            selected
                .into_iter()
                .filter(|id| !extracted.contains(id))
                .chain(outputs),
        )?;
        Ok(format!(
            "Extracted {count} control polygon(s) from {count} object(s)"
        ))
    }
}
