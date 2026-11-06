//! Contact solving and endpoint constraints for the shared getter.
use super::super::{Definition, segment};
use super::*;
impl Input {
    fn contact(
        &self,
        p: Point3,
        normal: Option<UnitVector3>,
    ) -> Result<Option<(Point3, UnitVector3)>, CommandError> {
        let Some(g) = self.guides.first() else {
            return Ok(None);
        };
        let c = g.source.curve_ref();
        let t = if self.fixed {
            g.parameter
        } else if self.kind == Kind::Tangent {
            c.tangent_parameter_near(p, g.parameter, normal, self.tolerance)?
                .ok_or(CommandError::Usage(
                    "No tangent reaches the endpoint in the selected curve domain",
                ))?
        } else {
            c.perpendicular_parameter_near(p, g.parameter, normal, self.tolerance)?
        };
        let s = c.evaluate_with_tangent(t)?;
        Ok(Some((s.point(), s.tangent())))
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
        if let Some((origin, tangent)) = self.contact(p, normal)? {
            let delta = origin.vector_to(p)?;
            let end = if self.kind == Kind::Tangent {
                if self.fixed {
                    crate::native_line_projection::closest_point(
                        origin,
                        origin.translated(tangent.as_vector())?,
                        p,
                    )?
                    .0
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
        let (_, tangent) = self.contact(self.start?, None).ok()??;
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
        let (anchor, direction) = self.contact(self.start?, None).ok()??;
        Some(crate::translation::DestinationConstraint {
            anchor,
            direction: Some(direction),
            distance: None,
        })
    }
}
