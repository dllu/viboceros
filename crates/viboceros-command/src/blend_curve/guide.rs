use super::*;
use crate::curve_reference::CurvePortion;
use std::sync::Arc;
use viboceros_geometry::CurveRef;
#[derive(Clone, Debug)]
pub(super) struct Guide {
    pub source: CapturedCurve,
    portion: Option<Arc<CurvePortion>>,
    pub parameter: Real,
    pub end: bool,
    pub flip: bool,
    pub surface: Option<Arc<super::surface::SurfaceGuide>>,
}
impl Guide {
    pub(super) fn capture(
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
        at_pick: bool,
        range: Option<([Real; 2], bool)>,
        surface_mode: bool,
    ) -> Result<Self, CommandError> {
        let source = CapturedCurve::capture(doc, id, edge)?;
        let surface = surface_mode
            .then(|| {
                super::surface::SurfaceGuide::capture(
                    doc,
                    id,
                    edge.ok_or(CommandError::Usage("Select a surface edge"))?,
                )
            })
            .transpose()?
            .map(Arc::new);
        let portion = range
            .map(|(parameters, wrap)| {
                CurvePortion::prepare(source.curve_ref(), parameters, wrap).map(Arc::new)
            })
            .transpose()?;
        let c = portion
            .as_ref()
            .map_or_else(|| source.curve_ref(), |p| p.curve_ref());
        if surface.is_none() && c.is_closed()? {
            return Err(CommandError::Usage(
                "BlendCrv requires an open source curve",
            ));
        }
        let end = if surface.is_some() {
            false
        } else if portion.is_some() {
            !at_pick
        } else {
            let pick = pick.ok_or(CommandError::Usage(USAGE))?;
            !at_pick
                && pick.distance_to(c.evaluate(*c.domain().end())?)?
                    < pick.distance_to(c.evaluate(*c.domain().start())?)?
        };
        let parameter = if surface.is_some() || (at_pick && portion.is_none()) {
            let pick = pick.ok_or(CommandError::Usage(USAGE))?;
            c.closest_parameter(pick, doc.tolerance())?
        } else if end || portion.is_some() {
            *c.domain().end()
        } else {
            *c.domain().start()
        };
        Ok(Self {
            source,
            portion,
            parameter,
            end,
            flip: false,
            surface,
        })
    }
    pub(super) fn temporary(&self) -> bool {
        self.portion.is_some()
    }
    pub(super) fn curve_ref(&self) -> CurveRef<'_> {
        self.portion
            .as_ref()
            .map_or_else(|| self.source.curve_ref(), |p| p.curve_ref())
    }
    pub(super) fn display(&self) -> &NurbsCurve {
        self.portion
            .as_ref()
            .map_or_else(|| self.source.curve(), |p| p.display())
    }
    pub(super) fn point(&self) -> Result<Point3, CommandError> {
        Ok(self.curve_ref().evaluate(self.parameter)?)
    }
    pub(super) fn side(&self) -> ParameterSide {
        if self.end ^ self.flip {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        }
    }
    pub(super) fn direction(&self, i: usize) -> Result<UnitVector3, CommandError> {
        if let Some(surface) = &self.surface {
            return surface.direction(self, i);
        }
        let t = self
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
        if let Some(surface) = &self.surface {
            return surface.jet(self, i, continuity);
        }
        if continuity >= 3 {
            let (_, derivatives) = self
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
            self.curve_ref()
                .curvature_vector_on_side(self.parameter, self.side())?
        } else {
            Vector3::try_new(0., 0., 0.)?
        };
        Ok(CurveBlendJet::new(self.point()?, direction, curvature))
    }
    pub(super) fn binding(&self) -> Result<String, CommandError> {
        if self.surface.is_some() {
            return Ok(format!(
                "Edges Edge={},{} {} Curves",
                self.source.object_id(),
                self.source.source().1.unwrap(),
                self.point()?.to_array().map(|v| v.to_string()).join(",")
            ));
        }
        if let Some(portion) = &self.portion {
            let binding = portion.argument(&self.source);
            // arguments() starts ordinary bindings in CurveEnd mode. Restore the
            // private portion's captured direction before its source is parsed.
            return Ok(if self.end {
                binding
            } else {
                format!("BlendStart=PickPoint {binding} BlendStart=CurveEnd")
            });
        }
        let id = self.source.object_id();
        let binding = self
            .source
            .source()
            .1
            .map_or_else(|| format!("Curve={id}"), |e| format!("Edge={id},{e}"));
        let c = self.curve_ref();
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
