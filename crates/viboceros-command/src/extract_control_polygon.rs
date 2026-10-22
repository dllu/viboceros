//! Control-net extraction uses native geometry conversion and source-group copies.
use crate::*;
use viboceros_geometry::SubDControlNetPriority;

mod arguments;
pub use arguments::{Invocation, parse};
mod options;
mod selection;
pub use options::{Options, OutputLayer};
pub use selection::{Selection, Target};

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
        let parsed = parse(args)?;
        Ok(parsed.targets.is_none().then(|| parsed.options.prompt()))
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
        let parsed = parse(args)?;
        let mut targets = parsed.targets.unwrap_or_default();
        if parsed.include_selected {
            targets.extend(document.selected_objects().map(|o| Target::Whole(o.id())));
        }
        Selection::prepare(document, targets, parsed.options)?.apply(document)
    }
}

fn control_geometry(
    geometry: &Geometry,
    options: Options,
    tolerance: Tolerance,
) -> Result<Option<Geometry>, CommandError> {
    Ok(match geometry {
        Geometry::SubD(s) => Some(Geometry::Mesh(s.control_net_mesh(options.priority)?)),
        Geometry::NurbsSurface(s) => s.control_polygon_mesh(tolerance)?.map(Geometry::Mesh),
        Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0]
            .surface()
            .control_polygon_mesh(tolerance)?
            .map(Geometry::Mesh),
        Geometry::PolyCurve(c) => Some(Geometry::Polyline(c.control_polygon(tolerance)?)),
        Geometry::NurbsCurve(c) => Some(Geometry::Polyline(c.control_polygon(tolerance)?)),
        geometry if geometry.curve_ref().is_some() => {
            let curve = geometry
                .nurbs_curve_representation()?
                .expect("accepted curve has an exact NURBS form");
            Some(Geometry::Polyline(curve.control_polygon(tolerance)?))
        }
        _ => return Err(CommandError::UnsupportedExtractControlPolygonGeometry),
    })
}
