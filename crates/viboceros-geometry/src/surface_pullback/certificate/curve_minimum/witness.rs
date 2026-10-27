//! Feasible witnesses from exact original-curve evaluation; numerical proposals are optional.
use super::*;
pub(super) struct Best {
    pub(super) result: CurveMinimumDistance,
    pub(super) square: Rational,
    pub(super) direction: P,
}

pub(super) struct Search<'a> {
    pub(super) curves: [&'a NurbsCurve; 2],
    pub(super) splines: [curve::Spline<4>; 2],
    pub(super) best: Option<Best>,
    pub(super) budget: Budget,
}
impl Search<'_> {
    pub(super) fn candidate(
        &mut self,
        parameters: [Real; 2],
        sides: [ParameterSide; 2],
        limit: Real,
    ) -> Result<(), GeometryError> {
        let mut exact = Vec::new();
        for i in 0..2 {
            let domain = self.curves[i].domain();
            let t = (rational(parameters[i]) - rational(*domain.start()))
                / (rational(*domain.end()) - rational(*domain.start()));
            if t < Rational::zero() || t > Rational::one() {
                return Ok(());
            }
            let h = self.splines[i].value_on_side(&t, sides[i], &mut self.budget)?;
            let p: P = std::array::from_fn(|j| &h[j] / &h[3]);
            for x in &p {
                self.budget.check(x)?;
            }
            exact.push(p);
        }
        let direction: P = std::array::from_fn(|i| &exact[0][i] - &exact[1][i]);
        let square: Rational = direction.iter().map(|x| x * x).sum();
        self.budget.check(&square)?;
        if self.best.as_ref().is_some_and(|b| {
            square > b.square || (square == b.square && parameters >= b.result.parameters)
        }) {
            return Ok(());
        }
        let approximate = direction.iter().map(scalar).collect::<Result<Vec<_>, _>>();
        let Ok(v) = approximate else {
            return Ok(());
        };
        let mut upper = v[0].hypot(v[1]).hypot(v[2]);
        if !upper.is_finite() {
            return Ok(());
        }
        let h = [
            direction[0].clone(),
            direction[1].clone(),
            direction[2].clone(),
            Rational::one(),
        ];
        let mut found = None;
        for _ in 0..8 {
            if let Some(bound) = algebra::norm_bound(&h, upper) {
                found = Some(bound);
                break;
            }
            upper = upper.next_up();
            if !upper.is_finite() {
                break;
            }
        }
        let Some(upper_distance) = found else {
            return Ok(());
        };
        let point = |p: &P| -> Result<Point3, GeometryError> {
            Point3::try_new(scalar(&p[0])?, scalar(&p[1])?, scalar(&p[2])?)
        };
        self.best = Some(Best {
            result: CurveMinimumDistance {
                parameters,
                sides,
                points: [point(&exact[0])?, point(&exact[1])?],
                upper_distance,
                maximum_error: limit,
            },
            square,
            direction,
        });
        Ok(())
    }
    pub(super) fn native(&self, i: usize, fraction: &Rational) -> Result<Real, GeometryError> {
        let d = self.curves[i].domain();
        scalar(&(rational(*d.start()) + fraction * (rational(*d.end()) - rational(*d.start()))))
    }
    pub(super) fn proposals(&mut self, tolerance: Tolerance) -> Result<(), GeometryError> {
        let queries = [
            self.curves[0].prepare_closest_parameter_search().ok(),
            self.curves[1].prepare_closest_parameter_search().ok(),
        ];
        for i in 0..2 {
            for f in [0., 0.5, 1.] {
                let mut p = [
                    *self.curves[0].domain().start(),
                    *self.curves[1].domain().start(),
                ];
                p[i] = self.curves[i].parameter_at(f)?;
                for _ in 0..8 {
                    let before = p;
                    for j in [1 - i, i] {
                        if let (Some(query), Ok(point)) =
                            (&queries[j], self.curves[1 - j].evaluate(p[1 - j]))
                            && let Ok(t) = query.closest_parameter(point, tolerance)
                        {
                            p[j] = t;
                        }
                    }
                    self.candidate(p, [ParameterSide::Right; 2], tolerance.absolute())?;
                    if p == before {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}
