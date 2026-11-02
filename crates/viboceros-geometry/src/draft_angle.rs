//! Draft-angle colors, following OpenNURBS's public color mapping.
//!
//! Rust adaptation of ON_SurfaceDraftAngleColorMapping::Color and ON_Color::SetHSV;
//! this is altered source, not the original OpenNURBS implementation.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
// See third_party/opennurbs/LICENSE and https://www.opennurbs.org for notices.
use crate::{GeometryError, Real, UnitVector3, Vector3, require_finite};

/// Saturated red/yellow/green/cyan/blue colors for oriented surface normals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DraftAngleMapping {
    up: Vector3,
    range_degrees: [Real; 2],
    dot_range: [Real; 2],
}

impl DraftAngleMapping {
    /// Endpoints may increase, decrease, or coincide, and must be within ±90°.
    pub fn try_new(up: Vector3, range_degrees: [Real; 2]) -> Result<Self, GeometryError> {
        require_finite(range_degrees, "draft angle range")?;
        if range_degrees.iter().any(|a| a.abs() > 90.) {
            return Err(GeometryError::Degenerate {
                context: "draft angle range outside ±90°",
            });
        }
        up.normalized_nonzero()?;
        Ok(Self {
            up,
            range_degrees,
            // Use the same expression as OpenNURBS, including the rounding
            // around singleton ranges and its 1e-8 near-singleton threshold.
            dot_range: range_degrees.map(|a| (std::f64::consts::FRAC_PI_2 - a.to_radians()).cos()),
        })
    }

    pub fn up(self) -> Vector3 {
        self.up
    }
    pub fn range_degrees(self) -> [Real; 2] {
        self.range_degrees
    }

    pub fn angle_degrees(self, normal: UnitVector3) -> Real {
        let up = self.up.normalized_nonzero().expect("validated nonzero up");
        (normal.x() * up.x() + normal.y() * up.y() + normal.z() * up.z())
            .clamp(-1., 1.)
            .asin()
            .to_degrees()
    }

    fn dot(self, normal: Vector3) -> Real {
        normal.x() * self.up.x() + normal.y() * self.up.y() + normal.z() * self.up.z()
    }

    /// The normal must already be unit length. Preserve stored mesh-normal
    /// rounding, as OpenNURBS does, rather than normalizing a second time.
    pub fn color(self, normal: Vector3) -> [u8; 3] {
        let dot = self.dot(normal);
        let [a, b] = self.dot_range;
        let t = if (a - b).abs() <= 1e-8 {
            if dot < a && dot < b {
                0.
            } else if dot > a && dot > b {
                1.
            } else {
                0.5
            }
        } else {
            ((dot - a) / (b - a)).clamp(0., 1.)
        };
        let hue = (4. * std::f64::consts::PI / 3. * t) * (3. / std::f64::consts::PI);
        let rgb = match hue.floor() as u8 {
            0 => [1., hue, 0.],
            1 => [2. - hue, 1., 0.],
            2 => [0., 1., hue - 2.],
            3 => [0., 4. - hue, 1.],
            _ => [0., 0., 1.],
        };
        rgb.map(|v: Real| (255. * v).round() as u8)
    }
}
