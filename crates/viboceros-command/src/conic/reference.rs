//! Immutable curve/edge capture and the common temporary SubCrv operations.
use super::*;
use crate::curve_reference::CapturedCurve;
use viboceros_document::ObjectId;
use viboceros_geometry::Curve3;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceStage {
    SelectCurve,
    TrimStart,
    TrimEnd,
    Endpoint,
}
#[derive(Clone, Debug)]
pub struct ReferenceInput {
    captured: CapturedCurve,
    working: Option<Curve3>,
    cursor: NurbsCurve,
    stage: ReferenceStage,
    start: Option<Real>,
    length: Option<Real>,
    hover: Option<Real>,
    locked_forward: Option<bool>,
}
impl ReferenceInput {
    pub fn capture(
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        subcurve: bool,
    ) -> Result<Self, CommandError> {
        let captured = CapturedCurve::capture(doc, id, edge)?;
        let cursor = captured.curve().clone();
        Ok(Self {
            captured,
            working: None,
            cursor,
            stage: if subcurve {
                ReferenceStage::TrimStart
            } else {
                ReferenceStage::Endpoint
            },
            start: None,
            length: None,
            hover: None,
            locked_forward: None,
        })
    }
    pub fn preview(
        &self,
        mut input: Input,
        point: Point3,
        normal: UnitVector3,
        _tolerance: Tolerance,
    ) -> Option<NurbsCurve> {
        if self.stage != ReferenceStage::Endpoint {
            return None;
        }
        let (p, t) = self.sample(point).ok()?;
        input.endpoint(p, t, normal).ok()?;
        input.default_curve()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.captured.current(doc)
    }
    pub fn stage(&self) -> ReferenceStage {
        self.stage
    }
    pub fn cursor(&self) -> &NurbsCurve {
        &self.cursor
    }
    pub fn curve(&self) -> CurveRef<'_> {
        self.working
            .as_ref()
            .map_or_else(|| self.captured.curve_ref(), Curve3::as_ref)
    }
    pub fn original(&self) -> CurveRef<'_> {
        self.captured.curve_ref()
    }
    pub fn source(&self) -> (&viboceros_document::GeometrySnapshot, Option<usize>) {
        self.captured.source()
    }
    pub fn trim_length(&self) -> Option<Real> {
        self.length
    }
    pub fn locked_forward(&self) -> Option<bool> {
        self.locked_forward
    }
    pub fn hover_parameter(&self) -> Option<Real> {
        self.hover
    }
    pub fn set_hover_parameter(&mut self, parameter: Real) -> Result<(), GeometryError> {
        if self.stage != ReferenceStage::TrimEnd || !self.original().domain().contains(&parameter) {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        self.hover = Some(parameter);
        Ok(())
    }
    pub fn hover_point(&mut self, point: Point3) -> Result<(), GeometryError> {
        let t = self
            .original()
            .closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)?;
        self.set_hover_parameter(t)
    }
    pub fn direction(&mut self, locked: bool) -> Result<(), CommandError> {
        if self.stage != ReferenceStage::TrimEnd {
            return Err(CommandError::Usage(
                "Direction requires a temporary subcurve end",
            ));
        }
        self.locked_forward = if locked {
            Some(crate::subcurve_input::cursor_forward(
                self.original(),
                self.start.unwrap(),
                self.hover.ok_or(CommandError::Usage(
                    "Move along the curve before locking direction",
                ))?,
            )?)
        } else {
            None
        };
        Ok(())
    }
    fn point_stations(&self, end: Real) -> Result<[Real; 2], GeometryError> {
        let start = self.start.ok_or(GeometryError::InvalidCurveTrimInterval)?;
        Ok(
            if self.locked_forward == Some(false) && self.original().is_closed()? {
                [end, start]
            } else {
                [start, end]
            },
        )
    }
    pub fn preview_range(&self) -> Result<Option<[Real; 2]>, GeometryError> {
        let Some(hover) = self.hover else {
            return Ok(None);
        };
        if self.length.is_some() {
            Ok(self.start.map(|start| [start, hover]))
        } else {
            self.point_stations(hover).map(Some)
        }
    }
    pub fn trim_start(&self) -> Option<Real> {
        self.start
    }
    pub fn project(&self, pick: Point3) -> Result<Point3, GeometryError> {
        let curve = self.curve();
        curve.evaluate(curve.closest_parameter(pick, Tolerance::NUMERICAL_VALIDATION)?)
    }
    pub fn sample(&self, pick: Point3) -> Result<(Point3, Vector3), GeometryError> {
        let curve = self.curve();
        let t = curve.closest_parameter(pick, Tolerance::NUMERICAL_VALIDATION)?;
        let s = curve.evaluate_with_tangent(t)?;
        Ok((s.point(), s.tangent().as_vector()))
    }
    pub fn trim_point(&mut self, point: Point3, tolerance: Tolerance) -> Result<(), CommandError> {
        let curve = self.captured.curve_ref();
        let parameter = curve.closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)?;
        match self.stage {
            ReferenceStage::TrimStart => {
                self.start = Some(parameter);
                self.stage = ReferenceStage::TrimEnd;
            }
            ReferenceStage::TrimEnd => {
                let interval = if let Some(length) = self.length {
                    if let Some(forward) = self.locked_forward {
                        crate::subcurve_input::interval_in_direction(
                            curve,
                            self.start.unwrap(),
                            length,
                            forward,
                            tolerance,
                        )?
                    } else {
                        crate::subcurve_input::interval(
                            curve,
                            self.start.unwrap(),
                            parameter,
                            length,
                            tolerance,
                        )?
                    }
                } else {
                    let [a, b] = self.point_stations(parameter)?;
                    crate::subcurve_input::point_parameters(curve, a, b, self.locked_forward, true)?
                };
                if let Some([a, b]) = interval {
                    self.set_working(Some(crate::curve_segments::directed(
                        &curve.to_owned(),
                        a,
                        b,
                    )?))?;
                } else {
                    self.stage = ReferenceStage::SelectCurve;
                }
            }
            ReferenceStage::SelectCurve => {
                return Err(CommandError::Usage("Select a curve or surface edge"));
            }
            ReferenceStage::Endpoint => return Err(CommandError::Usage(USAGE)),
        }
        Ok(())
    }
    pub fn length(&mut self, length: Real, tolerance: Tolerance) -> Result<(), CommandError> {
        if self.stage != ReferenceStage::TrimEnd
            || !length.is_finite()
            || length.abs() > self.captured.curve_ref().length(tolerance)?
        {
            return Err(CommandError::Usage(USAGE));
        }
        if length == 0. {
            self.stage = ReferenceStage::SelectCurve;
        } else {
            self.length = Some(length.abs());
            // Conic's single-curve getter asks for a confirmation for every
            // closed numeric portion; the standalone/multi-curve getter uses
            // a different chart-dependent input policy.
            if self.original().is_closed()? {
                self.locked_forward = None;
            } else if self.locked_forward.is_some() {
                let point = self.original().evaluate(self.start.unwrap())?;
                self.trim_point(point, tolerance)?;
            }
        }
        Ok(())
    }
    fn set_working(&mut self, working: Option<Curve3>) -> Result<(), GeometryError> {
        let cursor = working
            .as_ref()
            .map_or_else(|| self.original().to_nurbs(), |c| c.as_ref().to_nurbs())?;
        self.working = working;
        self.cursor = cursor;
        self.stage = ReferenceStage::Endpoint;
        Ok(())
    }
    pub fn full_curve(&mut self) {
        self.working = None;
        self.cursor = self.captured.curve().clone();
        self.stage = ReferenceStage::Endpoint;
        self.start = None;
        self.length = None;
        self.hover = None;
        self.locked_forward = None;
    }
}

#[cfg(test)]
mod tests;
