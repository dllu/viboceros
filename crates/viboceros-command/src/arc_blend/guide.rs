//! A temporary directed piece never replaces its captured document parent.
use super::*;
use std::sync::Arc;
use viboceros_geometry::{Curve3, CurveRef};
#[derive(Clone, Debug)]
struct Portion {
    parameters: [Real; 2],
    wrap: bool,
    curve: Curve3,
    display: NurbsCurve,
}
#[derive(Clone, Debug)]
pub(super) struct Guide {
    pub(super) source: CapturedCurve,
    portion: Option<Arc<Portion>>,
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
                let curve = prepare_subcurve(source.curve_ref(), parameters, wrap)?;
                let display = curve.as_ref().to_nurbs()?;
                Ok::<_, CommandError>(Arc::new(Portion {
                    parameters,
                    wrap,
                    curve,
                    display,
                }))
            })
            .transpose()?;
        let c = portion
            .as_ref()
            .map_or_else(|| source.curve_ref(), |p| p.curve.as_ref());
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
            .map_or_else(|| self.source.curve_ref(), |p| p.curve.as_ref())
    }
    pub(super) fn display(&self) -> &NurbsCurve {
        self.portion
            .as_ref()
            .map_or_else(|| self.source.curve(), |p| &p.display)
    }
    pub(super) fn argument(&self) -> Result<String, CommandError> {
        let id = self.source.object_id();
        let edge = self.source.source().1;
        if let Some(p) = &self.portion {
            let [a, b] = p.parameters;
            let key = if p.wrap { "SubCrvWrap" } else { "SubCrv" };
            return Ok(edge.map_or_else(
                || format!("{key}={id},{a},{b}"),
                |i| format!("{key}={id},Edge,{i},{a},{b}"),
            ));
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

/// Point selections reverse the bounded portion; numeric lengths may cross the
/// closed seam. Both paths use the shared native/portable trimming operations.
pub fn prepare_subcurve(
    curve: CurveRef<'_>,
    [a, b]: [Real; 2],
    wrap: bool,
) -> Result<Curve3, CommandError> {
    if a == b {
        return Err(viboceros_geometry::GeometryError::InvalidCurveTrimInterval.into());
    }
    let source = curve.to_owned();
    if wrap {
        return crate::curve_segments::directed(&source, a, b);
    }
    let piece = crate::curve_segments::trimmed(&source, a.min(b)..=a.max(b))?;
    Ok(if a > b {
        piece.reversed(Tolerance::NUMERICAL_VALIDATION)?
    } else {
        piece
    })
}
