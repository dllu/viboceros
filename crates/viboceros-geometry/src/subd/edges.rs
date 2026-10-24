//! Limit-edge samples retain original component ordinals through refinement.
use super::*;
mod curves;
impl SubD {
    /// Sample every active control edge on the limit surface. Polyline order
    /// matches `edges()` and endpoints match `surface_points()` in edge order.
    /// Samples are exact stations; no continuous curve error bound is claimed.
    pub fn edge_polylines(&self, density: u32) -> Result<Vec<Vec<Point3>>, GeometryError> {
        viboceros_opennurbs::subd_edge_samples(self.archive(), density)?
            .into_iter()
            .map(|line| line.into_iter().map(Point3::try_from).collect())
            .collect()
    }
    pub fn display_edge_polylines(&self) -> Result<Vec<Vec<Point3>>, GeometryError> {
        self.edge_polylines(self.display_density())
    }
}
