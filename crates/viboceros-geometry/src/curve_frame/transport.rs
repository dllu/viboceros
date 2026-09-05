use super::*;
use crate::CurveSample;
use std::collections::HashMap;

pub(super) struct Evaluator<'a> {
    curve: CurveRef<'a>,
    maximum: usize,
    cache: HashMap<(u64, bool), CurveSample>,
}

impl<'a> Evaluator<'a> {
    pub(super) fn new(curve: CurveRef<'a>, maximum: usize) -> Self {
        Self {
            curve,
            maximum,
            cache: HashMap::new(),
        }
    }
    pub(super) fn sample(
        &mut self,
        t: Real,
        side: ParameterSide,
    ) -> Result<CurveSample, GeometryError> {
        let key = (t.to_bits(), side == ParameterSide::Left);
        if let Some(s) = self.cache.get(&key) {
            return Ok(*s);
        }
        if self.cache.len() >= self.maximum {
            return Err(GeometryError::CurveFrameResourceLimit {
                maximum: self.maximum,
            });
        }
        let sample = self.curve.evaluate_with_tangent_on_side(t, side)?;
        self.cache.insert(key, sample);
        Ok(sample)
    }
    /// Richardson extrapolation of symmetric tangent-sphere transports:
    /// shortest great-circle steps have second-order global error. Comparing
    /// one step with two half steps cancels their leading twist error. A
    /// three-row table first tries a sixth-order candidate; a fourth row
    /// supplies an eighth-order candidate only if needed. The preceding
    /// two entries estimate the correction in either case.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn advance(
        &mut self,
        a: Real,
        b: Real,
        ta: UnitVector3,
        tb: UnitVector3,
        x: UnitVector3,
        budget: Real,
        depth: usize,
    ) -> Result<UnitVector3, GeometryError> {
        let m = a * 0.5 + b * 0.5;
        if !(a < m && m < b) {
            // Requested sites can lie immediately beside a natural seed. At
            // floating-point resolution there is no interior evaluation. Use
            // the minimum rotation only for a roundoff-sized turn whose
            // squared-turn indicator fits the allocated error budget. This
            // does not certify unobservable sub-ULP tangent excursions.
            let sine = ta.as_vector().cross(tb.as_vector())?.length()?;
            if sine <= 64.0 * Real::EPSILON
                && sine * sine <= budget
                && ta.as_vector().dot(tb.as_vector())? > 0.0
            {
                return minimal_rotation(x, ta, tb)
                    .ok_or(GeometryError::CurveFrameDidNotConverge)?;
            }
            return Err(GeometryError::CurveFrameDidNotConverge);
        }
        if depth >= 48 {
            return Err(GeometryError::CurveFrameDidNotConverge);
        }
        let mut nodes = [ta; 9];
        nodes[8] = tb;
        for (i, node) in nodes.iter_mut().enumerate().take(8).skip(2).step_by(2) {
            let fraction = i as Real / 8.0;
            *node = self
                .sample(a * (1.0 - fraction) + b * fraction, ParameterSide::Right)?
                .tangent();
        }
        let smooth = [ta, nodes[2], nodes[4], nodes[6], tb].windows(2).all(|p| {
            p[0].as_vector()
                .dot(p[1].as_vector())
                .is_ok_and(|dot| dot > 0.9)
        });
        if smooth {
            let (fine, error) = extrapolate(x, &nodes, 3)?;
            if error <= budget {
                return Ok(fine);
            }
            for i in [1, 3, 5, 7] {
                let fraction = i as Real / 8.0;
                nodes[i] = self
                    .sample(a * (1.0 - fraction) + b * fraction, ParameterSide::Right)?
                    .tangent();
            }
            let (fine, error) = extrapolate(x, &nodes, 4)?;
            if error <= budget {
                return Ok(fine);
            }
        }
        let middle = self.advance(a, m, ta, nodes[4], x, budget * 0.5, depth + 1)?;
        self.advance(m, b, nodes[4], tb, middle, budget * 0.5, depth + 1)
    }
}

fn extrapolate(
    x: UnitVector3,
    nodes: &[UnitVector3; 9],
    levels: usize,
) -> Result<(UnitVector3, Real), GeometryError> {
    let mut previous = [x; 4];
    for level in 0..levels {
        let stride = 8 >> level;
        let mut result = x;
        for i in (stride..=8).step_by(stride) {
            result = minimal_rotation(result, nodes[i - stride], nodes[i])
                .ok_or(GeometryError::CurveFrameDidNotConverge)??;
        }
        let mut row = [result; 4];
        for column in 1..=level {
            let divisor = ((1 << (2 * column)) - 1) as Real;
            row[column] = rotate(
                row[column - 1],
                nodes[8],
                twist(previous[column - 1], row[column - 1], nodes[8])? / divisor,
            )?;
        }
        if level == levels - 1 {
            let divisor = ((1 << (2 * level)) - 1) as Real;
            return Ok((
                row[level],
                twist(previous[level - 1], row[level - 1], nodes[8])?.abs() / divisor,
            ));
        }
        previous = row;
    }
    unreachable!("three or four extrapolation levels")
}

pub(super) fn minimal_rotation(
    x: UnitVector3,
    a: UnitVector3,
    b: UnitVector3,
) -> Option<Result<UnitVector3, GeometryError>> {
    let dot = a.as_vector().dot(b.as_vector()).ok()?;
    if dot <= -1.0 + 64.0 * Real::EPSILON {
        return None;
    }
    Some((|| {
        let factor = x.as_vector().dot(b.as_vector())? / (1.0 + dot);
        let xa = x.as_vector().to_array();
        let aa = a.as_vector().to_array();
        let ba = b.as_vector().to_array();
        perpendicular(
            Vector3::try_from(std::array::from_fn(|i| {
                (-factor).mul_add(aa[i] + ba[i], xa[i])
            }))?,
            b,
        )
    })())
}

fn twist(a: UnitVector3, b: UnitVector3, tangent: UnitVector3) -> Result<Real, GeometryError> {
    let sine = a
        .as_vector()
        .cross(b.as_vector())?
        .dot(tangent.as_vector())?;
    Ok(sine.atan2(a.as_vector().dot(b.as_vector())?))
}

fn rotate(x: UnitVector3, tangent: UnitVector3, angle: Real) -> Result<UnitVector3, GeometryError> {
    let (s, c) = angle.sin_cos();
    let y = tangent.as_vector().cross(x.as_vector())?.to_array();
    let x = x.as_vector().to_array();
    Vector3::try_from(std::array::from_fn(|i| c.mul_add(x[i], s * y[i])))?.normalized_nonzero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tangent_sphere_extrapolation_has_high_order_helix_convergence() {
        let unit = |a| Vector3::try_from(a).unwrap().normalized_nonzero().unwrap();
        let h: Real = 0.7;
        let speed = 1.0_f64.hypot(h);
        let tangent = |t: Real| unit([-t.sin(), t.cos(), h]);
        let end: Real = 4.7;
        let n = [-end.cos(), -end.sin(), 0.0];
        let b = [h * end.sin() / speed, -h * end.cos() / speed, 1.0 / speed];
        let angle = h * end / speed;
        let expected: [Real; 3] = std::array::from_fn(|i| angle.cos() * n[i] - angle.sin() * b[i]);
        let mut previous = Real::INFINITY;
        for count in [2, 4, 8] {
            let mut x = unit([-1.0, 0.0, 0.0]);
            for i in 0..count {
                let a = end * i as Real / count as Real;
                let b = end * (i + 1) as Real / count as Real;
                let nodes = std::array::from_fn(|j| tangent(a + (b - a) * j as Real / 8.0));
                x = extrapolate(x, &nodes, 4).unwrap().0;
            }
            let error = x
                .as_vector()
                .to_array()
                .into_iter()
                .zip(expected)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<Real>()
                .sqrt();
            assert!(
                previous / error > 150.0,
                "error {error}, previous {previous}"
            );
            previous = error;
        }
        assert!(previous < 1e-10);
    }
}
