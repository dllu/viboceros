//! Optional source-surface channels, separate from editable mesh topology.
use crate::{
    GeometryError, NurbsSurface, ParameterSide, Point2, Real, Tolerance, TriangleMesh, UnitVector3,
};

/// Each triangle corner keeps its own normal, preserving face and knot creases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TessellationCorner {
    /// Source B-rep face index; `None` for a standalone surface.
    pub face: Option<usize>,
    /// Parameters in a translated local chart of the source surface.
    pub uv: Point2,
    /// Additive chart origin in the source's native parameter domain. Retain
    /// this pair with `uv`: adding them in f64 can lose small local stations.
    pub uv_origin: [Real; 2],
    /// Oriented unit normal. An isolated unresolved singularity is `None`;
    /// it never discards regular normals elsewhere in the object.
    pub normal: Option<UnitVector3>,
    pub normal_source: TessellationNormalSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TessellationNormalSource {
    /// Exact regular U×V direction, with B-rep orientation applied.
    Analytic,
    /// Numerically converged one-sided approach along this triangle's UV ray.
    /// This is a sampled limit, not a certificate of path independence.
    SampledLimit,
    Unavailable,
}

/// Export channels parallel to `mesh.triangles()`, without welding hard edges.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTessellation {
    pub mesh: TriangleMesh,
    pub corners: Vec<[TessellationCorner; 3]>,
}

impl NurbsSurface {
    /// Retain analytic per-corner normals and native UV samples for export.
    pub fn tessellate_with_normals(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<SurfaceTessellation, GeometryError> {
        let (mesh, parameters) =
            self.tessellate_grid_with_parameters(samples_per_span, false, tolerance, true)?;
        let corners = mesh
            .triangles()
            .iter()
            .map(|triangle| {
                triangle_corners(self, triangle.map(|i| parameters[i as usize]), None, false)
            })
            .collect();
        Ok(SurfaceTessellation { mesh, corners })
    }
}

pub(crate) fn triangle_corners(
    surface: &NurbsSurface,
    uv: [Point2; 3],
    face: Option<usize>,
    reversed: bool,
) -> [TessellationCorner; 3] {
    // Use an incident sample to choose a side at internal knots, without
    // averaging parameter coordinates that might overflow or erase an interval.
    std::array::from_fn(|i| {
        let p = uv[i];
        let side = |axis: usize| {
            let coordinate = |p: Point2| if axis == 0 { p.x() } else { p.y() };
            uv.iter()
                .find(|q| coordinate(**q) != coordinate(p))
                .map_or(ParameterSide::Right, |q| {
                    if coordinate(*q) < coordinate(p) {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    }
                })
        };
        let regular = surface.normal_at_on_sides(p.x(), p.y(), side(0), side(1));
        let (normal, normal_source) = match regular {
            Ok(n) => (Some(n), TessellationNormalSource::Analytic),
            Err(_) => match limiting_normal(surface, p, uv[(i + 1) % 3], uv[(i + 2) % 3]) {
                Some(n) => (Some(n), TessellationNormalSource::SampledLimit),
                None => (None, TessellationNormalSource::Unavailable),
            },
        };
        let normal = normal.map(|n| if reversed { n.opposite() } else { n });
        TessellationCorner {
            face,
            uv: p,
            uv_origin: [0.; 2],
            normal,
            normal_source,
        }
    })
}

fn limiting_normal(surface: &NurbsSurface, p: Point2, a: Point2, b: Point2) -> Option<UnitVector3> {
    let mut previous: Option<UnitVector3> = None;
    let mut last = None;
    let mut converged = 0;
    for fraction in [1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8] {
        let sample = |p, a, b| -> Option<Real> {
            let middle = crate::interpolate_scalar([a, b], [0., 1.], 0.5).ok()?;
            crate::interpolate_scalar([p, middle], [0., 1.], fraction).ok()
        };
        let u = sample(p.x(), a.x(), b.x())?;
        let v = sample(p.y(), a.y(), b.y())?;
        if u == p.x() && v == p.y() {
            break;
        }
        let n = surface.normal_at(u, v).ok()?;
        if let Some(old) = previous {
            let error = (n.x() - old.x())
                .hypot(n.y() - old.y())
                .hypot(n.z() - old.z());
            converged = if error <= 1e-7 { converged + 1 } else { 0 };
        }
        previous = Some(n);
        last = Some(n);
    }
    (converged >= 2).then_some(last).flatten()
}

#[cfg(test)]
mod tests;
