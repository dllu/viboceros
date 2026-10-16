//! Shared source selection, fit construction and history for line/plane fitting.
use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointFitKind {
    Line,
    Plane,
}
impl PointFitKind {
    pub fn from_command(command: &str) -> Option<Self> {
        match command {
            "LineThroughPt" => Some(Self::Line),
            "PlaneThroughPt" => Some(Self::Plane),
            _ => None,
        }
    }
    pub const fn minimum_points(self) -> usize {
        match self {
            Self::Line => 2,
            Self::Plane => 3,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "LineThroughPt",
            Self::Plane => "PlaneThroughPt",
        }
    }
    fn usage(self) -> &'static str {
        match self {
            Self::Line => {
                "LineThroughPt (select at least two points, cloud points, mesh vertices or grips)"
            }
            Self::Plane => "PlaneThroughPt (select at least three points, cloud points or grips)",
        }
    }
    fn source_points(self, geometry: &Geometry) -> &[Point3] {
        match geometry {
            Geometry::Point(p) => std::slice::from_ref(p),
            Geometry::PointCloud(cloud) => cloud.points(),
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
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: match self.0 {
                PointFitKind::Line => ObjectSelectionFilter::LineFitPoints,
                PointFitKind::Plane => ObjectSelectionFilter::PlaneFitPoints,
            },
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![],
            menus: vec![],
            choices: vec![],
        }))
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
fn run(
    kind: PointFitKind,
    document: &mut Document,
    args: &[&str],
    postselected: bool,
) -> Result<String, CommandError> {
    require_consumed(args, 0, kind.usage())?;
    let maximum = viboceros_geometry::MAX_PLANE_FIT_POINTS;
    let points = document
        .selected_objects()
        .flat_map(|o| kind.source_points(o.geometry()))
        .copied()
        .chain(document.selected_control_points().map(|(_, p)| p))
        .take(maximum + 1)
        .collect::<Vec<_>>();
    let geometry = match kind {
        PointFitKind::Line => LineSegment::try_fit_to_points(&points)?.map(Geometry::Line),
        PointFitKind::Plane => NurbsSurface::try_fit_plane(&points)?.map(Geometry::NurbsSurface),
    };
    if geometry.is_none() && kind == PointFitKind::Plane {
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
mod tests;
