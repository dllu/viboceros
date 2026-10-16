//! Attained curve distances and an analytic correspondence for two lines.
use crate::exact_scalar::{Rational, rational, scalar};
use crate::{CurveRef, GeometryError, LineSegment, Point3};
use num_traits::Zero;
mod overlap;
pub use overlap::preserve_transverse_curve_ends;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct CurveDistanceWitness {
    /// Fractions in each source's NURBS chart, preserving the requested visit.
    pub normalized_parameters: [f64; 2],
    pub parameters: [f64; 2],
    pub points: [Point3; 2],
    pub distance: f64,
}

#[derive(Clone, Debug)]
pub struct CurveDistanceExtrema {
    pub intervals: [[f64; 2]; 2],
    pub maximum: CurveDistanceWitness,
    pub minimum: CurveDistanceWitness,
}

#[derive(Clone, Debug)]
pub struct LineDeviation {
    pub extrema: CurveDistanceExtrema,
    /// Corresponding interval ends; the second source can run backwards.
    pub ends: [CurveDistanceWitness; 2],
}

/// Compare corresponding locations on two lines in planes perpendicular to
/// their aligned direction bisector. This measures line deviation, rather than
/// the Hausdorff distance between finite segments. Perpendicular directions
/// (within 64 machine epsilons) have no distinguished alignment. Intervals
/// touching at only one projected location do not overlap.
///
/// Projection, clipping, and the quadratic minimum use exact binary64 inputs
/// after choosing the numerical bisector. Witnesses are evaluated on the
/// original sources, retaining their native parameter intervals.
pub fn line_deviation(
    first: LineSegment,
    second: LineSegment,
) -> Result<Option<LineDeviation>, GeometryError> {
    let u = first
        .start()
        .vector_to(first.end())?
        .normalized_nonzero()?
        .as_vector();
    let v = second
        .start()
        .vector_to(second.end())?
        .normalized_nonzero()?
        .as_vector();
    let dot = u.dot(v)?;
    if dot.abs() <= 64. * f64::EPSILON {
        return Ok(None);
    }
    let sign = if dot < 0. { -1. } else { 1. };
    let axis = std::array::from_fn::<_, 3, _>(|i| {
        rational(u.to_array()[i]) + rational(sign) * rational(v.to_array()[i])
    });
    let origin = first.start().to_array().map(rational);
    let project = |p: Point3| -> Rational {
        p.to_array()
            .map(rational)
            .iter()
            .zip(&origin)
            .zip(&axis)
            .map(|((p, o), a)| (p - o) * a)
            .sum()
    };
    let q = [
        [Rational::zero(), project(first.end())],
        [project(second.start()), project(second.end())],
    ];
    let low = q[0][0]
        .clone()
        .min(q[0][1].clone())
        .max(q[1][0].clone().min(q[1][1].clone()));
    let high = q[0][0]
        .clone()
        .max(q[0][1].clone())
        .min(q[1][0].clone().max(q[1][1].clone()));
    if low >= high {
        return Ok(None);
    }
    let fractions = [&low, &high].map(|s| -> Result<[f64; 2], GeometryError> {
        Ok([
            scalar(&((s - &q[0][0]) / (&q[0][1] - &q[0][0])))?.clamp(0., 1.),
            scalar(&((s - &q[1][0]) / (&q[1][1] - &q[1][0])))?.clamp(0., 1.),
        ])
    });
    let [f0, f1] = fractions;
    let fractions = [f0?, f1?];
    let witness = |f: [f64; 2]| -> Result<CurveDistanceWitness, GeometryError> {
        let points = [first.point_at(f[0])?, second.point_at(f[1])?];
        Ok(CurveDistanceWitness {
            normalized_parameters: f,
            parameters: [
                CurveRef::Line(&first).parameter_at(f[0])?,
                CurveRef::Line(&second).parameter_at(f[1])?,
            ],
            points,
            distance: points[0].distance_to(points[1])?,
        })
    };
    let ends = [witness(fractions[0])?, witness(fractions[1])?];
    let residual = ends.each_ref().map(|w| {
        std::array::from_fn::<_, 3, _>(|i| {
            rational(w.points[1].to_array()[i]) - rational(w.points[0].to_array()[i])
        })
    });
    let delta = std::array::from_fn::<_, 3, _>(|i| &residual[1][i] - &residual[0][i]);
    let denominator: Rational = delta.iter().map(|v| v * v).sum();
    let t = if denominator.is_zero() {
        0.
    } else {
        let numerator: Rational = residual[0].iter().zip(&delta).map(|(a, b)| -(a * b)).sum();
        scalar(
            &(numerator / denominator)
                .max(Rational::zero())
                .min(rational(1.)),
        )?
    };
    let candidate = witness(std::array::from_fn(|i| {
        (fractions[1][i] - fractions[0][i]).mul_add(t, fractions[0][i])
    }))?;
    let mut minimum = ends[0].clone();
    for w in [&ends[1], &candidate] {
        if w.distance < minimum.distance {
            minimum = w.clone();
        }
    }
    let maximum = ends[usize::from(ends[1].distance > ends[0].distance)].clone();
    Ok(Some(LineDeviation {
        extrema: CurveDistanceExtrema {
            intervals: std::array::from_fn(|i| {
                [
                    fractions[0][i].min(fractions[1][i]),
                    fractions[0][i].max(fractions[1][i]),
                ]
            }),
            maximum,
            minimum,
        },
        ends,
    }))
}
