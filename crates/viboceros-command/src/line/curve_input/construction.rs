//! Contact solving and endpoint constraints for the shared getter.
use super::super::{Definition, segment};
use super::*;
struct Contact {
    point: Point3,
    tangent: UnitVector3,
    project_tangent: bool,
}
impl Input {
    fn contact(
        &self,
        p: Point3,
        normal: Option<UnitVector3>,
    ) -> Result<Option<Contact>, CommandError> {
        let Some(g) = self.guides.first() else {
            return Ok(None);
        };
        let c = g.source.curve_ref();
        let mut project_tangent = self.fixed;
        let t = if self.fixed {
            g.parameter
        } else if self.kind == Kind::Tangent {
            if let Some(t) = c.tangent_parameter_near(p, g.parameter, normal, self.tolerance)? {
                t
            } else {
                // The native getter can use an open curve's end tangent when
                // its closest contact is a domain boundary. Keep this separate
                // from the exact geometric tangent-point query.
                let t = c.closest_parameter(p, self.tolerance)?;
                if c.is_closed()? || (t != *c.domain().start() && t != *c.domain().end()) {
                    return Err(CommandError::Usage(
                        "No tangent reaches the endpoint in the selected curve domain",
                    ));
                }
                project_tangent = true;
                t
            }
        } else {
            c.perpendicular_parameter_near(p, g.parameter, normal, self.tolerance)?
        };
        let s = c.evaluate_with_tangent(t)?;
        Ok(Some(Contact {
            point: s.point(),
            tangent: s.tangent(),
            project_tangent,
        }))
    }
    pub(super) fn definition(
        &self,
        origin: Point3,
        mut end: Point3,
    ) -> Result<Definition, CommandError> {
        if let Some(n) = self.length {
            end = origin.translated(origin.direction_to(end)?.as_vector().scaled(n)?)?;
        }
        let start = if self.both {
            origin.translated(end.vector_to(origin)?)?
        } else {
            origin
        };
        Ok(Definition {
            line: segment(start, end)?,
            accepted_point: end,
        })
    }
    pub(super) fn resolve(
        &self,
        p: Point3,
        normal: Option<UnitVector3>,
    ) -> Result<Option<Definition>, CommandError> {
        if !self.endpoint() {
            return Ok(None);
        }
        if let Some(contact) = self.contact(p, normal)? {
            let (origin, tangent) = (contact.point, contact.tangent);
            let delta = origin.vector_to(p)?;
            let end = if self.kind == Kind::Tangent {
                if contact.project_tangent {
                    let projected = crate::native_line_projection::closest_point(
                        origin,
                        origin.translated(tangent.as_vector())?,
                        p,
                    )?
                    .0;
                    if let Some(n) = normal {
                        let height = origin.vector_to(projected)?.dot(n.as_vector())?;
                        projected.translated(n.as_vector().scaled(-height)?)?
                    } else {
                        projected
                    }
                } else if let Some(n) = normal {
                    p.translated(n.as_vector().scaled(-delta.dot(n.as_vector())?)?)?
                } else {
                    p
                }
            } else if let Some(normal) = normal {
                if let Ok(direction) = tangent
                    .as_vector()
                    .cross(normal.as_vector())?
                    .normalized_nonzero()
                {
                    let distance = delta.dot(direction.as_vector())?;
                    origin.translated(direction.as_vector().scaled(distance)?)?
                } else {
                    p.translated(normal.as_vector().scaled(-delta.dot(normal.as_vector())?)?)?
                }
            } else {
                p.translated(
                    tangent
                        .as_vector()
                        .scaled(-delta.dot(tangent.as_vector())?)?,
                )?
            };
            self.definition(origin, end).map(Some)
        } else {
            self.definition(self.start.unwrap(), p).map(Some)
        }
    }
    pub fn preview(&self, p: Point3) -> Option<Definition> {
        self.resolve(
            p,
            (self.planar && !self.fixed).then_some(self.plane.z_axis()),
        )
        .ok()
        .flatten()
    }
    /// Sliding contacts honor Planar; a fixed normal plane takes precedence.
    pub fn cursor_point(&self, p: Point3, planar: bool) -> Option<Point3> {
        if !self.endpoint() {
            return Some(p);
        }
        if !self.guides.is_empty() && (planar || self.fixed) {
            let mut geometry_only = self.clone();
            geometry_only.length = None;
            geometry_only
                .resolve(p, (planar && !self.fixed).then_some(self.plane.z_axis()))
                .ok()
                .flatten()
                .map(|d| d.accepted_point)
        } else {
            Some(p)
        }
    }

    pub fn mouse_plane(&self) -> Option<Frame3> {
        if self.kind == Kind::Tangent {
            return None;
        }
        self.fixed.then_some(())?;
        let tangent = self.contact(self.start?, None).ok()??.tangent;
        Frame3::try_from_normal(
            self.start?,
            tangent.as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()
    }
    pub fn mouse_line(&self) -> Option<crate::translation::DestinationConstraint> {
        if self.kind != Kind::Tangent || !self.fixed {
            return None;
        }
        let contact = self.contact(self.start?, None).ok()??;
        let (anchor, direction) = (contact.point, contact.tangent);
        Some(crate::translation::DestinationConstraint {
            anchor,
            direction: Some(direction),
            distance: None,
        })
    }
}
