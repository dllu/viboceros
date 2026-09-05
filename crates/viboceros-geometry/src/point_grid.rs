//! Ordered control nets and tensor point-grid interpolation.

use crate::{Brep, GeometryError, NurbsSurface, Point3, Real, Tolerance, require_finite};
use faer::{Mat, prelude::*};

mod basis;
mod orientation;
#[cfg(test)]
mod tests;

/// Bounds both dense factorization size and the tensor data allocation.
pub const MAX_POINT_GRID_AXIS_COUNT: usize = 256;
/// Rhino's public point-grid helpers clamp requested degrees to this value.
pub const MAX_POINT_GRID_DEGREE: usize = 11;

impl Brep {
    /// Control-grid command construction, with shared edges at genuine creases.
    pub fn try_control_point_grid(
        points: &[Point3],
        count: [usize; 2],
        degree: [usize; 2],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let surface = NurbsSurface::try_control_point_grid(points, count, degree)?;
        command_brep(&surface, tolerance)
    }

    /// Point-grid command construction, retaining periodic seams and creases.
    pub fn try_through_point_grid(
        points: &[Point3],
        count: [usize; 2],
        degree: [usize; 2],
        closed: [bool; 2],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let surface = NurbsSurface::try_through_point_grid(points, count, degree, closed)?;
        command_brep(&surface, tolerance)
    }
}

fn command_brep(surface: &NurbsSurface, tolerance: Tolerance) -> Result<Brep, GeometryError> {
    let [u, v] = surface.sampled_kink_parameters(0.1_f64.to_radians())?;
    let brep = Brep::try_surface_grid(surface, &u, &v, tolerance)?;
    let inward = if brep.is_solid() {
        match orientation::is_inward(surface)? {
            Some(inward) => inward,
            None => brep.signed_volume(tolerance)? < 0.0,
        }
    } else {
        false
    };
    if inward {
        Ok(brep.reversed())
    } else {
        Ok(brep)
    }
}

impl NurbsSurface {
    /// Builds an open, unit-knot-spaced control net. Input points use U-fast
    /// order (`points[v * count_u + u]`), like every native tensor surface.
    /// Degrees are clamped to `1..=min(11, count - 1)` independently.
    pub fn try_control_point_grid(
        points: &[Point3],
        count: [usize; 2],
        degree: [usize; 2],
    ) -> Result<Self, GeometryError> {
        let degree = validate(points, count, degree, [false; 2])?;
        let knots = std::array::from_fn::<_, 2, _>(|axis| {
            (0..count[axis] + degree[axis] + 1)
                .map(|i| {
                    i.saturating_sub(degree[axis])
                        .min(count[axis] - degree[axis]) as Real
                })
                .collect()
        });
        let [u, v] = knots;
        Self::try_new(
            degree[0],
            degree[1],
            count[0],
            count[1],
            points.to_vec(),
            u,
            v,
        )
    }

    /// Constructs Rhino-style point-grid geometry from U-fast input points.
    /// Open directions interpolate at mean-chord parameters. Closed directions
    /// of degree greater than one instead constrain the first independent
    /// Greville sites, including *boundary-span continuation outside the active
    /// domain*. Thus not all input points necessarily lie on the active patch.
    /// This compatibility policy is not unconditional periodic interpolation.
    pub fn try_through_point_grid(
        points: &[Point3],
        count: [usize; 2],
        degree: [usize; 2],
        closed: [bool; 2],
    ) -> Result<Self, GeometryError> {
        let degree = validate(points, count, degree, closed)?;
        let [u, v] = [
            basis::Direction::new(points, count, 0, degree[0], closed[0])?,
            basis::Direction::new(points, count, 1, degree[1], closed[1])?,
        ];
        let origin = points[0].to_array();
        let mut scale = 0.0_f64;
        for p in points {
            for (a, b) in p.to_array().into_iter().zip(origin) {
                let delta = a - b;
                require_finite([delta], "point grid local coordinates")?;
                scale = scale.max(delta.abs());
            }
        }
        if scale == 0.0 {
            return Err(GeometryError::InvalidPointGrid {
                context: "coincident points",
            });
        }
        let targets = Mat::from_fn(count[0], count[1] * 3, |i, j| {
            (points[j / 3 * count[0] + i].to_array()[j % 3] - origin[j % 3]) / scale
        });
        let first = u.solve(&targets)?;
        let targets = Mat::from_fn(count[1], count[0] * 3, |i, j| first[(j / 3, i * 3 + j % 3)]);
        let second = v.solve(&targets)?;
        let mut controls = Vec::with_capacity(u.control_count * v.control_count);
        for j in 0..v.control_count {
            for i in 0..u.control_count {
                let p = if !closed[0]
                    && !closed[1]
                    && (i == 0 || i == count[0] - 1)
                    && (j == 0 || j == count[1] - 1)
                {
                    points[j * count[0] + i]
                } else {
                    Point3::try_from(std::array::from_fn(|axis| {
                        second[(j % count[1], (i % count[0]) * 3 + axis)] * scale + origin[axis]
                    }))?
                };
                controls.push(p);
            }
        }
        let control_scale = controls
            .iter()
            .flat_map(|p| p.to_array().into_iter().zip(origin))
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, Real::max);
        let surface = Self::try_new(
            degree[0],
            degree[1],
            u.control_count,
            v.control_count,
            controls,
            u.knots.clone(),
            v.knots.clone(),
        )?;
        // Check the actual tensor evaluator, independently of the construction
        // matrices. The allowance separates local solve error, amplified
        // cancellation in boundary-span continuation, and world-offset rounding.
        for (j, &tv) in v.parameters.iter().enumerate() {
            for (i, &tu) in u.parameters.iter().enumerate() {
                let actual = surface.evaluate_extended(tu, tv)?;
                for (a, b) in actual
                    .to_array()
                    .into_iter()
                    .zip(points[j * count[0] + i].to_array())
                {
                    let continuation_roundoff = 32.0
                        * Real::EPSILON
                        * (degree[0] + degree[1] + 2) as Real
                        * u.amplification[i]
                        * v.amplification[j]
                        * control_scale;
                    let allowance = 2e-12 * scale
                        + continuation_roundoff
                        + 64.0 * Real::EPSILON * a.abs().max(b.abs());
                    require_finite([allowance], "point grid residual allowance")?;
                    if (a - b).abs() > allowance {
                        return Err(GeometryError::InvalidPointGrid {
                            context: "tensor interpolation residual exceeds numeric allowance",
                        });
                    }
                }
            }
        }
        Ok(surface)
    }
}

fn validate(
    points: &[Point3],
    count: [usize; 2],
    degree: [usize; 2],
    closed: [bool; 2],
) -> Result<[usize; 2], GeometryError> {
    if count.iter().any(|n| *n > MAX_POINT_GRID_AXIS_COUNT) {
        return Err(GeometryError::PointGridResourceLimit {
            maximum: MAX_POINT_GRID_AXIS_COUNT,
        });
    }
    if count
        .iter()
        .enumerate()
        .any(|(axis, n)| *n < if closed[axis] { 3 } else { 2 })
        || count[0].checked_mul(count[1]) != Some(points.len())
    {
        return Err(GeometryError::InvalidPointGrid {
            context: "counts must match the complete rectangular grid (at least two open or three closed stations)",
        });
    }
    Ok(std::array::from_fn(|axis| {
        degree[axis].clamp(1, MAX_POINT_GRID_DEGREE.min(count[axis] - 1))
    }))
}
