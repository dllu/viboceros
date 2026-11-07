//! Prepared private portions keep their parent identity outside the geometry.
use super::*;
use crate::CommandError;
use viboceros_geometry::{Curve3, Real};
#[derive(Clone, Debug)]
pub struct CurvePortion {
    parameters: [Real; 2],
    wrap: bool,
    curve: Curve3,
    display: NurbsCurve,
}
impl CurvePortion {
    pub fn prepare(
        source: CurveRef<'_>,
        parameters: [Real; 2],
        wrap: bool,
    ) -> Result<Self, CommandError> {
        let curve = prepare_subcurve(source, parameters, wrap)?;
        let display = curve.as_ref().to_nurbs()?;
        Ok(Self {
            parameters,
            wrap,
            curve,
            display,
        })
    }
    pub fn curve_ref(&self) -> CurveRef<'_> {
        self.curve.as_ref()
    }
    pub fn display(&self) -> &NurbsCurve {
        &self.display
    }
    pub fn argument(&self, source: &CapturedCurve) -> String {
        let id = source.object_id();
        let [a, b] = self.parameters;
        let key = if self.wrap { "SubCrvWrap" } else { "SubCrv" };
        source.source().1.map_or_else(
            || format!("{key}={id},{a},{b}"),
            |i| format!("{key}={id},Edge,{i},{a},{b}"),
        )
    }
}
/// Bounded point order reverses a portion. Directed numeric intervals may cross
/// a closed seam. Both paths retain the shared native/portable trimming path.
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

#[derive(Clone, Copy, Debug)]
pub struct PortionBinding {
    pub object: ObjectId,
    pub edge: Option<usize>,
    pub parameters: [Real; 2],
    pub wrap: bool,
}
/// Recognize an explicit native-parameter temporary-source binding.
pub fn parse_portion_binding(text: &str) -> Option<Result<PortionBinding, CommandError>> {
    let (key, value) = text.split_once('=')?;
    let wrap = crate::option_name_eq(key, "SubCrvWrap");
    if !wrap && !crate::option_name_eq(key, "SubCrv") {
        return None;
    }
    Some((|| {
        let fields = value.split(',').collect::<Vec<_>>();
        let (object, edge, a, b) = match fields.as_slice() {
            [id, a, b] => (*id, None, *a, *b),
            [id, tag, index, a, b] if crate::option_name_eq(tag, "Edge") => (
                *id,
                Some(
                    index
                        .parse()
                        .map_err(|_| CommandError::Usage("Invalid SubCrv edge index"))?,
                ),
                *a,
                *b,
            ),
            _ => {
                return Err(CommandError::Usage(
                    "SubCrv=id,start,end or SubCrv=id,Edge,index,start,end",
                ));
            }
        };
        Ok(PortionBinding {
            object: object
                .parse()
                .map_err(|_| CommandError::Usage("Invalid SubCrv object"))?,
            edge,
            parameters: [crate::parse_finite_real(a)?, crate::parse_finite_real(b)?],
            wrap,
        })
    })())
}
