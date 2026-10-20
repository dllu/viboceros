//! Shared source selection, fit construction and history for line/plane fitting.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointFitKind {
    Line,
    Plane,
    Sphere,
}
impl PointFitKind {
    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "LineThroughPt" => Some(Self::Line),
            "PlaneThroughPt" => Some(Self::Plane),
            "Sphere FitPoints" => Some(Self::Sphere),
            _ => None,
        }
    }
    pub const fn minimum_points(self) -> usize {
        match self {
            Self::Line => 2,
            Self::Plane | Self::Sphere => 3,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "LineThroughPt",
            Self::Plane => "PlaneThroughPt",
            Self::Sphere => "Sphere FitPoints",
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Line => {
                "LineThroughPt (select at least two points, cloud points, mesh vertices or grips)"
            }
            Self::Plane => "PlaneThroughPt (select at least three points, cloud points or grips)",
            Self::Sphere => "Sphere FitPoints (select at least three points or grips)",
        }
    }
    fn source_points(self, geometry: &Geometry) -> &[Point3] {
        match geometry {
            Geometry::Point(p) => std::slice::from_ref(p),
            Geometry::PointCloud(cloud) if self != Self::Sphere => cloud.points(),
            Geometry::Mesh(mesh) if self == Self::Line => mesh.vertices(),
            _ => &[],
        }
    }
    /// Saturated count for the application's preselection threshold; no point
    /// arrays are copied and full mesh sources are eligible only for line fits.
    pub fn selected_point_count(self, document: &Document) -> usize {
        document
            .selected_objects()
            .map(|o| self.source_points(o.geometry()).len())
            .fold(0usize, usize::saturating_add)
            .saturating_add(
                document
                    .selected_control_points()
                    .take(self.minimum_points())
                    .count(),
            )
    }
    /// Read the selected point objects and current grip basis without editing
    /// the document, so nested construction getters share the same sources.
    pub fn selected_points(self, document: &Document) -> Vec<Point3> {
        document
            .selected_objects()
            .flat_map(|o| self.source_points(o.geometry()))
            .copied()
            .chain(document.selected_control_points().map(|(_, p)| p))
            .take(viboceros_geometry::MAX_PLANE_FIT_POINTS + 1)
            .collect()
    }
}

pub(crate) struct PointFitCommand(pub PointFitKind);
impl Command for PointFitCommand {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        require_consumed(args, 0, self.0.usage())?;
        Ok(Some(selection_prompt(self.0)))
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        run(self.0, document, args, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        run(self.0, document, args, true)
    }
}
pub fn selection_prompt(kind: PointFitKind) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: kind.name(),
        filter: match kind {
            PointFitKind::Line => ObjectSelectionFilter::LineFitPoints,
            PointFitKind::Plane => ObjectSelectionFilter::PlaneFitPoints,
            PointFitKind::Sphere => ObjectSelectionFilter::Points,
        },
        workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        options: vec![],
        menus: vec![],
        choices: vec![],
    }
}
pub(super) fn run(
    kind: PointFitKind,
    document: &mut Document,
    args: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    require_consumed(args, 0, kind.usage())?;
    let points = kind.selected_points(document);
    let geometry = match kind {
        PointFitKind::Line => LineSegment::try_fit_to_points(&points)?.map(Geometry::Line),
        PointFitKind::Plane => NurbsSurface::try_fit_plane(&points)?.map(Geometry::NurbsSurface),
        PointFitKind::Sphere => viboceros_geometry::SphereFit::try_fit_to_points(&points)?
            .map(|s| s.to_nurbs().map(Geometry::NurbsSurface))
            .transpose()?,
    };
    if geometry.is_none() && kind != PointFitKind::Line {
        return Err(CommandError::OperationDeclined);
    }
    let selection = document.selected_object_ids().collect::<Vec<_>>();
    let message = if let Some(geometry) = geometry {
        let id = document.add_geometry(geometry)?;
        format!("Added fitted object {id} from {} points", points.len())
    } else {
        "No line fits the selected points".into()
    };
    if postselected {
        document.release_command_selection_on_history_replay(selection)?;
        document.clear_selection();
    }
    Ok(message)
}

#[cfg(test)]
#[path = "plane_through_points/tests.rs"]
mod plane_tests;
#[cfg(test)]
mod sphere_tests;
#[cfg(test)]
mod tests;
