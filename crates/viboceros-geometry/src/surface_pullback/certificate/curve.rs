//! Exact polar-form restrictions, independent of curve knot domains.
use super::*;

pub(super) struct Spline<const D: usize> {
    degree: usize,
    knots: Vec<Rational>,
    controls: Vec<[Rational; D]>,
}
impl<const D: usize> Spline<D> {
    pub fn cuts(&self) -> Vec<Rational> {
        self.knots
            .iter()
            .filter(|k| **k >= Rational::zero() && **k <= Rational::one())
            .cloned()
            .collect()
    }
    pub fn extract(
        &self,
        left: &Rational,
        right: &Rational,
        budget: &mut Budget,
    ) -> Result<Vec<[Rational; D]>, GeometryError> {
        let middle = (left + right) / rational(2.);
        let span = self.knots.partition_point(|k| *k <= middle) - 1;
        extract(
            &self.knots,
            self.degree,
            span,
            &self.controls[span - self.degree..=span],
            left,
            right,
            budget,
        )
    }
}
impl Spline<3> {
    pub fn uv(curve: &NurbsCurve2, budget: &mut Budget) -> Result<Option<Self>, GeometryError> {
        budget.charge(curve.control_points().len() + curve.knots().len())?;
        let gauge = curve.control_points()[0].weight();
        if curve
            .control_points()
            .iter()
            .any(|c| c.weight().is_sign_negative() != gauge.is_sign_negative())
        {
            return Ok(None);
        }
        let controls = curve
            .control_points()
            .iter()
            .map(|c| {
                let w = rational(c.weight()) / rational(gauge);
                [
                    rational(c.point().x()) * &w,
                    rational(c.point().y()) * &w,
                    w,
                ]
            })
            .collect::<Vec<_>>();
        let domain = curve.domain();
        Ok(Some(Self {
            degree: curve.degree(),
            knots: normalize(curve.knots(), [*domain.start(), *domain.end()]),
            controls,
        }))
    }
}
impl Spline<4> {
    pub fn spatial(curve: &NurbsCurve, budget: &mut Budget) -> Result<Option<Self>, GeometryError> {
        budget.charge(curve.control_points().len() + curve.knots().len())?;
        let gauge = curve.control_points()[0].weight();
        if curve
            .control_points()
            .iter()
            .any(|c| c.weight().is_sign_negative() != gauge.is_sign_negative())
        {
            return Ok(None);
        }
        let controls = curve
            .control_points()
            .iter()
            .map(|c| {
                let w = rational(c.weight()) / rational(gauge);
                std::array::from_fn(|i| {
                    if i == 3 {
                        w.clone()
                    } else {
                        rational(c.point().to_array()[i]) * &w
                    }
                })
            })
            .collect();
        let domain = curve.domain();
        Ok(Some(Self {
            degree: curve.degree(),
            knots: normalize(curve.knots(), [*domain.start(), *domain.end()]),
            controls,
        }))
    }
}
fn normalize(knots: &[Real], domain: [Real; 2]) -> Vec<Rational> {
    let start = rational(domain[0]);
    let length = rational(domain[1]) - &start;
    knots
        .iter()
        .map(|k| (rational(*k) - &start) / &length)
        .collect()
}

pub(super) fn extract<const D: usize>(
    knots: &[Rational],
    degree: usize,
    span: usize,
    controls: &[[Rational; D]],
    left: &Rational,
    right: &Rational,
    budget: &mut Budget,
) -> Result<Vec<[Rational; D]>, GeometryError> {
    if knots[span - degree..=span].iter().all(|k| k == left)
        && knots[span + 1..=span + degree + 1]
            .iter()
            .all(|k| k == right)
    {
        budget.charge(D * controls.len())?;
        return Ok(controls.to_vec());
    }
    budget.charge(D * (degree + 1).pow(3))?;
    let mut output = Vec::with_capacity(degree + 1);
    for right_arguments in 0..=degree {
        let mut work = controls.to_vec();
        for level in 1..=degree {
            let t = if level <= degree - right_arguments {
                left
            } else {
                right
            };
            for j in (level..=degree).rev() {
                let k = span - degree + j;
                let alpha = (t - &knots[k]) / (&knots[k + degree - level + 1] - &knots[k]);
                let beta = Rational::one() - &alpha;
                work[j] = std::array::from_fn(|axis| {
                    &beta * &work[j - 1][axis] + &alpha * &work[j][axis]
                });
                for r in &work[j] {
                    budget.check(r)?;
                }
            }
        }
        output.push(work.swap_remove(degree));
    }
    Ok(output)
}

pub(super) fn bounds<const D: usize>(
    net: &[[Rational; D]],
    budget: &mut Budget,
) -> Result<Vec<[Rational; 2]>, GeometryError> {
    budget.charge(D * net.len())?;
    let mut bounds = (0..D - 1)
        .map(|i| {
            let v = &net[0][i] / &net[0][D - 1];
            [v.clone(), v]
        })
        .collect::<Vec<_>>();
    for h in net {
        for (axis, bound) in bounds.iter_mut().enumerate() {
            let value = &h[axis] / &h[D - 1];
            budget.check(&value)?;
            bound[0] = bound[0].clone().min(value.clone());
            bound[1] = bound[1].clone().max(value);
        }
    }
    Ok(bounds)
}
