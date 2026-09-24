//! Principal-quadrant nearest and farthest points without ranking rounded world evaluations.
use super::*;
use crate::exact_scalar::{Rational, rational, scalar};
use num_traits::{Signed, Zero};

impl Ellipse3 {
    /// Closest native rational parameter. Reflection reduces the query to the
    /// first principal quadrant, where the stationary equation is monotone.
    /// Extremely disparate projected scales that underflow its coefficients
    /// are rejected rather than silently choosing an approximate extremum.
    pub fn closest_parameter(self, target: Point3) -> Result<Real, GeometryError> {
        let x = self
            .x_axis
            .as_vector()
            .dot_point_difference(target, self.center);
        let y = self
            .y_axis
            .as_vector()
            .dot_point_difference(target, self.center);
        require_finite([x, y], "ellipse closest-point projection")?;
        let [ax, by, begin, end] =
            coefficients(self.radius_x, self.radius_y, x.abs(), y.abs(), false)?;
        let u = if y == 0. && begin >= 0. {
            0.
        } else if x == 0. && end <= 0. {
            1.
        } else {
            // g = b²-a² + a*px/X - b*py/Y increases strictly as the
            // unit-circle point (X,Y) traverses the positive quadrant.
            // Endpoint limits handle axis queries, including the medial axis.
            let (mut low, mut high): (Real, Real) = (0., 1.);
            for _ in 0..1076 {
                let middle = low.midpoint(high);
                if middle == low || middle == high {
                    break;
                }
                let [cx, cy] = crate::curve_evaluate::ellipse_unit_jet(middle, 0.0..=4.0)?[0];
                // Algebraically equivalent forms avoid subtracting nearly
                // equal terms near the evolute's axis endpoints. In particular,
                // 1-X = Y²/(1+X) does not lose a small stationary offset.
                let value = if cx >= cy {
                    begin + ax * cy * (cy / (cx * (1. + cx))) - by / cy
                } else {
                    end + ax / cx - by * cx * (cx / (cy * (1. + cy)))
                };
                if value < 0. {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            low.midpoint(high)
        };
        reflected_parameter(self, x, y, u, false)
    }

    /// Farthest native rational parameter from a point in three dimensions.
    /// By reflection the maximizer is in the quadrant opposite the projected
    /// query. Its stationary equation is monotone across that quadrant.
    pub fn farthest_parameter(self, target: Point3) -> Result<Real, GeometryError> {
        let x = self
            .x_axis
            .as_vector()
            .dot_point_difference(target, self.center);
        let y = self
            .y_axis
            .as_vector()
            .dot_point_difference(target, self.center);
        require_finite([x, y], "ellipse farthest-point projection")?;
        let [ax, by, begin, end] =
            coefficients(self.radius_x, self.radius_y, x.abs(), y.abs(), true)?;
        let u = if y == 0. && begin <= 0. {
            0.
        } else if x == 0. && end >= 0. {
            1.
        } else {
            let (mut low, mut high): (Real, Real) = (0., 1.);
            for _ in 0..1076 {
                let middle = low.midpoint(high);
                if middle == low || middle == high {
                    break;
                }
                let [cx, cy] = crate::curve_evaluate::ellipse_unit_jet(middle, 0.0..=4.0)?[0];
                // g = b²-a² - a*|x|/cos(u) + b*|y|/sin(u). The
                // endpoint forms retain small offsets near either axis.
                let value = if cx >= cy {
                    begin - ax * cy * (cy / (cx * (1. + cx))) + by / cy
                } else {
                    end - ax / cx + by * cx * (cx / (cy * (1. + cy)))
                };
                if value > 0. {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            low.midpoint(high)
        };
        reflected_parameter(self, x, y, u, true)
    }
}

fn reflected_parameter(
    ellipse: Ellipse3,
    x: Real,
    y: Real,
    u: Real,
    opposite: bool,
) -> Result<Real, GeometryError> {
    // Zero projected coordinates admit reflected ties. Choose the first
    // native parameter, including the seam, independently of signed zero.
    let mut station: Real = 4.;
    for sx in [false, true] {
        if x != 0. && sx != (x.is_sign_negative() ^ opposite) {
            continue;
        }
        for sy in [false, true] {
            if y != 0. && sy != (y.is_sign_negative() ^ opposite) {
                continue;
            }
            let t = match (sx, sy) {
                (false, false) => u,
                (true, false) => 2. - u,
                (true, true) => 2. + u,
                (false, true) => 4. - u,
            };
            station = station.min(if t == 4. { 0. } else { t });
        }
    }
    crate::parameter::map_parameter(station, 0.0..=4.0, ellipse.domain())
}

fn coefficients(
    a: Real,
    b: Real,
    x: Real,
    y: Real,
    farthest: bool,
) -> Result<[Real; 4], GeometryError> {
    let (a, b) = (rational(a), rational(b));
    let (ax, by) = (&a * rational(x), &b * rational(y));
    let difference = &b * &b - &a * &a;
    let scale = &ax + &by + difference.abs();
    if scale.is_zero() {
        return Ok([0.; 4]);
    }
    let convert = |value: Rational| {
        let zero = value.is_zero();
        let rounded = scalar(&(value / &scale))?;
        if !zero && !rounded.is_normal() {
            return Err(GeometryError::Degenerate {
                context: "ellipse closest-point numerical range",
            });
        }
        Ok(rounded)
    };
    Ok(if farthest {
        [
            convert(ax.clone())?,
            convert(by.clone())?,
            convert(&difference - ax)?,
            convert(difference + by)?,
        ]
    } else {
        [
            convert(ax.clone())?,
            convert(by.clone())?,
            convert(&difference + ax)?,
            convert(difference - by)?,
        ]
    })
}

#[cfg(test)]
mod tests;
