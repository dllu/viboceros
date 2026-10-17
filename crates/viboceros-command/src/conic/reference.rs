//! Immutable curve/edge capture and the common temporary SubCrv operations.
use super::*;
use crate::curve_reference::CapturedCurve;
use viboceros_document::ObjectId;
use viboceros_geometry::Curve3;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceStage {
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
        })
    }
    pub fn preview(
        &self,
        mut input: Input,
        point: Point3,
        normal: UnitVector3,
        tolerance: Tolerance,
    ) -> Option<NurbsCurve> {
        if self.stage == ReferenceStage::TrimEnd {
            let mut r = self.clone();
            r.trim_point(point, tolerance).ok()?;
            return Some(r.cursor);
        }
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
    pub fn trim_start(&self) -> Option<Real> {
        self.start
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
                    crate::subcurve_input::interval(
                        curve,
                        self.start.unwrap(),
                        parameter,
                        length,
                        tolerance,
                    )?
                } else {
                    crate::subcurve_input::point_parameters(
                        curve,
                        self.start.unwrap(),
                        parameter,
                        None,
                        true,
                    )?
                };
                let working = if let Some([a, b]) = interval {
                    Some(crate::curve_segments::directed(&curve.to_owned(), a, b)?)
                } else {
                    None
                };
                let cursor = working
                    .as_ref()
                    .map_or_else(|| curve.to_nurbs(), |c| c.as_ref().to_nurbs())?;
                self.working = working;
                self.cursor = cursor;
                self.stage = ReferenceStage::Endpoint;
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
            self.full_curve();
        } else {
            self.length = Some(length.abs());
        }
        Ok(())
    }
    pub fn full_curve(&mut self) {
        self.working = None;
        self.cursor = self.captured.curve().clone();
        self.stage = ReferenceStage::Endpoint;
        self.start = None;
        self.length = None;
    }
}
