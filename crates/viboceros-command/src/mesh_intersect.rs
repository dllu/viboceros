//! Non-destructive mesh intersection curves with shared tolerance preferences.
use super::mesh_boolean::Options;
use super::*;
use std::{borrow::Cow, sync::Arc};
#[cfg(feature = "native-smlib")]
mod geometry;
const USAGE: &str = "MeshIntersect [Tolerance=Auto|Document|Zero] [CoplanarFaces=Drop|Mesh|Polylines]; select at least two meshes, surfaces or polysurfaces";

pub(super) struct MeshIntersectCommand {
    options: Arc<remembered::Remembered<Options>>,
}
impl MeshIntersectCommand {
    pub(super) fn new(options: Arc<remembered::Remembered<Options>>) -> Self {
        Self { options }
    }
    fn parse(&self, args: &[&str]) -> Result<Options, CommandError> {
        let mut result = self.options.get();
        let (mut tolerance, mut coplanar, mut i) = (false, false, 0);
        while i < args.len() {
            let (name, value, consumed) = orient_option(args, i, USAGE)?;
            if option_name_eq(name, "Tolerance") && !tolerance {
                result.tolerance = ["Auto", "Document", "Zero"]
                    .into_iter()
                    .find(|v| option_name_eq(value, v))
                    .ok_or(CommandError::Usage(USAGE))?;
                tolerance = true;
            } else if option_name_eq(name, "CoplanarFaces") && !coplanar {
                result.coplanar = ["Drop", "Mesh", "Polylines"]
                    .into_iter()
                    .find(|v| option_name_eq(value, v))
                    .ok_or(CommandError::Usage(USAGE))?;
                coplanar = true;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
            i += consumed;
        }
        Ok(result)
    }
    fn execute(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let options = self.parse(args)?;
        self.options.set(options);
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        let objects = if post {
            doc.selected_objects().collect::<Vec<_>>()
        } else {
            doc.objects().filter(|o| doc.is_selected(o.id())).collect()
        };
        let meshes = objects
            .into_iter()
            .filter_map(|object| {
                Some(match object.geometry() {
                    Geometry::Mesh(m) => Ok(Cow::Borrowed(m)),
                    Geometry::NurbsSurface(s) => s
                        .tessellate(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())
                        .map(Cow::Owned),
                    Geometry::Brep(b) => b
                        .display_mesh(interface::SURFACE_DISPLAY_SAMPLES_PER_SPAN, doc.tolerance())
                        .map(Cow::Owned),
                    _ => return None,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if meshes.len() < 2 || meshes.len() > 128 {
            return Err(CommandError::Usage(USAGE));
        }
        let curves = compute(
            &meshes.iter().map(Cow::as_ref).collect::<Vec<_>>(),
            options,
            doc.tolerance(),
        )?;
        drop(meshes);
        doc.release_command_selection_on_history_replay(selected)?;
        let count = curves.len();
        let ids = curves
            .into_iter()
            .map(|p| doc.add_geometry(Geometry::Polyline(p)))
            .collect::<Result<Vec<_>, _>>()?;
        doc.release_command_selection_on_history_replay(ids.iter().copied())?;
        doc.select_objects_direct(ids, SelectionMode::Add)?;
        Ok(format!("Created {count} mesh intersection(s)"))
    }
}
impl Command for MeshIntersectCommand {
    fn name(&self) -> &'static str {
        "MeshIntersect"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = self.parse(args)?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::EdgeAnalysis,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![
                ChoiceSelectionOption {
                    name: "Tolerance",
                    value: options.tolerance,
                    choices: &["Auto", "Document", "Zero"],
                    toggle: None,
                },
                ChoiceSelectionOption {
                    name: "CoplanarFaces",
                    value: options.coplanar,
                    choices: &["Drop", "Mesh", "Polylines"],
                    toggle: None,
                },
            ],
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        self.options.set(self.parse(args)?);
        Ok(())
    }
    fn cancel_empty_object_selection(&self) -> bool {
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.execute(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.execute(doc, args, true)
    }
}
#[cfg(not(feature = "native-smlib"))]
fn compute(
    _: &[&TriangleMesh],
    _: Options,
    _: Tolerance,
) -> Result<Vec<viboceros_geometry::Polyline3>, CommandError> {
    Err(CommandError::MeshIntersectRequiresNativeKernel)
}
#[cfg(feature = "native-smlib")]
fn compute(
    meshes: &[&TriangleMesh],
    options: Options,
    tolerance: Tolerance,
) -> Result<Vec<viboceros_geometry::Polyline3>, CommandError> {
    geometry::compute(meshes, options, tolerance)
}
