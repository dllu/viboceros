//! Planar face insets retaining supporting surfaces and shared boundary topology.
use super::*;
use crate::{CurveEditPointSolver, CurveOffsetOperations, UnitVector3};
mod curves;
mod output_curves;
mod outputs;
mod polygon;
pub use outputs::{PlanarInsetOutput, PlanarInsetOutputMode};
mod regions;
mod source_edges;
mod storage;
use source_edges::SourceEdges;
#[cfg(test)]
mod tests;
const MAX_CORNERS: usize = 100_000;
fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidBrepTopology { context }
}
/// Surface Inset options shared by typed commands and interactive previews.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanarInsetOptions {
    pub distance: Real,
    pub crease_corners: bool,
    pub loose: bool,
}
/// Optional native curve producers; topology and validation stay shared.
#[derive(Clone, Copy, Default)]
pub struct PlanarInsetOperations {
    pub curves: Option<CurveOffsetOperations>,
    pub edit_points: Option<CurveEditPointSolver>,
}
struct Boundary {
    inset: Vec<BrepTrim>,
    borders: Vec<BrepFace>,
}
#[derive(Default)]
struct Work {
    bytes: usize,
    source_edges: SourceEdges,
    network_curves: output_curves::CurveOutputs,
    keep_curves: bool,
    selected: BTreeSet<usize>,
    face: usize,
    boundary: usize,
}
struct NetworkCurves {
    face: usize,
    boundary: usize,
    cap: NurbsCurve,
    connectors: Vec<NurbsCurve>,
    edges: Vec<usize>,
}
struct PreparedInset {
    brep: Brep,
    source_edges: Vec<bool>,
    networks: Vec<NetworkCurves>,
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
    /// Outer boundaries shrink and holes expand. Supporting surfaces and
    /// original boundary loci survive, with shared edges subdivided where
    /// crease connectors cross them. Selected slots hold caps; borders append.
    /// `crease_corners` adds border strips at polygon corners and G2 breaks
    /// in curved chains. G2 closed loops stay annular.
    /// Non-affine charts, curved G0 corners, seam trims, poles, crossing/collapsing
    /// offsets and cusp topology changes are errors. Sources are immutable on error.
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
        self.try_inset_planar_faces_with_options(
            selected,
            PlanarInsetOptions {
                distance,
                crease_corners,
                loose: false,
            },
            tolerance,
            PlanarInsetOperations {
                curves: operations,
                ..PlanarInsetOperations::default()
            },
        )
    }
    /// Loose mode offsets original Greville edit points without refining the
    /// source basis. It retains the edit-point API's degree/count limits and
    /// splits new internal tangent kinks at the native one-degree threshold.
    pub fn try_inset_planar_faces_with_options(
        &self,
        selected: &[usize],
        options: PlanarInsetOptions,
        tolerance: Tolerance,
        operations: PlanarInsetOperations,
    ) -> Result<Self, GeometryError> {
        self.inset_prepared(selected, options, tolerance, operations, false)
            .map(|p| p.brep)
    }
    fn inset_prepared(
        &self,
        selected: &[usize],
        options: PlanarInsetOptions,
        tolerance: Tolerance,
        operations: PlanarInsetOperations,
        keep_curves: bool,
    ) -> Result<PreparedInset, GeometryError> {
        let PlanarInsetOptions {
            distance,
            crease_corners,
            ..
        } = options;
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
            source_edges: SourceEdges::new(self),
            selected: indices.clone(),
            keep_curves,
            ..Default::default()
        };
        let mut output = self.clone();
        for i in indices {
            output.inset_planar_face(i, options, tolerance, operations, &mut work)?;
        }
        work.source_edges.reserve_edges(output.edges.len());
        Ok(PreparedInset {
            brep: Self::try_new(output.vertices, output.edges, output.faces, tolerance)?,
            source_edges: work.source_edges.flags(),
            networks: work.network_curves.into_records(),
        })
    }
    fn inset_planar_face(
        &mut self,
        index: usize,
        options: PlanarInsetOptions,
        tolerance: Tolerance,
        operations: PlanarInsetOperations,
        work: &mut Work,
    ) -> Result<(), GeometryError> {
        let PlanarInsetOptions {
            distance,
            crease_corners: crease,
            ..
        } = options;
        let face = self.faces[index].clone();
        let [origin, east, _, north] = face.surface.try_affine_patch_corners(tolerance)?;
        let normal = origin
            .vector_to(east)?
            .cross(origin.vector_to(north)?)?
            .normalized_nonzero()?;
        let polygonal = face.loops.iter().all(|l| polygon::supported(self, l));
        let mut caps = Vec::new();
        let mut borders = Vec::new();
        work.face = index;
        for (boundary_index, boundary) in face.loops.iter().enumerate() {
            work.boundary = boundary_index;
            let result = if polygon::supported(self, boundary) {
                self.inset_polygon_boundary(&face, boundary, normal, distance, crease, tolerance)?
            } else {
                self.inset_smooth_boundary(
                    &face, boundary, normal, options, tolerance, operations, work,
                )?
            };
            caps.push(result.inset);
            borders.extend(result.borders);
        }
        // Subdivision has already refreshed this face's original loops.
        work.source_edges.remove_face(self, index);
        self.faces[index] = if polygonal {
            BrepFace::try_from_polygon_boundaries(face.surface, face.reversed, caps)?
        } else {
            regions::validate(
                self,
                &self.faces[index],
                &caps,
                normal,
                tolerance,
                operations.curves,
            )?;
            let loops = caps
                .into_iter()
                .zip(&face.loops)
                .map(|(trims, old)| BrepLoop::try_new(old.loop_type, trims))
                .collect::<Result<_, _>>()?;
            BrepFace::try_new(face.surface, face.reversed, loops)?
        };
        let first = self.faces.len();
        work.source_edges.append_faces(index, borders.len());
        self.faces.extend(borders);
        work.source_edges
            .add_faces(self, std::iter::once(index).chain(first..self.faces.len()));
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
