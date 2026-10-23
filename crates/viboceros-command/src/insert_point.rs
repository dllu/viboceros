//! Edge-constrained insertion previews, point-local Undo, and one history admission.
use crate::*;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{PolygonEdgePoint, UnitVector3};
mod arguments;
pub use arguments::{InsertAction, parse_actions};

#[derive(Clone, Debug)]
pub struct InsertSession {
    object: ObjectId,
    snapshot: GeometrySnapshot,
    tolerance: Tolerance,
    edges: Vec<LineSegment>,
    picks: Vec<PolygonEdgePoint>,
    candidate: Geometry,
    view_normal: Option<UnitVector3>,
    pick_mesh: Option<std::sync::Arc<TriangleMesh>>,
}
impl InsertSession {
    pub fn new(document: &Document, object: ObjectId) -> Result<Self, CommandError> {
        if !document.is_object_selectable(object) {
            return Err(CommandError::InsertPointUnavailable);
        }
        let source = document
            .object(object)
            .ok_or(CommandError::InsertPointUnavailable)?;
        let tolerance = document.tolerance();
        let edges = match source.geometry() {
            Geometry::Mesh(m) => m.wireframe_lines(Tolerance::MESH_VALIDATION)?,
            Geometry::SubD(s) => s
                .edges()
                .iter()
                .map(|&[a, b]| {
                    LineSegment::try_new(
                        s.control_points()[a as usize],
                        s.control_points()[b as usize],
                        tolerance,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(CommandError::InsertPointUnavailable),
        };
        Ok(Self {
            object,
            snapshot: source.geometry_snapshot().clone(),
            tolerance,
            edges,
            picks: Vec::new(),
            candidate: source.geometry().clone(),
            view_normal: None,
            pick_mesh: None,
        })
    }
    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn source_edges(&self) -> &[LineSegment] {
        &self.edges
    }
    pub fn point_count(&self) -> usize {
        self.picks.len()
    }
    pub fn candidate(&self) -> &Geometry {
        &self.candidate
    }
    /// Prepare the original control cage only when a graphical getter needs it.
    pub fn prepare_picking(&mut self) -> Result<(), CommandError> {
        if self.pick_mesh.is_none()
            && let Geometry::SubD(subd) = &*self.snapshot
        {
            self.pick_mesh = Some(std::sync::Arc::new(subd.control_mesh()?));
        }
        Ok(())
    }
    pub fn source_pick_mesh(&self) -> Option<&TriangleMesh> {
        match &*self.snapshot {
            Geometry::Mesh(mesh) => Some(mesh),
            Geometry::SubD(_) => self.pick_mesh.as_deref(),
            _ => None,
        }
    }
    pub fn validate(&self, document: &Document) -> Result<(), CommandError> {
        if document.tolerance() != self.tolerance
            || !document.is_object_selectable(self.object)
            || document
                .object(self.object)
                .is_none_or(|o| o.geometry_snapshot() != &self.snapshot)
        {
            return Err(CommandError::InsertPointStale);
        }
        Ok(())
    }
    fn compute(
        &self,
        picks: &[PolygonEdgePoint],
        normal: Option<UnitVector3>,
    ) -> Result<Geometry, CommandError> {
        match &*self.snapshot {
            Geometry::Mesh(m) => Ok(Geometry::Mesh(m.try_insert_edge_path(
                picks,
                normal,
                self.tolerance,
            )?)),
            Geometry::SubD(s) => Ok(Geometry::SubD(s.try_insert_edge_path(
                picks,
                normal,
                self.tolerance,
            )?)),
            _ => Err(CommandError::InsertPointUnavailable),
        }
    }
    pub fn insert(
        &mut self,
        document: &Document,
        pick: PolygonEdgePoint,
        view_normal: Option<UnitVector3>,
    ) -> Result<(), CommandError> {
        self.validate(document)?;
        if self.picks.len() >= 256 {
            return Err(CommandError::InsertPointResourceLimit);
        }
        if pick.edge >= self.edges.len() {
            return Err(CommandError::Usage(arguments::USAGE));
        }
        let mut picks = self.picks.clone();
        picks.push(pick);
        let normal = self.view_normal.or(view_normal);
        let candidate = self.compute(&picks, normal)?;
        self.picks = picks;
        self.candidate = candidate;
        self.view_normal = normal;
        Ok(())
    }
    pub fn pick_point(&self, point: Point3) -> Result<PolygonEdgePoint, CommandError> {
        let mut nearest: Option<(Real, PolygonEdgePoint)> = None;
        for (edge, line) in self.edges.iter().enumerate() {
            let parameter = line.closest_parameter(point, Tolerance::MESH_VALIDATION)?;
            let distance = line.point_at(parameter)?.distance_to(point)?;
            if nearest.is_none_or(|(best, _)| distance < best) {
                nearest = Some((distance, PolygonEdgePoint { edge, parameter }));
            }
        }
        nearest
            .map(|(_, pick)| pick)
            .ok_or(CommandError::InsertPointUnavailable)
    }
    pub fn undo_point(&mut self, document: &Document) -> Result<bool, CommandError> {
        self.validate(document)?;
        if self.picks.is_empty() {
            return Ok(false);
        }
        let mut picks = self.picks.clone();
        picks.pop();
        let candidate = self.compute(&picks, self.view_normal)?;
        self.candidate = candidate;
        self.picks = picks;
        if self.picks.is_empty() {
            self.view_normal = None;
        }
        Ok(true)
    }
    pub fn apply_preview(&self, document: &mut Document) -> Result<(), CommandError> {
        self.validate(document)?;
        if !self.picks.is_empty() {
            document.replace_object_geometries_preserving_control_selection([(
                self.object,
                self.candidate.clone(),
            )])?;
            document.move_objects_to_end_in_order([self.object])?;
        }
        document.clear_selection();
        Ok(())
    }
    pub fn finish(self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "InsertPoint", move |doc| self.apply(doc))
    }
    pub(super) fn apply(self, document: &mut Document) -> Result<String, CommandError> {
        self.validate(document)?;
        let count = self.picks.len();
        self.apply_preview(document)?;
        Ok(if count == 0 {
            "No points inserted".into()
        } else {
            format!("Inserted {count} edge point(s)")
        })
    }
}

pub(super) struct InsertPointCommand;
impl Command for InsertPointCommand {
    fn name(&self) -> &'static str {
        "InsertPoint"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        arguments::run(document, args)
    }
}
