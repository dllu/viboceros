//! Immutable curve and surface references during a Line Normal getter.
use super::*;
use crate::curve_reference::CapturedCurve;
use viboceros_document::GeometrySnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Object,
    Edge(usize),
    Face(usize),
}
/// Script bindings name a parent and optionally one component of its topology.
pub fn parse_binding(word: &str) -> Option<(ObjectId, Component)> {
    let (key, value) = word.trim_start_matches('_').split_once('=')?;
    if key.eq_ignore_ascii_case("Curve") || key.eq_ignore_ascii_case("Surface") {
        Some((value.parse().ok()?, Component::Object))
    } else {
        let (id, index) = crate::curve_reference::parse_edge(value)?;
        if key.eq_ignore_ascii_case("Edge") {
            Some((id, Component::Edge(index)))
        } else if key.eq_ignore_ascii_case("Face") {
            Some((id, Component::Face(index)))
        } else {
            None
        }
    }
}
#[derive(Clone, Debug)]
pub struct Reference {
    id: ObjectId,
    parent: GeometrySnapshot,
    tolerance: Tolerance,
    face: Option<usize>,
    curve: Option<CapturedCurve>,
}
impl Reference {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        component: Component,
        pick: Point3,
        ignore_trims: bool,
    ) -> Result<Self, CommandError> {
        let object = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(viboceros_document::DocumentError::ObjectNotFound(id))?;
        let geometry = object.geometry();
        let curve = match component {
            Component::Edge(index) => Some(CapturedCurve::capture(doc, id, Some(index))?),
            Component::Object if geometry.curve_ref().is_some() => {
                Some(CapturedCurve::capture(doc, id, None)?)
            }
            _ => None,
        };
        let face = if curve.is_some() {
            None
        } else {
            let face = match component {
                Component::Face(index) => Some(index),
                Component::Object => None,
                Component::Edge(_) => unreachable!(),
            };
            let location =
                crate::surface_reference::closest(geometry, face, pick, false, doc.tolerance())?
                    .ok_or(CommandError::Usage(
                        "Select a curve, surface, or polysurface face",
                    ))?;
            // Without IgnoreTrims the base getter ranges over the selected
            // polysurface; a face pick or IgnoreTrims retains one surface.
            if face.is_some() || ignore_trims {
                Some(location.face)
            } else {
                None
            }
        };
        Ok(Self {
            id,
            parent: object.geometry_snapshot().clone(),
            tolerance: doc.tolerance(),
            face,
            curve,
        })
    }
    pub fn object_id(&self) -> ObjectId {
        self.id
    }
    pub fn curve(&self) -> Option<&NurbsCurve> {
        self.curve.as_ref().map(CapturedCurve::curve)
    }
    pub fn surface_target(&self, ignore_trims: bool) -> Option<(ObjectId, Option<usize>, bool)> {
        self.curve
            .is_none()
            .then_some((self.id, self.face, ignore_trims))
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.id)
            && doc
                .object(self.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn location(&self, point: Point3, ignore_trims: bool) -> Result<Location, CommandError> {
        if let Some(curve) = &self.curve {
            let curve = curve.curve_ref();
            let parameter = curve.closest_parameter(point, self.tolerance)?;
            let sample = curve.evaluate_with_tangent(parameter)?;
            let curvature = curve.curvature_vector(parameter)?;
            let constraint = if curvature.length()? == 0. {
                Constraint::Plane(sample.tangent())
            } else {
                Constraint::Axis(curvature.normalized_nonzero()?.opposite())
            };
            return Ok(Location {
                point: sample.point(),
                constraint,
            });
        }
        let location = crate::surface_reference::closest(
            &self.parent,
            self.face,
            point,
            ignore_trims,
            self.tolerance,
        )?
        .ok_or(CommandError::Usage(
            "Normal surface location is unavailable",
        ))?;
        let normal = location.surface.normal_at(location.uv[0], location.uv[1])?;
        // Line Normal follows the face orientation. Move Normal's existing
        // underlying-surface convention is a different getter contract.
        Ok(Location {
            point: location.point,
            constraint: Constraint::Axis(if location.reversed {
                normal.opposite()
            } else {
                normal
            }),
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Constraint {
    Axis(UnitVector3),
    /// With zero curvature Rhino retains the entire normal plane.
    Plane(UnitVector3),
}
#[derive(Clone, Copy, Debug)]
pub struct Location {
    pub point: Point3,
    pub constraint: Constraint,
}
