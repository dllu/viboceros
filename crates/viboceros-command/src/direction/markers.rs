//! Camera-independent direction markers shared by Dir and persistent analysis.
use super::*;

#[derive(Clone, Debug)]
pub struct Marker {
    pub object: ObjectId,
    pub point: Point3,
    // Curve tangent or surface U, V, oriented normal.
    pub axes: [Option<Vector3>; 3],
    pub surface: bool,
}

pub fn collect_markers<'a>(
    geometries: impl IntoIterator<Item = (ObjectId, &'a Geometry)>,
    tolerance: Tolerance,
) -> Vec<Marker> {
    let mut markers = Vec::new();
    for (id, geometry) in geometries {
        if let Some(curve) = geometry.curve_ref() {
            let domain = curve.domain();
            for fraction in [0.15, 0.5, 0.85] {
                let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                if let Ok(sample) = curve.evaluate_with_tangent(t) {
                    markers.push(Marker {
                        object: id,
                        point: sample.point(),
                        axes: [Some(sample.tangent().as_vector()), None, None],
                        surface: false,
                    });
                }
            }
        } else {
            match geometry {
                Geometry::NurbsSurface(surface) => {
                    if let Some(marker) = surface_marker(id, surface, None, None, tolerance) {
                        markers.push(marker);
                    }
                }
                Geometry::Brep(brep) => {
                    for face in brep.faces() {
                        if let Some(marker) =
                            surface_marker(id, face.surface(), Some(face), None, tolerance)
                        {
                            markers.push(marker);
                        }
                    }
                }
                Geometry::Mesh(mesh) => {
                    // Bound annotation density independently of triangle count.
                    for face in (0..mesh.triangles().len())
                        .step_by(mesh.triangles().len().div_ceil(256).max(1))
                    {
                        if let Some(marker) = mesh_marker(id, mesh, face) {
                            markers.push(marker);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    markers
}

fn surface_marker(
    id: ObjectId,
    surface: &NurbsSurface,
    face: Option<&BrepFace>,
    point: Option<Point3>,
    tolerance: Tolerance,
) -> Option<Marker> {
    let (u, v) = if let Some(point) = point {
        surface.closest_parameters(point, tolerance).ok()?
    } else {
        let mut parameter = None;
        'outer: for u in [0.5, 0.25, 0.75, 0.125, 0.875] {
            for v in [0.5, 0.25, 0.75, 0.125, 0.875] {
                let u = *surface.domain_u().start() * (1. - u) + *surface.domain_u().end() * u;
                let v = *surface.domain_v().start() * (1. - v) + *surface.domain_v().end() * v;
                if face.is_none_or(|f| f.contains_parameters(u, v, tolerance).unwrap_or(false))
                    && surface.normal_at(u, v).is_ok()
                {
                    parameter = Some((u, v));
                    break 'outer;
                }
            }
        }
        parameter.or_else(|| {
            // A small trimmed region can miss the interior sampling grid.
            // Regular trim locations provide exact on-surface fallback arrows.
            for trim in face?.loops().iter().flat_map(|l| l.trims()) {
                let domain = trim.curve().domain();
                for fraction in [0.5, 0.25, 0.75] {
                    let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                    let Ok(uv) = trim.curve().evaluate(t) else {
                        continue;
                    };
                    if surface.normal_at(uv.x(), uv.y()).is_ok() {
                        return Some((uv.x(), uv.y()));
                    }
                }
            }
            None
        })?
    };
    if face.is_some_and(|f| !f.contains_parameters(u, v, tolerance).unwrap_or(false)) {
        return None;
    }
    let (point, du, dv) = surface.evaluate_with_derivatives(u, v).ok()?;
    let normal = surface.normal_at(u, v).ok()?.as_vector();
    Some(Marker {
        object: id,
        point,
        surface: true,
        axes: [
            du.normalized_nonzero().ok().map(|v| v.as_vector()),
            dv.normalized_nonzero().ok().map(|v| v.as_vector()),
            Some(if face.is_some_and(|f| f.is_reversed()) {
                normal.scaled(-1.).ok()?
            } else {
                normal
            }),
        ],
    })
}

fn mesh_marker(id: ObjectId, mesh: &TriangleMesh, face: usize) -> Option<Marker> {
    let [a, b, c] = mesh
        .triangles()
        .get(face)?
        .map(|i| mesh.vertices()[i as usize]);
    let point = Point3::try_new(
        a.x() / 3. + b.x() / 3. + c.x() / 3.,
        a.y() / 3. + b.y() / 3. + c.y() / 3.,
        a.z() / 3. + b.z() / 3. + c.z() / 3.,
    )
    .ok()?;
    Some(Marker {
        object: id,
        point,
        axes: [None, None, Some(mesh.face_normal(face).ok()?.as_vector())],
        surface: false,
    })
}

pub fn marker_at(
    id: ObjectId,
    geometry: &Geometry,
    face: usize,
    point: Option<Point3>,
    tolerance: Tolerance,
) -> Option<Marker> {
    match geometry {
        Geometry::Brep(brep) => {
            let face = brep.faces().get(face)?;
            surface_marker(id, face.surface(), Some(face), point, tolerance)
        }
        Geometry::NurbsSurface(surface) => surface_marker(id, surface, None, point, tolerance),
        Geometry::Mesh(mesh) => mesh_marker(id, mesh, face),
        _ => None,
    }
}
