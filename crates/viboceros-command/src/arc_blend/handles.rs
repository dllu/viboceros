//! Modal source-contact and chord-ratio adjustment without document edits.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Handle {
    Start,
    Ratio,
    End,
}
#[derive(Clone, Debug)]
pub(super) enum AdjustmentCurve {
    Source(usize),
    Chord(NurbsCurve),
}
impl Input {
    pub fn active_handle(&self) -> Option<Handle> {
        self.active_handle
    }
    pub fn handles(&self) -> Option<[Point3; 3]> {
        let c = self.curve.as_ref()?;
        let start = c.evaluate(*c.domain().start()).ok()?;
        let end = c.evaluate(*c.domain().end()).ok()?;
        let a = if self.geometry.ratio <= 1. {
            self.geometry.ratio / (1. + self.geometry.ratio)
        } else {
            1. / (1. + 1. / self.geometry.ratio)
        };
        let chord = start.vector_to(end).ok()?;
        Some([
            start,
            start
                .translated(
                    viboceros_geometry::Vector3::try_from(chord.to_array().map(|x| x * a)).ok()?,
                )
                .ok()?,
            end,
        ])
    }
    pub fn adjustment_curve(&self) -> Option<&NurbsCurve> {
        match self.adjustment_curve.as_ref()? {
            AdjustmentCurve::Source(i) => Some(self.guides.get(*i)?.source.curve()),
            AdjustmentCurve::Chord(c) => Some(c),
        }
    }
    /// A free mouse pick on the ratio chord uses closest viewing lines.
    /// Typed points and object snaps instead use ordinary 3D projection.
    pub fn ratio_parameter_in_view(
        &self,
        origin_offset: viboceros_geometry::Vector3,
        view_direction: viboceros_geometry::Vector3,
    ) -> Option<Real> {
        if self.active_handle != Some(Handle::Ratio) {
            return None;
        }
        let points = self.handles()?;
        let constraint = crate::translation::DestinationConstraint {
            anchor: points[0],
            direction: Some(points[0].direction_to(points[2]).ok()?),
            distance: None,
        };
        let point = constraint.project_view_line(origin_offset, view_direction)?;
        self.adjustment_curve()?
            .closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)
            .ok()
    }
    pub fn select_handle(&mut self, handle: Handle) -> Result<(), CommandError> {
        let points = self.handles().ok_or(CommandError::Usage(USAGE))?;
        let curve = match handle {
            Handle::Start => AdjustmentCurve::Source(0),
            Handle::End => AdjustmentCurve::Source(1),
            Handle::Ratio => AdjustmentCurve::Chord(
                viboceros_geometry::LineSegment::try_new(points[0], points[2], self.tolerance)?
                    .to_nurbs()?,
            ),
        };
        self.active_handle = Some(handle);
        self.adjustment_curve = Some(curve);
        Ok(())
    }
    pub fn finish_adjustment(&mut self) {
        self.active_handle = None;
        self.adjustment_curve = None;
    }
    /// Parameters belong to the original source, or the endpoint chord for the
    /// ratio handle. The selected curve is bounded, including for typed picks.
    pub fn adjust_parameter(&mut self, parameter: Real) -> Result<(), CommandError> {
        let c = self.adjustment_curve().ok_or(CommandError::Usage(USAGE))?;
        c.evaluate(parameter)?;
        let mut next = self.clone();
        match self.active_handle.ok_or(CommandError::Usage(USAGE))? {
            Handle::Start => next.guides[0].parameter = parameter,
            Handle::End => next.guides[1].parameter = parameter,
            Handle::Ratio => {
                let a =
                    (parameter - *c.domain().start()) / (*c.domain().end() - *c.domain().start());
                if a <= 0. || a >= 1. {
                    next.radius_difference = Some(next.last_radius_difference);
                } else {
                    next.radius_difference = None;
                    next.geometry.ratio = a / (1. - a);
                }
            }
        }
        next.rebuild()?;
        *self = next;
        Ok(())
    }
    /// Viewport constraints expose the rational display curve's parameters.
    /// Map those back before evaluating the original source's contact jet.
    pub fn adjust_constraint_parameter(&mut self, parameter: Real) -> Result<(), CommandError> {
        let parameter = match self
            .adjustment_curve
            .as_ref()
            .ok_or(CommandError::Usage(USAGE))?
        {
            AdjustmentCurve::Source(i) => self.guides[*i]
                .source
                .curve_ref()
                .parameter_from_nurbs(parameter)?,
            AdjustmentCurve::Chord(_) => parameter,
        };
        self.adjust_parameter(parameter)
    }
    pub fn adjust_point(&mut self, point: Point3) -> Result<(), CommandError> {
        let t = match self
            .adjustment_curve
            .as_ref()
            .ok_or(CommandError::Usage(USAGE))?
        {
            AdjustmentCurve::Source(i) => self.guides[*i]
                .source
                .curve_ref()
                .closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)?,
            AdjustmentCurve::Chord(c) => {
                c.closest_parameter(point, Tolerance::NUMERICAL_VALIDATION)?
            }
        };
        self.adjust_parameter(t)
    }
}
