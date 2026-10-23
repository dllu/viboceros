//! Local control-edge and face splits retain the source archive and identities.
use super::*;
#[derive(Clone, Debug)]
pub struct SubDVertexInsertion {
    pub subd: SubD,
    pub vertex: usize,
    pub new_edge: usize,
}
impl SubD {
    /// Inserts a connected path over original control edges. The public native
    /// splits retain tags, sharpness and component identities. Without an
    /// explicit parallel-view normal, crossings use the first incident face's
    /// plane. Near-endpoint user picks are clamped to [0.001, 0.999].
    pub fn try_insert_edge_path(
        &self,
        picks: &[crate::PolygonEdgePoint],
        view_normal: Option<crate::UnitVector3>,
        tolerance: crate::Tolerance,
    ) -> Result<Self, GeometryError> {
        let plan = crate::edge_insertion::Plan::new(
            self.control_points(),
            self.faces(),
            self.edges(),
            picks,
            true,
            view_normal,
            tolerance,
        )?;
        if plan.splits.is_empty() && plan.chords.is_empty() {
            return Ok(self.clone());
        }
        let splits = plan
            .splits
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.edge,
                    s.parameter,
                    plan.points[self.control_points().len() + i].to_array(),
                )
            })
            .collect::<Vec<_>>();
        Self::from_native(viboceros_opennurbs::subd_insert_path(
            self.archive(),
            &splits,
            &plan.chords,
        )?)
    }
    pub fn try_insert_edge_vertex(
        &self,
        edge: usize,
        point: Point3,
    ) -> Result<Option<SubDVertexInsertion>, GeometryError> {
        if edge >= self.edges().len() {
            return Err(viboceros_opennurbs::SubDError(
                "SubD insertion edge outside its source table".into(),
            )
            .into());
        }
        viboceros_opennurbs::subd_insert_vertex(self.archive(), edge, point.to_array())?
            .map(|(data, vertex, new_edge)| {
                Self::from_native(data).map(|subd| SubDVertexInsertion {
                    subd,
                    vertex,
                    new_edge,
                })
            })
            .transpose()
    }
    pub fn try_insert_face_edge(
        &self,
        face: usize,
        a: usize,
        b: usize,
    ) -> Result<Option<(Self, usize)>, GeometryError> {
        if face >= self.faces().len()
            || a >= self.control_points().len()
            || b >= self.control_points().len()
        {
            return Err(viboceros_opennurbs::SubDError(
                "SubD face insertion index outside its source table".into(),
            )
            .into());
        }
        viboceros_opennurbs::subd_insert_face_edge(self.archive(), face, a, b)?
            .map(|(data, edge)| Self::from_native(data).map(|s| (s, edge)))
            .transpose()
    }
}
