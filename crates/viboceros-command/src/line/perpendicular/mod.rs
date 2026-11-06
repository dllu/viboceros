//! Read-only Line Perpendicular getter, shared by scripts and the viewport.
use crate::{CommandError, curve_reference::CapturedCurve};
use viboceros_document::{Document, ObjectId};
use viboceros_geometry::{CurveRef, Frame3, Point3, Real, Tolerance, UnitVector3};
mod arguments;
mod options;
pub use arguments::requested;
pub(super) use arguments::run;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Curve,
    FreeStart,
    CurveBase,
    Endpoint,
    FirstOfTwo,
    SecondOfTwo,
}
#[derive(Clone, Debug)]
struct Guide {
    source: CapturedCurve,
    parameter: Real,
}
#[derive(Clone, Debug)]
pub struct Input {
    plane: Frame3,
    tolerance: Tolerance,
    stage: Stage,
    guides: Vec<Guide>,
    start: Option<Point3>,
    fixed: bool,
    on_curve: bool,
    both: bool,
    length: Option<Real>,
    clears_selection: bool,
    end_point: bool,
    planar: bool,
}
impl Input {
    pub fn new(plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            plane,
            tolerance,
            stage: Stage::Curve,
            guides: Vec::new(),
            start: None,
            fixed: false,
            on_curve: false,
            both: false,
            length: None,
            clears_selection: false,
            end_point: false,
            planar: false,
        }
    }
    pub fn set_drafting(&mut self, plane: Frame3, planar: bool) {
        self.plane = plane;
        self.planar = planar;
    }
    pub fn stage(&self) -> Stage {
        self.stage
    }
    pub fn plane(&self) -> Frame3 {
        self.plane
    }
    pub fn both_sides(&self) -> bool {
        self.both
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.start
    }
    pub fn clears_selection(&self) -> bool {
        self.clears_selection
    }
    pub fn requires_curve(&self) -> bool {
        matches!(
            self.stage,
            Stage::Curve | Stage::FirstOfTwo | Stage::SecondOfTwo
        )
    }
    pub fn accepts_curve(&self) -> bool {
        self.requires_curve()
            || self.stage == Stage::Endpoint && self.guides.is_empty() && !self.end_point
    }
    pub fn endpoint(&self) -> bool {
        self.stage == Stage::Endpoint
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance() && self.guides.iter().all(|g| g.source.current(doc))
    }
    pub fn base_curve(&self) -> Option<&viboceros_geometry::NurbsCurve> {
        (self.stage == Stage::CurveBase).then(|| self.guides[0].source.curve())
    }
    pub fn reset(&mut self, tolerance: Tolerance) {
        self.tolerance = tolerance;
        self.guides.clear();
        self.start = None;
        self.length = None;
        self.end_point = false;
        self.fixed = false;
        self.stage = if self.on_curve {
            Stage::Curve
        } else if self.clears_selection {
            Stage::FirstOfTwo
        } else {
            Stage::Curve
        };
    }
    pub fn pick_curve(
        &mut self,
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
    ) -> Result<Option<super::Definition>, CommandError> {
        if !self.accepts_curve() {
            return Err(CommandError::Usage(
                "Line Perpendicular is waiting for a point",
            ));
        }
        let source = CapturedCurve::capture(doc, id, edge)?;
        let parameter = source.curve_ref().closest_parameter(pick, self.tolerance)?;
        let point = source.curve_ref().evaluate(parameter)?;
        if self.stage == Stage::Endpoint {
            let origin = self.start.unwrap();
            let t = source.curve_ref().perpendicular_parameter_near(
                origin,
                parameter,
                None,
                self.tolerance,
            )?;
            let mut end = source.curve_ref().evaluate(t)?;
            let tangent = source.curve_ref().evaluate_with_tangent(t)?.tangent();
            if end.vector_to(origin)?.dot(tangent.as_vector())?.abs()
                > 1e-9 * end.distance_to(origin)?.max(1.)
            {
                // A failed forced perpendicular snap falls back to the curve
                // point under the pointer, rather than inventing an extension.
                end = point;
            }
            return self.definition(origin, end).map(Some);
        }
        if self.stage == Stage::SecondOfTwo {
            let first = &self.guides[0];
            let pair = viboceros_geometry::common_perpendicular_near(
                first.source.curve_ref(),
                first.parameter,
                source.curve_ref(),
                parameter,
                self.tolerance,
            )?
            .ok_or(CommandError::Usage(
                "No common perpendicular in the selected curve domains",
            ))?;
            return self.definition(pair[0], pair[1]).map(Some);
        }
        self.guides.push(Guide { source, parameter });
        if self.stage == Stage::FirstOfTwo {
            self.stage = Stage::SecondOfTwo;
        } else if self.on_curve {
            self.stage = Stage::CurveBase;
            self.fixed = true;
        } else {
            self.start = Some(point);
            self.stage = Stage::Endpoint;
        }
        Ok(None)
    }
    pub fn point(&mut self, p: Point3) -> Result<Option<super::Definition>, CommandError> {
        match self.stage {
            Stage::FreeStart => {
                self.start = Some(p);
                self.stage = Stage::Endpoint;
                Ok(None)
            }
            Stage::CurveBase => {
                let g = &mut self.guides[0];
                g.parameter = g.source.curve_ref().closest_parameter(p, self.tolerance)?;
                self.start = Some(g.source.curve_ref().evaluate(g.parameter)?);
                self.stage = Stage::Endpoint;
                Ok(None)
            }
            Stage::Endpoint => self.resolve(
                p,
                (self.planar && !self.fixed).then_some(self.plane.z_axis()),
            ),
            _ => Err(CommandError::Usage("Pick a curve for Line Perpendicular")),
        }
    }
    pub fn enter(&mut self) -> bool {
        if self.endpoint() && !self.fixed && !self.guides.is_empty() && !self.end_point {
            self.fixed = true;
            true
        } else {
            false
        }
    }
    pub fn number(&mut self, n: Real) -> Result<(), CommandError> {
        if !self.endpoint() || !n.is_finite() || n == 0. {
            return Err(CommandError::Usage(
                "Line Perpendicular needs a finite nonzero length at the endpoint prompt",
            ));
        }
        self.length = Some(n);
        Ok(())
    }
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
        } else {
            c.perpendicular_parameter_near(p, g.parameter, normal, self.tolerance)?
        };
        let s = c.evaluate_with_tangent(t)?;
        Ok(Some((s.point(), s.tangent())))
    }
    fn definition(
        &self,
        origin: Point3,
        mut end: Point3,
    ) -> Result<super::Definition, CommandError> {
        if let Some(n) = self.length {
            end = origin.translated(origin.direction_to(end)?.as_vector().scaled(n)?)?;
        }
        let start = if self.both {
            origin.translated(end.vector_to(origin)?)?
        } else {
            origin
        };
        Ok(super::Definition {
            line: super::segment(start, end)?,
            accepted_point: end,
        })
    }
    fn resolve(
        &self,
        p: Point3,
        normal: Option<UnitVector3>,
    ) -> Result<Option<super::Definition>, CommandError> {
        if !self.endpoint() {
            return Ok(None);
        }
        if let Some((origin, tangent)) = self.contact(p, normal)? {
            let delta = origin.vector_to(p)?;
            let end = if let Some(normal) = normal {
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
    pub fn preview(&self, p: Point3) -> Option<super::Definition> {
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
        self.fixed.then_some(())?;
        let (_, tangent) = self.contact(self.start?, None).ok()??;
        Frame3::try_from_normal(
            self.start?,
            tangent.as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )
        .ok()
    }
    pub fn curve_ref(&self) -> Option<CurveRef<'_>> {
        self.guides.first().map(|g| g.source.curve_ref())
    }
}
