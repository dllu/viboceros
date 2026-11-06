//! Read-only curve-contact line getters, shared by scripts and viewports.
use crate::{CommandError, curve_reference::CapturedCurve};
use viboceros_document::{Document, ObjectId};
use viboceros_geometry::{CurveRef, Frame3, Point3, Real, Tolerance, UnitVector3};
mod arguments;
mod construction;
mod options;
pub use arguments::requested;
pub(super) use arguments::run;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Perpendicular,
    Tangent,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Perpendicular => "Perpendicular",
            Self::Tangent => "Tangent",
        }
    }
    pub fn parse(word: &str) -> Option<Self> {
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Perpendicular") {
            Some(Self::Perpendicular)
        } else if word.eq_ignore_ascii_case("Tangent") {
            Some(Self::Tangent)
        } else {
            None
        }
    }
}
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
    kind: Kind,
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
        Self::with_kind(Kind::Perpendicular, plane, tolerance)
    }
    pub fn with_kind(kind: Kind, plane: Frame3, tolerance: Tolerance) -> Self {
        Self {
            kind,
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
    pub fn kind(&self) -> Kind {
        self.kind
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
                "Curve-contact Line is waiting for a point",
            ));
        }
        let source = CapturedCurve::capture(doc, id, edge)?;
        let parameter = source.curve_ref().closest_parameter(pick, self.tolerance)?;
        let point = source.curve_ref().evaluate(parameter)?;
        if self.stage == Stage::Endpoint {
            let origin = self.start.unwrap();
            let t = if self.kind == Kind::Tangent {
                source.curve_ref().tangent_parameter_near(
                    origin,
                    parameter,
                    None,
                    self.tolerance,
                )?
            } else {
                Some(source.curve_ref().perpendicular_parameter_near(
                    origin,
                    parameter,
                    None,
                    self.tolerance,
                )?)
            };
            let mut end = t
                .map(|t| source.curve_ref().evaluate(t))
                .transpose()?
                .unwrap_or(point);
            if self.kind == Kind::Perpendicular
                && end
                    .vector_to(origin)?
                    .dot(
                        source
                            .curve_ref()
                            .evaluate_with_tangent(t.unwrap())?
                            .tangent()
                            .as_vector(),
                    )?
                    .abs()
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
            let solve = match self.kind {
                Kind::Perpendicular => viboceros_geometry::common_perpendicular_near,
                Kind::Tangent => viboceros_geometry::common_tangent_near,
            };
            let pair = solve(
                first.source.curve_ref(),
                first.parameter,
                source.curve_ref(),
                parameter,
                self.tolerance,
            )?
            .ok_or(CommandError::Usage(
                "No common line contact in the selected curve domains",
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
            _ => Err(CommandError::Usage("Pick a curve for the Line start")),
        }
    }
    pub fn enter(&mut self) -> bool {
        if self.kind == Kind::Tangent && self.endpoint() && self.end_point {
            return true;
        }
        if self.endpoint() && !self.fixed && !self.guides.is_empty() && !self.end_point {
            self.fixed = true;
            true
        } else {
            false
        }
    }
    pub fn number(&mut self, n: Real) -> Result<Option<super::Definition>, CommandError> {
        if !self.endpoint() || !n.is_finite() || n == 0. {
            return Err(CommandError::Usage(
                "Curve-contact Line needs a finite nonzero length at the endpoint prompt",
            ));
        }
        self.length = Some(n);
        if self.kind == Kind::Tangent && self.fixed && self.both {
            let origin = self.start.unwrap();
            let g = &self.guides[0];
            let tangent = g
                .source
                .curve_ref()
                .evaluate_with_tangent(g.parameter)?
                .tangent();
            return self
                .definition(origin, origin.translated(tangent.as_vector())?)
                .map(Some);
        }
        Ok(None)
    }
    pub fn curve_ref(&self) -> Option<CurveRef<'_>> {
        self.guides.first().map(|g| g.source.curve_ref())
    }
}
