//! Retain only data affecting Center targets, not B-rep UV trims or attributes.
use super::*;

#[derive(Debug)]
pub(super) enum Source {
    Curve(Geometry),
    Surface(NurbsSurface),
    Faces(Vec<Option<FaceSource>>),
}

#[derive(Debug)]
pub(super) struct FaceSource {
    surface: NurbsSurface,
    edges: Vec<Option<(NurbsCurve, bool)>>,
}

impl Source {
    pub(super) fn new(geometry: &Geometry) -> Self {
        match geometry {
            Geometry::NurbsSurface(s) => Self::Surface(s.clone()),
            Geometry::Brep(b) => Self::Faces(
                b.faces()
                    .iter()
                    .map(|f| {
                        (f.loops().len() == 1).then(|| FaceSource {
                            surface: f.surface().clone(),
                            edges: f.loops()[0]
                                .trims()
                                .iter()
                                .map(|t| {
                                    Some((b.edges()[t.edge()?].curve().clone(), t.is_reversed_3d()))
                                })
                                .collect(),
                        })
                    })
                    .collect(),
            ),
            _ => Self::Curve(geometry.clone()),
        }
    }

    pub(super) fn matches(&self, geometry: &Geometry) -> bool {
        match (self, geometry) {
            (Self::Curve(old), current) => old == current,
            (Self::Surface(old), Geometry::NurbsSurface(current)) => old == current,
            (Self::Faces(old), Geometry::Brep(current)) => {
                old.len() == current.faces().len()
                    && old.iter().zip(current.faces()).all(|(old, face)| {
                        let Some(old) = old else {
                            return face.loops().len() != 1;
                        };
                        face.loops().len() == 1
                            && old.surface == *face.surface()
                            && old.edges.len() == face.loops()[0].trims().len()
                            && old
                                .edges
                                .iter()
                                .zip(face.loops()[0].trims())
                                .all(|(old, trim)| match (old, trim.edge()) {
                                    (None, None) => true,
                                    (Some((curve, reversed)), Some(edge)) => {
                                        *curve == *current.edges()[edge].curve()
                                            && *reversed == trim.is_reversed_3d()
                                    }
                                    _ => false,
                                })
                    })
            }
            _ => false,
        }
    }
}
