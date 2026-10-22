//! Mesh sharing and surface tessellation have different smoothing policies.
use super::{NaVector3, Point3, Real, TriangleMesh};
use std::borrow::Cow;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

const SMOOTH_SHADING_COSINE: Real = std::f64::consts::FRAC_1_SQRT_2;
const FALLBACK: NaVector3<Real> = NaVector3::new(0., 0., 1.);

pub(super) fn mesh_corner_normals(mesh: &TriangleMesh) -> Vec<[NaVector3<Real>; 3]> {
    let normals = mesh
        .vertex_normals()
        .map(Cow::Borrowed)
        .or_else(|| mesh.computed_vertex_normals().ok().map(Cow::Owned));
    mesh.triangles()
        .iter()
        .enumerate()
        .map(|(face, triangle)| {
            triangle.map(|raw| {
                normals
                    .as_deref()
                    .and_then(|normals| {
                        let n = normals[raw as usize];
                        NaVector3::new(n.x(), n.y(), n.z()).try_normalize(0.)
                    })
                    .unwrap_or_else(|| {
                        mesh.face_normal(face)
                            .map(|n| NaVector3::new(n.x(), n.y(), n.z()))
                            .unwrap_or(FALLBACK)
                    })
            })
        })
        .collect()
}

pub(super) fn tessellation_corner_normals(mesh: &TriangleMesh) -> Vec<[NaVector3<Real>; 3]> {
    if mesh.vertex_normals().is_some() {
        return mesh_corner_normals(mesh);
    }
    let face_normals = (0..mesh.triangles().len())
        .map(|index| {
            mesh.face_normal(index)
                .map(|normal| NaVector3::new(normal.x(), normal.y(), normal.z()))
                .ok()
        })
        .collect::<Vec<_>>();

    // Surface tessellation duplicates vertices at knot-span boundaries. Group
    // coincident samples so continuous spans shade together; retain sharp caps.
    let mut incident_faces: HashMap<[u64; 3], Vec<usize>> = HashMap::new();
    for (face_index, triangle) in mesh.triangles().iter().enumerate() {
        for &vertex_index in triangle {
            let point = mesh.vertices()[vertex_index as usize];
            incident_faces
                .entry(point_position_key(point))
                .or_default()
                .push(face_index);
        }
    }

    mesh.triangles()
        .iter()
        .enumerate()
        .map(|(face_index, triangle)| {
            let Some(reference) = face_normals[face_index] else {
                return [FALLBACK; 3];
            };
            triangle.map(|vertex_index| {
                let point = mesh.vertices()[vertex_index as usize];
                let mut sum = NaVector3::zeros();
                for &incident in &incident_faces[&point_position_key(point)] {
                    if let Some(candidate) = face_normals[incident]
                        && reference.dot(&candidate) >= SMOOTH_SHADING_COSINE
                    {
                        sum += candidate;
                    }
                }
                sum.try_normalize(Real::EPSILON).unwrap_or(reference)
            })
        })
        .collect()
}

fn point_position_key(point: Point3) -> [u64; 3] {
    [point.x(), point.y(), point.z()].map(|value| {
        if value == 0.0 {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }
    })
}
