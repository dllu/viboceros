//! Numeric getter orientation and open-end clamping, separate from length math.
use viboceros_geometry::{Curve3, CurveRef, GeometryError, Real, Tolerance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SubcurveMode {
    #[default]
    Shorten,
    MarkEnds,
}
impl SubcurveMode {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim_start_matches('_');
        if value.eq_ignore_ascii_case("Shorten") {
            Some(Self::Shorten)
        } else if value.eq_ignore_ascii_case("MarkEnds") {
            Some(Self::MarkEnds)
        } else {
            None
        }
    }
    pub fn option(self) -> &'static str {
        match self {
            Self::Shorten => "Shorten",
            Self::MarkEnds => "MarkEnds",
        }
    }
}

/// Standalone SubCrv retains full closed traversals and source orientation.
pub fn piece(
    curve: CurveRef<'_>,
    anchor: Real,
    confirmation: Real,
    length: Real,
    tolerance: Tolerance,
) -> Result<Option<Curve3>, GeometryError> {
    if length == 0. {
        return Ok(None);
    }
    if !length.is_finite() || length.abs() > curve.length(tolerance)? {
        return Err(GeometryError::InvalidCurveTrimInterval);
    }
    if curve.is_closed()? && length.abs() == curve.length(tolerance)? {
        return curve
            .to_owned()
            .try_subcurve_at_arc_length(anchor, length.abs(), tolerance);
    }
    let Some([start, end]) = interval(curve, anchor, confirmation, length.abs(), tolerance)? else {
        return Ok(None);
    };
    Ok(Some(curve.to_owned().try_subcurve(start, end)?))
}

pub fn interval(
    curve: CurveRef<'_>,
    anchor: Real,
    confirmation: Real,
    length: Real,
    tolerance: Tolerance,
) -> Result<Option<[Real; 2]>, GeometryError> {
    let domain = curve.domain();
    let closed = curve.is_closed()?;
    if closed && length == curve.length(tolerance)? {
        return Ok(None);
    }
    let forward = if closed {
        confirmation != anchor
            && curve
                .to_owned()
                .try_subcurve(anchor, confirmation)?
                .as_ref()
                .length(tolerance)?
                <= curve.length(tolerance)? * 0.5
    } else {
        confirmation > anchor
    };
    let signed = if forward { length } else { -length };
    let result = curve
        .to_owned()
        .try_subcurve_at_arc_length_with_endpoint(anchor, signed, tolerance)?;
    let endpoint = if let Some((_, endpoint)) = result {
        endpoint
    } else if forward {
        *domain.end()
    } else {
        *domain.start()
    };
    Ok(Some(if forward {
        [anchor, endpoint]
    } else {
        [endpoint, anchor]
    }))
}

/// FromMidpoint uses the entered magnitude as the distance on each side.
/// Open sides clamp independently; closed endpoints wrap in the original chart.
pub fn midpoint_piece(
    curve: CurveRef<'_>,
    center: Real,
    radius: Real,
    tolerance: Tolerance,
) -> Result<Option<Curve3>, GeometryError> {
    if !center.is_finite() || !curve.domain().contains(&center) {
        return Err(GeometryError::InvalidCurveTrimInterval);
    }
    if radius == 0. {
        return Ok(None);
    }
    let radius = radius.abs();
    if !radius.is_finite() || radius > curve.length(tolerance)? {
        return Err(GeometryError::InvalidCurveTrimInterval);
    }
    let source = curve.to_owned();
    let domain = curve.domain();
    if curve.is_closed()? {
        let total = curve.length(tolerance)?;
        if radius == total || radius == total * 0.5 {
            return Ok(None);
        }
    }
    let left = source
        .try_subcurve_at_arc_length_with_endpoint(center, -radius, tolerance)?
        .map(|(_, t)| t)
        .unwrap_or(*domain.start());
    let right = source
        .try_subcurve_at_arc_length_with_endpoint(center, radius, tolerance)?
        .map(|(_, t)| t)
        .unwrap_or(*domain.end());
    if left == right {
        return Ok(None);
    }
    Ok(Some(source.try_subcurve(left, right)?))
}

pub fn midpoint_radius(
    curve: CurveRef<'_>,
    center: Real,
    end: Real,
    tolerance: Tolerance,
) -> Result<Real, GeometryError> {
    if center == end {
        return Ok(0.);
    }
    let length = curve
        .to_owned()
        .try_subcurve(center, end)?
        .as_ref()
        .length(tolerance)?;
    Ok(if curve.is_closed()? {
        length.min(curve.length(tolerance)? - length)
    } else {
        length
    })
}
