//! A temporary directed piece never replaces its captured document parent.
use super::*;
use crate::curve_reference::CurvePortion;
use std::sync::Arc;
use viboceros_geometry::CurveRef;
#[derive(Clone, Debug)]
pub(super) struct Guide {
    pub(super) source: CapturedCurve,
    portion: Option<Arc<CurvePortion>>,
    pub(super) parameter: Real,
    pub(super) end: bool,
}
impl Guide {
    pub(super) fn capture(
        doc: &Document,
        id: ObjectId,
        edge: Option<usize>,
        pick: Option<Point3>,
        range: Option<([Real; 2], bool)>,
    ) -> Result<Self, CommandError> {
        let source = CapturedCurve::capture(doc, id, edge)?;
        let portion = range
            .map(|(parameters, wrap)| {
                CurvePortion::prepare(source.curve_ref(), parameters, wrap).map(Arc::new)
            })
            .transpose()?;
        let c = portion
            .as_ref()
            .map_or_else(|| source.curve_ref(), |p| p.curve_ref());
        if c.is_closed()? {
            return Err(CommandError::Usage("ArcBlend requires open curve ends"));
        }
        let end = if portion.is_some() {
            true
        } else {
            let pick = pick.ok_or(CommandError::Usage(USAGE))?;
            pick.distance_to(c.evaluate(*c.domain().end())?)?
                < pick.distance_to(c.evaluate(*c.domain().start())?)?
        };
        let parameter = if end {
            *c.domain().end()
        } else {
            *c.domain().start()
        };
        Ok(Self {
            source,
            portion,
            parameter,
            end,
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
    pub(super) fn argument(&self) -> Result<String, CommandError> {
        let id = self.source.object_id();
        let edge = self.source.source().1;
        if let Some(p) = &self.portion {
            return Ok(p.argument(&self.source));
        }
        let binding = edge.map_or_else(|| format!("Curve={id}"), |i| format!("Edge={id},{i}"));
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
