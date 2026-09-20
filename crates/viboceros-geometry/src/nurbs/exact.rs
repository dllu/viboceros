//! Exact homogeneous recurrences shared by guarded curve and surface evaluators.
use crate::{GeometryError, NurbsCurve, Real, Vector3, require_finite};
pub(crate) use num_rational::BigRational as Rational;
use num_traits::{One, ToPrimitive, Zero};

pub(crate) type Homogeneous<const D: usize = 4> = [Rational; D];

pub(crate) fn rational(value: Real) -> Rational {
    Rational::from_float(value).expect("validated finite geometry scalar")
}

pub(crate) fn scalar(value: &Rational) -> Result<Real, GeometryError> {
    let value = value.to_f64().ok_or(GeometryError::NonFinite {
        context: "exact rational projection",
    })?;
    require_finite([value], "exact rational projection")?;
    Ok(value)
}

pub(crate) fn vector(values: &[Rational; 3]) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        scalar(&values[0])?,
        scalar(&values[1])?,
        scalar(&values[2])?,
    )
}

pub(crate) fn curve_controls(curve: &NurbsCurve, span: usize) -> Vec<Homogeneous> {
    curve.control_points()[span - curve.degree()..=span]
        .iter()
        .map(|control| {
            let w = rational(control.weight());
            let p = control.point();
            [
                rational(p.x()) * &w,
                rational(p.y()) * &w,
                rational(p.z()) * &w,
                w,
            ]
        })
        .collect()
}

/// Evaluates an exact parameter, including stations with no binary64 native
/// representation. The supplied span determines the side at a multiple knot.
pub(crate) fn evaluate_at<const D: usize>(
    knots: &[Real],
    degree: usize,
    span: usize,
    parameter: &Rational,
    mut work: Vec<Homogeneous<D>>,
) -> Result<Homogeneous<D>, GeometryError> {
    for level in 1..=degree {
        for local in (level..=degree).rev() {
            let index = span - degree + local;
            let left = rational(knots[index]);
            let width = rational(knots[index + degree - level + 1]) - &left;
            if width <= Rational::zero() {
                return Err(GeometryError::InvalidKnotVector {
                    context: "exact de Boor interval",
                });
            }
            let alpha = (parameter - left) / width;
            let complement = Rational::one() - &alpha;
            work[local] = std::array::from_fn(|i| {
                &work[local - 1][i] * &complement + &work[local][i] * &alpha
            });
        }
    }
    Ok(work.swap_remove(degree))
}

#[derive(Clone, Copy)]
pub(crate) struct Direction<'a> {
    pub(crate) knots: &'a [Real],
    pub(crate) degree: usize,
    pub(crate) span: usize,
    pub(crate) parameter: Real,
}

impl Direction<'_> {
    pub(crate) fn differentiated(self) -> Self {
        Self {
            knots: &self.knots[1..self.knots.len() - 1],
            degree: self.degree - 1,
            span: self.span - 1,
            ..self
        }
    }

    pub(crate) fn evaluate<const D: usize>(
        self,
        work: Vec<Homogeneous<D>>,
    ) -> Result<Homogeneous<D>, GeometryError> {
        evaluate_at(
            self.knots,
            self.degree,
            self.span,
            &rational(self.parameter),
            work,
        )
    }

    pub(crate) fn derivative_controls<const D: usize>(
        self,
        net: &[Homogeneous<D>],
        width: usize,
        along_u: bool,
    ) -> Result<Vec<Homogeneous<D>>, GeometryError> {
        let height = net.len() / width;
        let (output_width, output_height) = if along_u {
            (width - 1, height)
        } else {
            (width, height - 1)
        };
        let mut output = Vec::with_capacity(output_width * output_height);
        for j in 0..output_height {
            for i in 0..output_width {
                let index = self.span - self.degree + if along_u { i } else { j };
                let interval =
                    rational(self.knots[index + self.degree + 1]) - rational(self.knots[index + 1]);
                if interval <= Rational::zero() {
                    return Err(GeometryError::InvalidKnotVector {
                        context: "exact derivative interval",
                    });
                }
                let factor = Rational::from_integer(self.degree.into()) / interval;
                let lower = &net[j * width + i];
                let upper = &net[j * width + i + if along_u { 1 } else { width }];
                output.push(std::array::from_fn(|k| (&upper[k] - &lower[k]) * &factor));
            }
        }
        Ok(output)
    }
}
