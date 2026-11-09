use crate::{CurveOffsetCornerStyle, GeometryError, NurbsCurve, Point3, UnitVector3};

/// Corner construction for ordinary offset ribbons.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RibbonCornerStyle {
    #[default]
    Sharp,
    Round,
    Smooth,
    Chamfer,
    None,
}

impl From<CurveOffsetCornerStyle> for RibbonCornerStyle {
    fn from(value: CurveOffsetCornerStyle) -> Self {
        match value {
            CurveOffsetCornerStyle::Sharp => Self::Sharp,
            CurveOffsetCornerStyle::Round => Self::Round,
            CurveOffsetCornerStyle::Chamfer => Self::Chamfer,
            CurveOffsetCornerStyle::None => Self::None,
        }
    }
}

impl RibbonCornerStyle {
    pub(super) fn layout_style(self) -> CurveOffsetCornerStyle {
        match self {
            Self::Sharp => CurveOffsetCornerStyle::Sharp,
            Self::Round => CurveOffsetCornerStyle::Round,
            Self::Smooth | Self::Chamfer => CurveOffsetCornerStyle::Chamfer,
            Self::None => CurveOffsetCornerStyle::None,
        }
    }
}

/// A cubic tangent blend with chord*(3-dot(incoming,outgoing))/6 handles.
/// The surrounding strips provide oriented unit tangents; no source lengths
/// enter this local fan.
pub(super) fn smooth_corner(
    start: Point3,
    end: Point3,
    incoming: UnitVector3,
    outgoing: UnitVector3,
) -> Result<NurbsCurve, GeometryError> {
    let cosine = incoming
        .as_vector()
        .dot(outgoing.as_vector())?
        .clamp(-1., 1.);
    let handle = start.distance_to(end)? * ((3. - cosine) / 6.);
    NurbsCurve::try_new(
        3,
        vec![
            start,
            start.translated(incoming.as_vector().scaled(handle)?)?,
            end.translated(outgoing.as_vector().scaled(-handle)?)?,
            end,
        ],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
}
