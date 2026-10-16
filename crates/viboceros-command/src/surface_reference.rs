//! Shared closest-point resolution on immutable surface and B-rep face references.
use viboceros_document::Geometry;
use viboceros_geometry::{GeometryError, NurbsSurface, Point3, Tolerance};
pub struct SurfacePoint<'a> {
    pub surface: &'a NurbsSurface,
    pub face: usize,
    pub uv: [f64; 2],
    pub point: Point3,
    pub reversed: bool,
}
pub fn surface(geometry: &Geometry, face: Option<usize>) -> Option<(&NurbsSurface, usize, bool)> {
    match geometry {
        Geometry::NurbsSurface(s) if face.is_none_or(|f| f == 0) => Some((s, 0, false)),
        Geometry::Brep(b) => {
            let f = face.or_else(|| (b.faces().len() == 1).then_some(0))?;
            let face = b.faces().get(f)?;
            Some((face.surface(), f, face.is_reversed()))
        }
        _ => None,
    }
}
pub fn closest(
    geometry: &Geometry,
    face: Option<usize>,
    pick: Point3,
    ignore_trims: bool,
    tolerance: Tolerance,
) -> Result<Option<SurfacePoint<'_>>, GeometryError> {
    let face = match geometry {
        Geometry::Brep(b) if face.is_none() && b.faces().len() > 1 => {
            match b.closest_face_parameters(pick, tolerance)? {
                Some((f, _, _)) => Some(f),
                None => return Ok(None),
            }
        }
        _ => face,
    };
    let Some((surface, face, reversed)) = surface(geometry, face) else {
        return Ok(None);
    };
    let (u, v) = match geometry {
        Geometry::Brep(b) if !ignore_trims => {
            match b.closest_parameters_on_face(face, pick, tolerance)? {
                Some(uv) => uv,
                None => return Ok(None),
            }
        }
        _ => surface.closest_parameters(pick, tolerance)?,
    };
    Ok(Some(SurfacePoint {
        surface,
        face,
        uv: [u, v],
        point: surface.evaluate(u, v)?,
        reversed,
    }))
}
