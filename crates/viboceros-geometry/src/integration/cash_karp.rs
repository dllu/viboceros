//! Embedded Cash–Karp 5/4 quadrature with retained accepted checkpoints.
//!
//! This integrates a nonnegative, dimensionless speed on [0, 1]. The standard
//! tableau and step controller are also used by the Apache-2.0 SMLib ODE
//! integrator. Keeping this small scalar path in Rust permits diagnostics in
//! builds without the optional native kernel. It is not dense-output inversion.

use crate::{GeometryError, Real, require_finite};

pub(crate) const MAX_STEPS: usize = 65_536;
const A: [Real; 6] = [0., 1. / 5., 3. / 10., 3. / 5., 1., 7. / 8.];
const C: [Real; 6] = [37. / 378., 0., 250. / 621., 125. / 594., 0., 512. / 1771.];
const D: [Real; 6] = [
    C[0] - 2825. / 27648.,
    0.,
    C[2] - 18575. / 48384.,
    C[3] - 13525. / 55296.,
    -277. / 14336.,
    C[5] - 1. / 4.,
];

/// The same embedded rule for coupled states. The caller supplies its physical
/// error norm and acceptance policy; this function never accepts a step.
pub(crate) fn system_step<const N: usize>(
    x: Real,
    h: Real,
    state: [Real; N],
    mut rhs: impl FnMut(Real, [Real; N]) -> Result<[Real; N], GeometryError>,
) -> Result<([Real; N], [Real; N]), GeometryError> {
    require_finite([x, h], "Cash–Karp step inputs")?;
    const B: [[Real; 5]; 6] = [
        [0.; 5],
        [1. / 5., 0., 0., 0., 0.],
        [3. / 40., 9. / 40., 0., 0., 0.],
        [3. / 10., -9. / 10., 6. / 5., 0., 0.],
        [-11. / 54., 5. / 2., -70. / 27., 35. / 27., 0.],
        [
            1631. / 55296.,
            175. / 512.,
            575. / 13824.,
            44275. / 110592.,
            253. / 4096.,
        ],
    ];
    let mut values = [[0.; N]; 6];
    for i in 0..6 {
        let y = std::array::from_fn(|axis| {
            state[axis] + h * (0..i).map(|j| B[i][j] * values[j][axis]).sum::<Real>()
        });
        require_finite(y, "Cash–Karp state")?;
        let parameter = x + A[i] * h;
        require_finite([parameter], "Cash–Karp stage parameter")?;
        values[i] = rhs(parameter, y)?;
        require_finite(values[i], "Cash–Karp state derivative")?;
    }
    let output = std::array::from_fn(|axis| {
        state[axis] + h * (0..6).map(|i| C[i] * values[i][axis]).sum::<Real>()
    });
    let error =
        std::array::from_fn(|axis| h * (0..6).map(|i| D[i] * values[i][axis]).sum::<Real>());
    require_finite(output, "Cash–Karp output state")?;
    require_finite(error, "Cash–Karp state error")?;
    Ok((output, error))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Checkpoint {
    pub parameter: Real,
    pub integral: Real,
    pub error: Real,
}

pub(crate) struct Trace {
    pub checkpoints: Vec<Checkpoint>,
    pub attempted_steps: usize,
}

pub(crate) fn integrate(
    fractional_tolerance: Real,
    step_budget: usize,
    mut speed: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Trace, GeometryError> {
    if !fractional_tolerance.is_finite() || fractional_tolerance <= 0. || fractional_tolerance >= 1.
    {
        return Err(GeometryError::InvalidArcLengthIntegrationTolerance);
    }
    let mut checkpoints = vec![Checkpoint {
        parameter: 0.,
        integral: 0.,
        error: 0.,
    }];
    let (mut x, mut y, mut h): (Real, Real, Real) = (0., 0., 0.01);
    let mut attempted_steps = 0;
    while x < 1. {
        let first = checked_speed(speed(x)?)?;
        // Choose the scale before clipping the last proposed step. Retaining
        // this ordering makes the controller deterministic at the endpoint.
        let scale = y.abs() + (h * first).abs() + 1e-30;
        require_finite([scale], "Cash–Karp error scale")?;
        h = h.min(1. - x);
        let (increment, error, relative_error) = loop {
            if attempted_steps >= step_budget.min(MAX_STEPS) || x + h <= x {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            attempted_steps += 1;
            let (increment, error) = step(x, h, first, &mut speed)?;
            let relative_error = (error.abs() / scale) / fractional_tolerance;
            if relative_error <= 1. {
                break (increment, error, relative_error);
            }
            h = (0.9 * h * relative_error.powf(-0.25)).max(0.1 * h);
        };
        x = if h == 1. - x { 1. } else { x + h };
        y += increment;
        require_finite([x, y], "Cash–Karp integral")?;
        checkpoints.push(Checkpoint {
            parameter: x,
            integral: y,
            error: error.abs(),
        });
        h = if relative_error > 1.89e-4 {
            0.9 * h * relative_error.powf(-0.2)
        } else {
            5. * h
        };
    }
    Ok(Trace {
        checkpoints,
        attempted_steps,
    })
}

fn checked_speed(value: Real) -> Result<Real, GeometryError> {
    require_finite([value], "Cash–Karp speed")?;
    if value < 0. {
        return Err(GeometryError::NumericalIntegrationDidNotConverge);
    }
    Ok(value)
}

fn step(
    x: Real,
    h: Real,
    first: Real,
    speed: &mut impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<(Real, Real), GeometryError> {
    let mut values = [first; 6];
    for i in 1..6 {
        values[i] = checked_speed(speed((x + A[i] * h).min(1.))?)?;
    }
    let increment = h * C.iter().zip(values).map(|(c, v)| c * v).sum::<Real>();
    let error = h * D.iter().zip(values).map(|(d, v)| d * v).sum::<Real>();
    require_finite([increment, error], "Cash–Karp step")?;
    Ok((increment, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coupled_step_follows_a_harmonic_oscillator() {
        let h: Real = 0.1;
        let (value, error) = system_step(0., h, [1., 0.], |_, s| Ok([s[1], -s[0]])).unwrap();
        assert!((value[0] - h.cos()).abs() < 2e-9, "{value:?} {error:?}");
        assert!((value[1] + h.sin()).abs() < 2e-9, "{value:?} {error:?}");
    }

    #[test]
    fn polynomial_quadrature_retains_endpoints_and_accepted_steps() {
        let trace = integrate(1e-8, MAX_STEPS, |x| Ok(3. * x * x)).unwrap();
        assert_eq!(trace.checkpoints[0].parameter, 0.);
        assert_eq!(trace.checkpoints.last().unwrap().parameter, 1.);
        assert!((trace.checkpoints.last().unwrap().integral - 1.).abs() < 1e-14);
        assert!(
            trace
                .checkpoints
                .windows(2)
                .all(|p| { p[0].parameter < p[1].parameter && p[0].integral <= p[1].integral })
        );
        assert!(
            trace
                .checkpoints
                .iter()
                .all(|p| p.error.is_finite() && p.error >= 0.)
        );
    }

    #[test]
    fn rational_circle_refines_to_analytic_length_without_using_oracle_nodes() {
        let w = 0.5_f64.sqrt();
        let speed = |t: Real| Ok(2. * w / ((1. - t).powi(2) + 2. * w * t * (1. - t) + t * t));
        let coarse = integrate(1e-2, MAX_STEPS, speed).unwrap();
        let fine = integrate(1e-12, MAX_STEPS, speed).unwrap();
        let expected = std::f64::consts::FRAC_PI_2;
        assert_eq!(coarse.checkpoints.len(), 5);
        assert!(fine.checkpoints.len() > coarse.checkpoints.len());
        assert!(fine.attempted_steps > fine.checkpoints.len() - 1);
        assert!((coarse.checkpoints.last().unwrap().integral - expected).abs() > 1e-5);
        assert!((fine.checkpoints.last().unwrap().integral - expected).abs() < 2e-11);
    }

    #[test]
    fn invalid_speeds_tolerances_and_budget_propagate_as_errors() {
        for tolerance in [0., -1., 1., Real::NAN, Real::INFINITY] {
            assert_eq!(
                integrate(tolerance, MAX_STEPS, |_| Ok(1.)).err(),
                Some(GeometryError::InvalidArcLengthIntegrationTolerance)
            );
        }
        for speed in [-1., Real::NAN, Real::INFINITY] {
            assert!(integrate(1e-8, MAX_STEPS, |_| Ok(speed)).is_err());
        }
        assert_eq!(
            integrate(1e-8, 0, |_| Ok(1.)).err(),
            Some(GeometryError::NumericalIntegrationDidNotConverge)
        );
        assert_eq!(
            integrate(1e-8, 1, |_| Ok(1.)).err(),
            Some(GeometryError::NumericalIntegrationDidNotConverge)
        );
        let error = GeometryError::Degenerate {
            context: "test speed",
        };
        assert_eq!(
            integrate(1e-8, MAX_STEPS, |_| Err(error.clone())).err(),
            Some(error)
        );
    }
}
