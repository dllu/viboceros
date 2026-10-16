//! Private C2 arc-length-to-output proposals; these never certify geometry.
use super::*;

pub(super) struct Clock {
    pub pieces: Vec<NurbsCurve>,
    /// Output-parameter derivative with respect to local model arc length.
    pub slopes: Vec<f64>,
}

impl Clock {
    pub fn new(stations: &[CurveFitStation], lengths: &[f64]) -> Result<Self, Error> {
        let secants = stations
            .windows(2)
            .zip(lengths)
            .map(|(p, &length)| (p[1].fit_parameter - p[0].fit_parameter) / length)
            .collect::<Vec<_>>();
        if lengths.iter().any(|s| !s.is_finite() || *s <= 0.)
            || secants.iter().any(|s| !s.is_finite() || *s <= 0.)
        {
            return Err(invalid("collapsed arc-length clock"));
        }
        let mut slopes = vec![secants[0]];
        slopes.extend(secants.windows(2).map(|p| p[0].min(p[1])));
        slopes.push(*secants.last().unwrap());
        let mut pieces = Vec::with_capacity(lengths.len());
        for (j, &length) in lengths.iter().enumerate() {
            let a = stations[j].fit_parameter;
            let b = stations[j + 1].fit_parameter;
            let left = length * slopes[j] / 5.;
            let right = length * slopes[j + 1] / 5.;
            // Quintic Hermite pieces share first derivatives and zero second
            // derivatives at each contact. The smaller adjacent secant keeps
            // all Bernstein increments positive, proving monotonicity of each
            // stored proposal. Decline floating collapse rather than clipping.
            let controls = [a, a + left, a + 2. * left, b - 2. * right, b - right, b];
            if controls.iter().any(|s| !s.is_finite()) || controls.windows(2).any(|p| p[0] >= p[1])
            {
                return Err(invalid("nonmonotone stored arc-length clock"));
            }
            pieces.push(NurbsCurve::try_new(
                5,
                controls
                    .into_iter()
                    .map(|x| Point3::try_new(x, 0., 0.))
                    .collect::<Result<Vec<_>, _>>()?,
                vec![0., 0., 0., 0., 0., 0., 1., 1., 1., 1., 1., 1.],
            )?);
        }
        Ok(Self { pieces, slopes })
    }
}
