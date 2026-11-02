//! Cached, one-sided surface jets at existing tessellation anchors.
use crate::{
    AffineTransform3, GeometryError, NurbsSurface, ParameterSide, Point2, Real, SurfaceJet2,
    SurfaceTessellation, TriangleMesh, UnitVector3,
};
use std::collections::HashMap;
#[cfg(test)]
#[path = "curvature_tests.rs"]
mod tests;

/// Geometry and differential samples used for curvature analysis. It owns no
/// model state. A vertex on a crease can retain distinct jets on either side.
pub struct CurvatureMesh {
    pub mesh: TriangleMesh,
    pub normals: Vec<[Option<UnitVector3>; 3]>,
    jets: Vec<SurfaceJet2>,
    principal: Vec<[Real; 2]>,
    indices: Vec<[Option<usize>; 3]>,
}

impl CurvatureMesh {
    pub(crate) fn from_subd(
        mesh: TriangleMesh,
        normals: Vec<[crate::Vector3; 4]>,
        jets: Vec<SurfaceJet2>,
        corner_indices: Vec<Option<usize>>,
    ) -> Result<Self, GeometryError> {
        if normals.len() != mesh.faces().len()
            || corner_indices.len() != 4 * mesh.faces().len()
            || corner_indices.iter().flatten().any(|i| *i >= jets.len())
        {
            return Err(GeometryError::Degenerate {
                context: "SubD differential sample channels",
            });
        }
        let values = jets
            .iter()
            .map(|j| j.curvature().ok().map(|k| k.principal))
            .collect::<Vec<_>>();
        let mut indices = Vec::new();
        let mut oriented = Vec::new();
        for (i, triangle) in mesh.triangles().iter().enumerate() {
            let face = i / 2;
            let corners = mesh.faces()[face].indices();
            let positions = triangle.map(|v| corners.iter().position(|c| *c == v).unwrap());
            indices.push(
                positions.map(|p| corner_indices[4 * face + p].filter(|i| values[*i].is_some())),
            );
            oriented.push(positions.map(|p| normals[face][p].normalized_nonzero().ok()));
        }
        Ok(Self {
            mesh,
            normals: oriented,
            jets,
            principal: values.into_iter().map(|v| v.unwrap_or([0., 0.])).collect(),
            indices,
        })
    }
    pub(crate) fn from_tessellation(t: SurfaceTessellation, surfaces: &[&NurbsSurface]) -> Self {
        let mut cache = HashMap::new();
        let mut jets = Vec::new();
        let mut principal = Vec::new();
        let mut indices = Vec::with_capacity(t.corners.len());
        let normals = t.corners.iter().map(|c| c.map(|c| c.normal)).collect();
        for (triangle, corners) in t.mesh.triangles().iter().zip(&t.corners) {
            let uv = corners.map(|c| c.uv);
            indices.push(std::array::from_fn(|i| {
                let side = |axis| super::corner_side(uv, i, axis);
                let key = (
                    corners[i].face,
                    triangle[i],
                    side(0) == ParameterSide::Left,
                    side(1) == ParameterSide::Left,
                );
                *cache.entry(key).or_insert_with(|| {
                    let surface = surfaces[corners[i].face.unwrap_or(0)];
                    let jet = sample_jet(surface, uv, i, [side(0), side(1)])?;
                    let k = jet.curvature().ok()?.principal;
                    let index = jets.len();
                    jets.push(jet);
                    principal.push(k);
                    Some(index)
                })
            }));
        }
        Self {
            mesh: t.mesh,
            normals,
            jets,
            principal,
            indices,
        }
    }

    pub fn principal_curvatures(
        &self,
        transform: Option<AffineTransform3>,
    ) -> Vec<[Option<[Real; 2]>; 3]> {
        let transformed = transform.map(|map| {
            self.jets
                .iter()
                .map(|j| {
                    let vector = |v| map.transform_vector(v);
                    let jet = SurfaceJet2 {
                        point: j.point,
                        derivative_u: vector(j.derivative_u).ok()?,
                        derivative_v: vector(j.derivative_v).ok()?,
                        derivative_uu: vector(j.derivative_uu).ok()?,
                        derivative_uv: vector(j.derivative_uv).ok()?,
                        derivative_vv: vector(j.derivative_vv).ok()?,
                    };
                    jet.curvature().ok().map(|c| c.principal)
                })
                .collect::<Vec<_>>()
        });
        self.indices
            .iter()
            .map(|c| {
                c.map(|i| {
                    i.and_then(|i| {
                        transformed
                            .as_ref()
                            .map_or(Some(self.principal[i]), |k| k[i])
                    })
                })
            })
            .collect()
    }
}

impl NurbsSurface {
    pub fn curvature_mesh(
        &self,
        samples_per_span: usize,
        tolerance: crate::Tolerance,
    ) -> Result<CurvatureMesh, GeometryError> {
        Ok(CurvatureMesh::from_tessellation(
            self.tessellate_with_normals(samples_per_span, tolerance)?,
            &[self],
        ))
    }
}

fn sample_jet(
    s: &NurbsSurface,
    uv: [Point2; 3],
    i: usize,
    sides: [ParameterSide; 2],
) -> Option<SurfaceJet2> {
    let p = uv[i];
    let jet = s
        .evaluate_with_second_derivatives_on_sides(p.x(), p.y(), sides[0], sides[1])
        .ok()?;
    if jet.curvature().is_ok() {
        return Some(jet);
    }
    // An incident UV ray can resolve a singular corner's sampled limit. A
    // diverging cone apex or unstable approach remains unavailable.
    let mut previous: Option<[Real; 2]> = None;
    let mut converged = 0;
    let a = uv[(i + 1) % 3];
    let b = uv[(i + 2) % 3];
    for f in [1e-2, 1e-3, 1e-4, 1e-5, 1e-6] {
        let u = crate::interpolate_scalar([p.x(), 0.5 * a.x() + 0.5 * b.x()], [0., 1.], f).ok()?;
        let v = crate::interpolate_scalar([p.y(), 0.5 * a.y() + 0.5 * b.y()], [0., 1.], f).ok()?;
        let jet = s
            .evaluate_with_second_derivatives_on_sides(u, v, sides[0], sides[1])
            .ok()?;
        let k = jet.curvature().ok()?.principal;
        if let Some(old) = previous {
            converged = if k
                .iter()
                .zip(old)
                .all(|(a, b)| (a - b).abs() <= 1e-7 * (1. + a.abs().max(b.abs())))
            {
                converged + 1
            } else {
                0
            };
            if converged >= 2 {
                return Some(jet);
            }
        }
        previous = Some(k);
    }
    None
}
