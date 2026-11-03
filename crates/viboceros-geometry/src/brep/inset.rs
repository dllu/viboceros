//! Planar face insets retaining supporting surfaces and shared boundary topology.
use super::*;
use crate::{CurveOffsetOperations, UnitVector3};
mod curves;
mod polygon;
mod regions;
mod storage;
#[cfg(test)]
mod tests;
const MAX_CORNERS: usize = 100_000;
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}
struct Boundary {
    inset: Vec<BrepTrim>,
    borders: Vec<BrepFace>,
}
struct Work {
    bytes: usize,
}
impl Work {
    fn charge_curve(&mut self, curve: &NurbsCurve) -> Result<(), GeometryError> {
        self.bytes = self
            .bytes
            .saturating_add(curve.control_points().len().saturating_mul(96))
            .saturating_add(curve.knots().len().saturating_mul(24));
        if self.bytes > 64 * 1024 * 1024 {
            return Err(invalid("planar inset exceeds its output-storage budget"));
        }
        Ok(())
    }
}
impl Brep {
    /// Insets planar faces with straight polygon loops or smooth NURBS loops.
    /// Outer boundaries shrink, holes expand, and original surfaces/outer
    /// edges survive. Selected slots hold caps; border faces are appended.
    /// `crease_corners` adds border strips at polygon corners and at joins
    /// between smooth source edges. A single smooth closed edge stays annular.
    /// Non-affine charts, curved G0 corners, seams, poles, crossing/collapsing
    /// offsets and topology changes are errors. Sources are immutable on error.
    /// Admission bounds selected corners to 100,000, boundary-pair work to
    /// 16 million and estimated output storage to 64 MiB before surface copies.
    pub fn try_inset_planar_faces(
        &self,
        selected: &[usize],
        distance: Real,
        crease_corners: bool,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        self.try_inset_planar_faces_with_operations(
            selected,
            distance,
            crease_corners,
            tolerance,
            None,
        )
    }
    /// Supplies the existing usd-brep offset/intersection/edit producers while
    /// retaining admission, topology and region validation in this layer.
    pub fn try_inset_planar_faces_with_operations(
        &self,
        selected: &[usize],
        distance: Real,
        crease_corners: bool,
        tolerance: Tolerance,
        operations: Option<CurveOffsetOperations>,
    ) -> Result<Self, GeometryError> {
        if !distance.is_finite() || distance <= 0. || selected.is_empty() {
            return Err(invalid(
                "planar inset requires faces and a positive finite distance",
            ));
        }
        let mut indices = BTreeSet::new();
        let mut corners = 0usize;
        let mut pairs = 0usize;
        for &i in selected {
            let face = self
                .faces
                .get(i)
                .ok_or_else(|| invalid("missing inset face"))?;
            if !indices.insert(i) {
                return Err(invalid("duplicate inset face"));
            }
            let n = face.loops.iter().map(|l| l.trims.len()).sum::<usize>();
            corners = corners.saturating_add(n);
            pairs = pairs.saturating_add(n.saturating_mul(n));
            if corners > MAX_CORNERS || pairs > 16_000_000 {
                return Err(invalid("planar inset exceeds its boundary work limit"));
            }
        }
        let mut work = Work {
            bytes: self.inset_storage_estimate(&indices, crease_corners)?,
        };
        let mut output = self.clone();
        for i in indices {
            output.inset_planar_face(
                i,
                distance,
                crease_corners,
                tolerance,
                operations,
                &mut work,
            )?;
        }
        Self::try_new(output.vertices, output.edges, output.faces, tolerance)
    }
    fn inset_planar_face(
        &mut self,
        index: usize,
        distance: Real,
        crease: bool,
        tolerance: Tolerance,
        operations: Option<CurveOffsetOperations>,
        work: &mut Work,
    ) -> Result<(), GeometryError> {
        let face = self.faces[index].clone();
        let [origin, east, _, north] = face.surface.try_affine_patch_corners(tolerance)?;
        let normal = origin
            .vector_to(east)?
            .cross(origin.vector_to(north)?)?
            .normalized_nonzero()?;
        let polygonal = face.loops.iter().all(|l| polygon::supported(self, l));
        let mut caps = Vec::new();
        let mut borders = Vec::new();
        for boundary in &face.loops {
            let result = if polygon::supported(self, boundary) {
                self.inset_polygon_boundary(&face, boundary, normal, distance, crease, tolerance)?
            } else {
                self.inset_smooth_boundary(
                    &face, boundary, normal, distance, crease, tolerance, operations, work,
                )?
            };
            caps.push(result.inset);
            borders.extend(result.borders);
        }
        self.faces[index] = if polygonal {
            BrepFace::try_from_polygon_boundaries(face.surface, face.reversed, caps)?
        } else {
            regions::validate(self, &face, &caps, normal, tolerance, operations)?;
            let loops = caps
                .into_iter()
                .zip(&face.loops)
                .map(|(trims, old)| BrepLoop::try_new(old.loop_type, trims))
                .collect::<Result<_, _>>()?;
            BrepFace::try_new(face.surface, face.reversed, loops)?
        };
        self.faces.extend(borders);
        Ok(())
    }
    fn inset_line_trim(
        &mut self,
        surface: &NurbsSurface,
        vertices: [usize; 2],
        points: [Point3; 2],
        tolerance: Tolerance,
    ) -> Result<BrepTrim, GeometryError> {
        if points[0].distance_to(points[1])? <= tolerance.absolute() {
            return Err(invalid("inset collapses a boundary edge"));
        }
        let curve = NurbsCurve::try_new(1, points.to_vec(), vec![0., 0., 1., 1.])?;
        let uv = surface.try_pullback_affine_curve(&curve, tolerance)?;
        let edge = self.edges.len();
        self.edges.push(BrepEdge::try_new(vertices, curve, 0.)?);
        let iso = trim_iso::classify(&uv, surface);
        BrepTrim::try_new(
            vertices,
            Some(edge),
            false,
            uv,
            BrepTrimType::Mated,
            iso,
            [0.; 2],
        )
    }
}

fn reverse_trim(trim: &BrepTrim) -> Result<BrepTrim, GeometryError> {
    let mut result = trim.clone();
    result.vertices.swap(0, 1);
    result.reversed_3d = !result.reversed_3d;
    result.curve = result.curve.reversed()?;
    Ok(result)
}
