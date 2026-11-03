//! Cached, one-sided surface jets at existing tessellation anchors.
use crate::{
    AffineTransform3, GeometryError, NurbsSurface, ParameterSide, Point2, Real, SurfaceJet2,
    SurfaceTessellation, TriangleMesh, UnitVector3,
};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
#[cfg(test)]
#[path = "curvature_tests.rs"]
mod tests;

/// Geometry and differential samples used for curvature analysis. It owns no
/// model state. NURBS jets and curvature metrics are evaluated on demand.
/// A crease retains distinct samples on either side.
pub struct CurvatureMesh {
    pub mesh: TriangleMesh,
    pub normals: Vec<[Option<UnitVector3>; 3]>,
    samples: LazyLock<Samples, SampleBuilder>,
}

type SampleBuilder = Box<dyn FnOnce() -> Samples + Send + Sync>;
struct Samples {
    jets: Vec<SurfaceJet2>,
    principal: Vec<[Real; 2]>,
    indices: Vec<[Option<usize>; 3]>,
}

impl CurvatureMesh {
    /// Distinct vertex samples for statistics, independent of triangle valence.
    /// A crease can contribute both one-sided values at the same vertex.
    pub fn principal_samples(&self) -> impl Iterator<Item = [Real; 2]> + '_ {
        let samples = &*self.samples;
        let mut seen = HashSet::new();
        self.mesh
            .triangles()
            .iter()
            .zip(&samples.indices)
            .flat_map(move |(vertices, indices)| {
                vertices
                    .iter()
                    .zip(indices)
                    .filter_map(move |(vertex, index)| {
                        index.map(|index| (*vertex, samples.principal[index]))
                    })
            })
            .filter_map(move |(vertex, k)| {
                seen.insert((vertex, k[0].to_bits(), k[1].to_bits()))
                    .then_some(k)
            })
    }

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
        let mut indices = Vec::new();
        let mut oriented = Vec::new();
        for (i, triangle) in mesh.triangles().iter().enumerate() {
            let face = i / 2;
            let corners = mesh.faces()[face].indices();
            let positions = triangle.map(|v| corners.iter().position(|c| *c == v).unwrap());
            indices.push(positions.map(|p| corner_indices[4 * face + p]));
            oriented.push(positions.map(|p| normals[face][p].normalized_nonzero().ok()));
        }
        Ok(Self {
            mesh,
            normals: oriented,
            samples: LazyLock::new(Box::new(move || {
                let values = jets
                    .iter()
                    .map(|j| j.curvature().ok().map(|k| k.principal))
                    .collect::<Vec<_>>();

                for corner in &mut indices {
                    for index in corner {
                        *index = index.filter(|i| values[*i].is_some());
                    }
                }
                Samples {
                    jets,
                    principal: values.into_iter().map(|v| v.unwrap_or([0., 0.])).collect(),
                    indices,
                }
            })),
        })
    }
    pub(crate) fn from_tessellation(t: SurfaceTessellation, surfaces: &[&NurbsSurface]) -> Self {
        let normals = t.corners.iter().map(|c| c.map(|c| c.normal)).collect();
        let triangles = t.mesh.triangles().to_vec();
        let corners = t.corners;
        let surfaces = surfaces.iter().map(|s| (*s).clone()).collect::<Vec<_>>();
        Self {
            mesh: t.mesh,
            normals,
            samples: LazyLock::new(Box::new(move || {
                let mut cache = HashMap::new();
                let mut regular = HashMap::new();
                let mut jets = Vec::new();
                let mut principal = Vec::new();
                let mut indices = Vec::with_capacity(corners.len());
                for (triangle, corners) in triangles.iter().zip(&corners) {
                    let uv = corners.map(|c| c.uv);
                    indices.push(std::array::from_fn(|i| {
                        let side = |axis| super::corner_side(uv, i, axis);
                        let key = (
                            corners[i].face,
                            triangle[i],
                            side(0) == ParameterSide::Left,
                            side(1) == ParameterSide::Left,
                        );
                        let surface = &surfaces[corners[i].face.unwrap_or(0)];
                        let sides = super::regular_sides(surface, uv[i], [side(0), side(1)]);
                        let regular_key = (
                            corners[i].face,
                            triangle[i],
                            sides.map(|s| s == ParameterSide::Left),
                        );
                        if let Some(index) = regular.get(&regular_key) {
                            return Some(*index);
                        }
                        if let Some(index) = cache.get(&key) {
                            return *index;
                        }
                        let sampled = sample_jet(surface, uv, i, [side(0), side(1)]);
                        let Some((jet, k, is_regular)) = sampled else {
                            cache.insert(key, None);
                            return None;
                        };
                        let index = jets.len();
                        jets.push(jet);
                        principal.push(k);
                        if is_regular {
                            regular.insert(regular_key, index);
                        } else {
                            cache.insert(key, Some(index));
                        }
                        Some(index)
                    }));
                }
                Samples {
                    jets,
                    principal,
                    indices,
                }
            })),
        }
    }

    pub fn principal_curvatures(
        &self,
        transform: Option<AffineTransform3>,
    ) -> Vec<[Option<[Real; 2]>; 3]> {
        let samples = &*self.samples;
        let transformed = transform.map(|map| {
            samples
                .jets
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
        samples
            .indices
            .iter()
            .map(|c| {
                c.map(|i| {
                    i.and_then(|i| {
                        transformed
                            .as_ref()
                            .map_or(Some(samples.principal[i]), |k| k[i])
                    })
                })
            })
            .collect()
    }
}

impl NurbsSurface {
    pub fn curvature_mesh_with_density(
        &self,
        density: crate::NurbsMeshDensity,
        tolerance: crate::Tolerance,
    ) -> Result<CurvatureMesh, GeometryError> {
        Ok(CurvatureMesh::from_tessellation(
            self.tessellate_with_density(density, tolerance)?,
            &[self],
        ))
    }
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
) -> Option<(SurfaceJet2, [Real; 2], bool)> {
    let p = uv[i];
    let jet = s
        .evaluate_with_second_derivatives_on_sides(p.x(), p.y(), sides[0], sides[1])
        .ok()?;
    if let Ok(k) = jet.curvature() {
        return Some((jet, k.principal, true));
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
                return Some((jet, k, false));
            }
        }
        previous = Some(k);
    }
    None
}
