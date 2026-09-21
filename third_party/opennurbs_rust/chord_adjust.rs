// Adapted from AdjustPointListAlongChord / AdjustNurbsCurve in
// opennurbs_brep.cpp, OpenNURBS 23fc677ba06e49212296ca75fab7fb6c2851b4ce.
// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// See LICENSE and README.md in this directory. This is a modified Rust port,
// not upstream OpenNURBS: projections and displacements use exact rationals.

use crate::exact_scalar::{Rational, rational, scalar};
use crate::{GeometryError, Point3, Real};

/// Preserve each control's chord coordinate while interpolating the two end
/// displacements. Short chords relative to the control polygon use end-only
/// editing. The caller retains weights/knots and certifies displacement.
pub(crate) fn adjust(
    points: &[Point3],
    targets: [Point3; 2],
) -> Result<Vec<Point3>, GeometryError> {
    let first = points[0];
    let last = points[points.len() - 1];
    let mut result = points.to_vec();
    result[0] = targets[0];
    *result.last_mut().unwrap() = targets[1];
    let closed = first == last && targets[0] == targets[1];
    let chord_length = first.distance_to(last).unwrap_or(Real::INFINITY);
    let polygon_length = points
        .windows(2)
        .map(|p| p[0].distance_to(p[1]).unwrap_or(Real::INFINITY))
        .sum::<Real>();
    if !closed && (chord_length < Real::EPSILON.sqrt() || chord_length < 0.01 * polygon_length) {
        return Ok(result);
    }
    let a = first.to_array().map(rational);
    let b = last.to_array().map(rational);
    let d: [Rational; 3] = std::array::from_fn(|i| &b[i] - &a[i]);
    let length_squared: Rational = d.iter().map(|v| v * v).sum();
    let delta0: [Rational; 3] = std::array::from_fn(|i| rational(targets[0].to_array()[i]) - &a[i]);
    let delta1: [Rational; 3] = std::array::from_fn(|i| rational(targets[1].to_array()[i]) - &b[i]);
    for i in 1..points.len() - 1 {
        let p = points[i].to_array().map(rational);
        let fraction = if closed {
            rational(0.)
        } else {
            (0..3).map(|j| (&p[j] - &a[j]) * &d[j]).sum::<Rational>() / &length_squared
        };
        let mut point = [0.; 3];
        for j in 0..3 {
            point[j] = scalar(
                &(&p[j] + (rational(1.) - &fraction) * &delta0[j] + &fraction * &delta1[j]),
            )?;
        }
        result[i] = Point3::try_from(point)?;
    }
    Ok(result)
}
