//! Surface curvature metrics and false colors used by CurvatureAnalysis.
// Rust adaptation of ON_SurfaceCurvature::{KappaValue,MinimumRadius,MaximumRadius}
// and ON_SurfaceCurvatureColorMapping::Color; altered source.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED. See third_party/opennurbs/LICENSE.
use crate::{GeometryError, Real, require_finite};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CurvatureStyle {
    #[default]
    Gaussian,
    Mean,
    MinRadius,
    MaxRadius,
}

impl CurvatureStyle {
    pub const ALL: [Self; 4] = [Self::Gaussian, Self::Mean, Self::MinRadius, Self::MaxRadius];
    pub fn name(self) -> &'static str {
        match self {
            Self::Gaussian => "Gaussian",
            Self::Mean => "Mean",
            Self::MinRadius => "MinRadius",
            Self::MaxRadius => "MaxRadius",
        }
    }
    pub fn value(self, principal: [Real; 2]) -> Option<Real> {
        let [a, b] = principal;
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let radius = |k: Real| if k > 1e-300 { 1. / k } else { 1e300 };
        Some(match self {
            Self::Gaussian => a * b,
            Self::Mean => (if a == b { a } else { 0.5 * (a + b) }).abs(),
            Self::MinRadius => radius(a.abs().max(b.abs())),
            // A saddle has zero directional curvature between its principal
            // axes, hence an infinite maximum radius even if neither k is zero.
            Self::MaxRadius => {
                if a.signum() * b.signum() <= 0. || a.abs() <= 1e-300 || b.abs() <= 1e-300 {
                    1e300
                } else {
                    radius(a.abs().min(b.abs()))
                }
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvatureColorMapping {
    style: CurvatureStyle,
    range: [Real; 2],
}
impl CurvatureColorMapping {
    /// The first endpoint is blue and the second red. Reversed and singleton
    /// intervals are supported. Radius styles use model length units.
    pub fn try_new(style: CurvatureStyle, range: [Real; 2]) -> Result<Self, GeometryError> {
        require_finite(range, "curvature color range")?;
        Ok(Self { style, range })
    }
    pub fn style(self) -> CurvatureStyle {
        self.style
    }
    pub fn range(self) -> [Real; 2] {
        self.range
    }
    pub fn color(self, principal: [Real; 2]) -> Option<[u8; 3]> {
        let k = self.style.value(principal)?;
        let [min, max] = self.range;
        let t = if min != max {
            ((max - k) * (1. / (max - min))).clamp(0., 1.)
        } else if k > min {
            0.
        } else if k < min {
            1.
        } else {
            0.5
        };
        t.is_finite()
            .then(|| crate::analysis_color::saturated_hue(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saddle_max_radius_detects_a_flat_direction_and_mean_is_unsigned() {
        assert_eq!(CurvatureStyle::MaxRadius.value([2., -1.]), Some(1e300));
        assert_eq!(CurvatureStyle::MinRadius.value([2., -1.]), Some(0.5));
        assert_eq!(CurvatureStyle::Mean.value([-2., -1.]), Some(1.5));
        assert_eq!(CurvatureStyle::MinRadius.value([0., 0.]), Some(1e300));
    }
}
