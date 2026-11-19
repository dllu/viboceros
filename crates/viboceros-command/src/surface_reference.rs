//! Shared selection and closest-point resolution on surface and B-rep face references.
use crate::CommandError;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_geometry::{Brep, GeometryError, NurbsSurface, Point3, Tolerance};
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

const BOUNDARY_REFERENCE_USAGE: &str = "Specify a boundary object ID or ID:face";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoundaryRef {
    pub object: ObjectId,
    pub face: Option<usize>,
}

impl BoundaryRef {
    pub fn parse(input: &str) -> Result<Self, CommandError> {
        let (id, face) = match input.split_once(':') {
            Some((id, face)) => (
                id,
                Some(
                    face.parse()
                        .map_err(|_| CommandError::Usage(BOUNDARY_REFERENCE_USAGE))?,
                ),
            ),
            None => (input, None),
        };
        Ok(Self {
            object: id
                .parse()
                .map_err(|_| CommandError::Usage(BOUNDARY_REFERENCE_USAGE))?,
            face,
        })
    }

    pub fn capture(self, document: &Document) -> Result<Brep, CommandError> {
        let object = document
            .object(self.object)
            .filter(|_| document.is_object_selectable(self.object))
            .ok_or(CommandError::Usage("Boundary surface is unavailable"))?;
        let tolerance = document.tolerance();
        match object.geometry() {
            Geometry::NurbsSurface(surface) if self.face.unwrap_or(0) == 0 => {
                Ok(Brep::try_surface_face(surface.clone(), tolerance)?)
            }
            Geometry::Brep(brep) => {
                let index = self.face.or_else(|| (brep.faces().len() == 1).then_some(0));
                let index = index.filter(|&index| index < brep.faces().len()).ok_or(
                    CommandError::Usage("Pick one boundary face or specify its index"),
                )?;
                Ok(brep.duplicate_faces(&[index], tolerance)?)
            }
            _ => Err(CommandError::Usage("Pick one boundary surface")),
        }
    }
}

impl std::fmt::Display for BoundaryRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.object)?;
        if let Some(index) = self.face {
            write!(f, ":{index}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoundaryInput {
    #[default]
    None,
    Select,
    Reference(BoundaryRef),
}

impl BoundaryInput {
    /// Recognize a boundary getter option or an explicit face reference.
    pub fn parse_option(word: &str) -> Result<Option<Self>, CommandError> {
        let word = word.trim_start_matches(['_', '-']);
        let (key, value) = word
            .split_once('=')
            .map_or((word, None), |(key, value)| (key, Some(value)));
        if !key.eq_ignore_ascii_case("ToBoundary") {
            return Ok(None);
        }
        Ok(Some(match value {
            Some(value) => Self::Reference(BoundaryRef::parse(value)?),
            None => Self::Select,
        }))
    }
}
