//! Catmull–Clark subdivision surfaces backed by the public OpenNURBS kernel.
//! Control topology is retained independently of display tessellations.
use crate::{
    AffineTransform3, BoundingBox3, GeometryError, MeshFace, Point3, TriangleMesh, Vector3,
};
use std::sync::Arc;
mod append;
mod chains;
mod cone;
mod control_net;
mod crease;
mod cylinder;
mod edges;
mod ellipsoid;
mod extrusion_conversion;
mod insert;
mod interpolation;
mod mesh_conversion;
mod primitives;
mod seams;
mod soft_crease;
mod sphere;
mod spin;
mod subdivision;
mod surface_conversion;
mod torus;
mod truncated_cone;
pub use append::SubDAppendResult;
pub use chains::{SubDDirectedEdge, SubDEdgeChain};
pub use control_net::SubDControlNetPriority;
pub use cylinder::SubDCylinderCap;
pub use insert::SubDVertexInsertion;
pub use mesh_conversion::SubDFromMeshOptions;
pub use seams::SubDEdgeSeamResult;
pub use sphere::SubDSphereStyle;
pub use surface_conversion::SubDFromSurfaceMethod;

/// A display tessellation with exact native limit normals for each quad corner.
/// A zero vector marks an undefined normal in a degenerate sector.
#[derive(Clone, Debug)]
pub struct SubDDisplayMesh {
    pub mesh: TriangleMesh,
    /// Corresponding active control-face ordinal for each display quad.
    pub face_sources: Vec<usize>,
    pub face_corner_normals: Vec<[Vector3; 4]>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SubD {
    data: Arc<viboceros_opennurbs::SubDData>,
    points: Arc<[Point3]>,
    surface_points: Arc<[Point3]>,
    bounds: BoundingBox3,
}
impl SubD {
    /// Smooth interior edges, crease boundary edges, and automatically assigned
    /// vertex tags. Faces must have consistent winding and manifold incidence.
    pub fn try_from_polygons(
        points: Vec<Point3>,
        faces: Vec<Vec<u32>>,
    ) -> Result<Self, GeometryError> {
        let coordinates = points.iter().map(|p| p.to_array()).collect::<Vec<_>>();
        Self::from_native(viboceros_opennurbs::subd_from_polygons(
            &coordinates,
            &faces,
        )?)
    }
    /// Restores the complete SubD record, including tags and edge sharpness.
    pub fn try_from_archive(bytes: &[u8]) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_from_archive(bytes)?)
    }
    fn from_native(mut data: viboceros_opennurbs::SubDData) -> Result<Self, GeometryError> {
        if data.edge_sharpness.len() != data.edges.len()
            || data.edge_face_counts.len() != data.edges.len()
        {
            return Err(viboceros_opennurbs::SubDError(
                "Incomplete control edge properties".into(),
            )
            .into());
        }
        data.selected_faces.clear();
        let points = data
            .points
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let surface_points = data
            .surface_points
            .iter()
            .copied()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let bounds = BoundingBox3::from_points(points.iter().copied())?;
        Ok(Self {
            data: Arc::new(data),
            points: points.into(),
            surface_points: surface_points.into(),
            bounds,
        })
    }
    pub fn control_points(&self) -> &[Point3] {
        &self.points
    }
    /// Exact limit-surface locations corresponding to the control vertices.
    pub fn surface_points(&self) -> &[Point3] {
        &self.surface_points
    }
    pub fn faces(&self) -> &[Vec<u32>] {
        &self.data.faces
    }
    pub fn edges(&self) -> &[[u32; 2]] {
        &self.data.edges
    }
    /// OpenNURBS tags: 1 smooth, 2 crease, 3 corner, 4 dart.
    pub fn vertex_tags(&self) -> &[u8] {
        &self.data.vertex_tags
    }
    /// OpenNURBS tags: 1 smooth, 2 crease, 4 smooth between tagged vertices.
    pub fn edge_tags(&self) -> &[u8] {
        &self.data.edge_tags
    }
    /// Soft sharpness at the two ends of each oriented control edge, 0..=4.
    /// Hard creases have a crease tag and zero soft sharpness.
    pub fn edge_sharpness(&self) -> &[[f64; 2]] {
        &self.data.edge_sharpness
    }
    pub fn edge_face_counts(&self) -> &[u32] {
        &self.data.edge_face_counts
    }
    /// Uses OpenNURBS' validated component setters, including neighbor updates.
    pub fn try_with_vertex_tags(&self, tags: &[(usize, u8)]) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_set_tags(
            self.archive(),
            true,
            tags,
        )?)
    }
    pub fn try_with_edge_tags(&self, tags: &[(usize, u8)]) -> Result<Self, GeometryError> {
        Self::from_native(viboceros_opennurbs::subd_set_tags(
            self.archive(),
            false,
            tags,
        )?)
    }
    /// Native control-cage facets for boundary selection. The displayed SubD
    /// continues to use the separately evaluated limit surface.
    pub fn control_mesh(&self) -> Result<TriangleMesh, GeometryError> {
        self.control_net_mesh(SubDControlNetPriority::Geometry)
    }
    pub fn archive(&self) -> &[u8] {
        &self.data.archive
    }
    /// Conservative control-hull bounds, enclosing the limit surface.
    pub fn bounds(&self) -> BoundingBox3 {
        self.bounds
    }
    pub fn try_with_control_points(&self, points: &[Point3]) -> Result<Self, GeometryError> {
        if points == self.control_points() {
            return Ok(self.clone());
        }
        let points = points.iter().map(|p| p.to_array()).collect::<Vec<_>>();
        Self::from_native(viboceros_opennurbs::subd_edit_points(
            self.archive(),
            &points,
        )?)
    }
    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        if transform == AffineTransform3::identity() {
            return Ok(self.clone());
        }
        let linear = transform.linear_rows();
        let translation = transform.translation().to_array();
        let matrix = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                if row == 3 {
                    if column == 3 { 1. } else { 0. }
                } else if column == 3 {
                    translation[row]
                } else {
                    linear[row][column]
                }
            })
        });
        Self::from_native(viboceros_opennurbs::subd_transform(self.archive(), matrix)?)
    }
    /// Samples the exact limit surface on a uniformly subdivided net. Density
    /// 1..=6 means 1..=6 subdivision steps; at most one million quads are made.
    /// Polygonal interpolation between samples has no model-tolerance guarantee.
    pub fn tessellate(&self, density: u32) -> Result<TriangleMesh, GeometryError> {
        let data = viboceros_opennurbs::subd_tessellate(self.archive(), density)?;
        Self::tessellation(data).map(|display| display.mesh)
    }
    pub fn tessellate_with_normals(&self, density: u32) -> Result<SubDDisplayMesh, GeometryError> {
        Self::tessellation(viboceros_opennurbs::subd_tessellate_with_normals(
            self.archive(),
            density,
        )?)
    }
    fn tessellation(data: viboceros_opennurbs::SubDData) -> Result<SubDDisplayMesh, GeometryError> {
        let mut normals = data.corner_normals.into_iter();
        let mut face_corner_normals = Vec::new();
        while let Some(a) = normals.next() {
            let b = normals.next().ok_or_else(|| {
                viboceros_opennurbs::SubDError("Incomplete SubD face normals".into())
            })?;
            let c = normals.next().ok_or_else(|| {
                viboceros_opennurbs::SubDError("Incomplete SubD face normals".into())
            })?;
            let d = normals.next().ok_or_else(|| {
                viboceros_opennurbs::SubDError("Incomplete SubD face normals".into())
            })?;
            face_corner_normals.push([
                Vector3::try_from(a)?,
                Vector3::try_from(b)?,
                Vector3::try_from(c)?,
                Vector3::try_from(d)?,
            ]);
        }
        let vertices = data
            .points
            .into_iter()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let faces = data
            .faces
            .into_iter()
            .map(|f| {
                let [a, b, c, d] = f[..] else {
                    unreachable!("subdivision produces quads")
                };
                MeshFace::Quad([a, b, c, d])
            })
            .collect();
        // Valid SubDs can have coincident evaluated samples. Retain their quad
        // records instead of making subdivision depend on a mesh area epsilon.
        Ok(SubDDisplayMesh {
            mesh: TriangleMesh::try_from_face_records(vertices, faces)?,
            face_corner_normals,
            face_sources: data.face_sources.into_iter().map(|f| f as usize).collect(),
        })
    }
    /// Adapts the display density to stay within the native quad budget.
    pub fn display_mesh(&self) -> Result<TriangleMesh, GeometryError> {
        self.tessellate(self.display_density())
    }
    pub fn display_mesh_with_normals(&self) -> Result<SubDDisplayMesh, GeometryError> {
        self.tessellate_with_normals(self.display_density())
    }
    fn display_density(&self) -> u32 {
        let mut density = 3;
        let corners = self.faces().iter().map(Vec::len).sum::<usize>();
        while density > 1 && corners * 4usize.pow(density - 1) > 1_000_000 {
            density -= 1;
        }
        density
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn quad() -> SubD {
        SubD::try_from_polygons(
            [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
                .into_iter()
                .map(|p| Point3::try_from(p).unwrap())
                .collect(),
            vec![vec![0, 1, 2, 3]],
        )
        .unwrap()
    }
    #[test]
    fn rounded_boundary_is_a_limit_surface_not_a_flat_control_mesh() {
        let s = quad();
        assert_eq!(s.vertex_tags(), &[2, 2, 2, 2]);
        assert_eq!(s.edge_tags(), &[2, 2, 2, 2]);
        assert_eq!(
            s.surface_points()[0],
            Point3::try_new(4. / 6., 0.5, 0.).unwrap()
        );
        assert_eq!(s.tessellate(1).unwrap().faces().len(), 4);
        assert_eq!(s.tessellate(3).unwrap().faces().len(), 64);
        assert!(s.tessellate(0).is_err());
        assert!(s.tessellate(7).is_err());
    }
    #[test]
    fn archives_roundtrip_and_edits_keep_the_topology_and_tags() {
        let s = quad();
        let loaded = SubD::try_from_archive(s.archive()).unwrap();
        assert_eq!(s.control_points(), loaded.control_points());
        assert_eq!(s.surface_points(), loaded.surface_points());
        assert_eq!(s.faces(), loaded.faces());
        let mut points = s.control_points().to_vec();
        points[2] = Point3::try_new(4., 3., 2.).unwrap();
        let edited = s.try_with_control_points(&points).unwrap();
        assert_eq!(edited.control_points(), points);
        assert_eq!(s.faces(), edited.faces());
        assert_eq!(s.vertex_tags(), edited.vertex_tags());
        assert_eq!(s.control_points()[2].z(), 0.);
        assert!(s.try_with_control_points(&points[..2]).is_err());
        for n in [0, 8, 16, s.archive().len() - 1] {
            assert!(SubD::try_from_archive(&s.archive()[..n]).is_err());
        }
    }
    #[test]
    fn triangle_ngon_closed_surface_and_topology_errors() {
        let points = quad().control_points().to_vec();
        let tri = SubD::try_from_polygons(points[..3].to_vec(), vec![vec![0, 1, 2]]).unwrap();
        assert_eq!(tri.tessellate(2).unwrap().faces().len(), 12);
        assert!(
            SubD::try_from_polygons(points.clone(), vec![vec![0, 1, 2], vec![0, 1, 3]]).is_err()
        );
        assert!(SubD::try_from_polygons(points.clone(), vec![vec![0, 1, 1, 3]]).is_err());
        assert!(SubD::try_from_polygons(points.clone(), vec![vec![0, 1, 8]]).is_err());
        let tetra = SubD::try_from_polygons(
            points,
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![1, 2, 3], vec![2, 0, 3]],
        )
        .unwrap();
        assert_eq!(tetra.vertex_tags(), &[1, 1, 1, 1]);
        assert_eq!(tetra.tessellate(2).unwrap().faces().len(), 48);
    }
}
