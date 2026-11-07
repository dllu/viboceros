use super::*;
#[derive(Clone, Debug)]
pub(super) struct Guide {
    pub source: CapturedCurve,
    pub parameter: Real,
    pub end: bool,
    pub flip: bool,
}
impl Guide {
    pub(super) fn capture(
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Point3,
        at_pick: bool,
    ) -> Result<Self, CommandError> {
        let source = CapturedCurve::capture(doc, id, edge)?;
        let c = source.curve_ref();
        if c.is_closed()? {
            return Err(CommandError::Usage(
                "BlendCrv requires an open source curve",
            ));
        }
        let end = !at_pick
            && pick.distance_to(c.evaluate(*c.domain().end())?)?
                < pick.distance_to(c.evaluate(*c.domain().start())?)?;
        let parameter = if at_pick {
            c.closest_parameter(pick, doc.tolerance())?
        } else if end {
            *c.domain().end()
        } else {
            *c.domain().start()
        };
        Ok(Self {
            source,
            parameter,
            end,
            flip: false,
        })
    }
    pub(super) fn point(&self) -> Result<Point3, CommandError> {
        Ok(self.source.curve_ref().evaluate(self.parameter)?)
    }
    pub(super) fn side(&self) -> ParameterSide {
        if self.end ^ self.flip {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        }
    }
    pub(super) fn direction(&self, i: usize) -> Result<UnitVector3, CommandError> {
        let t = self
            .source
            .curve_ref()
            .evaluate_with_tangent_on_side(self.parameter, self.side())?
            .tangent();
        Ok(if ((i == 0) == self.end) ^ self.flip {
            t
        } else {
            t.opposite()
        })
    }
    pub(super) fn jet(&self, i: usize, continuity: u8) -> Result<CurveBlendJet, CommandError> {
        if continuity >= 3 {
            let (_, derivatives) = self
                .source
                .curve_ref()
                .evaluate_with_fourth_derivatives_on_side(self.parameter, self.side())?;
            let reversed = !(((i == 0) == self.end) ^ self.flip);
            let jet = CurveBlendJet::from_curve_derivatives(self.point()?, derivatives, reversed)?;
            // The captured Native G4 default has a zero fourth derivative,
            // including on nonzero-fourth-derivative rational/quintic sources.
            return Ok(jet.with_fourth_derivative(Vector3::try_new(0., 0., 0.)?));
        }
        let direction = match self.direction(i) {
            Ok(d) => Some(d),
            Err(_) if continuity == 0 => None,
            Err(e) => return Err(e),
        };
        let curvature = if continuity >= 2 {
            self.source
                .curve_ref()
                .curvature_vector_on_side(self.parameter, self.side())?
        } else {
            Vector3::try_new(0., 0., 0.)?
        };
        Ok(CurveBlendJet::new(self.point()?, direction, curvature))
    }
    pub(super) fn binding(&self) -> Result<String, CommandError> {
        let id = self.source.object_id();
        let binding = self
            .source
            .source()
            .1
            .map_or_else(|| format!("Curve={id}"), |e| format!("Edge={id},{e}"));
        let c = self.source.curve_ref();
        let p = c.evaluate(if self.end {
            *c.domain().end()
        } else {
            *c.domain().start()
        })?;
        Ok(format!(
            "{binding} {}",
            p.to_array().map(|v| v.to_string()).join(",")
        ))
    }
}
