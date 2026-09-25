//! Stable full meridian loops for a plane close to the torus axis.

use super::super::SurfaceSurfaceIntersectionEvent;
use crate::{Frame3, GeometryError, NurbsCurve, Real};

const TURN: Real = std::f64::consts::TAU;
const MAX_SEGMENTS: usize = 4096;

#[derive(Clone, Copy)]
struct Section {
    frame: Frame3,
    major: Real,
    minor: Real,
    horizontal_axis: [Real; 2],
    axial_slope: Real,
    radial_offset: Real,
}

#[derive(Clone, Copy)]
struct Sample {
    position: [Real; 3],
    derivative: [Real; 3],
}

pub(super) fn intersect(
    (frame, major, minor): (Frame3, Real, Real),
    horizontal_axis: [Real; 2],
    axial_slope: Real,
    radial_offset: Real,
    fit_tolerance: Real,
) -> Result<Vec<SurfaceSurfaceIntersectionEvent>, GeometryError> {
    let section = Section {
        frame,
        major,
        minor,
        horizontal_axis,
        axial_slope,
        radial_offset,
    };
    [1.0, -1.0]
        .into_iter()
        .map(|side| fit(section, side, fit_tolerance).map(SurfaceSurfaceIntersectionEvent::Curve))
        .collect()
}

impl Section {
    fn sample(self, side: Real, angle: Real) -> Sample {
        let (sine, cosine) = angle.sin_cos();
        let radial = self.major + self.minor * cosine;
        let radial_derivative = -self.minor * sine;
        let height = self.minor * sine;
        let height_derivative = self.minor * cosine;
        let along = -self.axial_slope.mul_add(height, self.radial_offset);
        let along_derivative = -self.axial_slope * height_derivative;
        let transverse = ((radial - along.abs()) * (radial + along.abs())).sqrt();
        let lateral = side * transverse;
        let lateral_derivative =
            side * (radial * radial_derivative - along * along_derivative) / transverse;
        let [ux, uy] = self.horizontal_axis;
        Sample {
            position: [
                along.mul_add(ux, -lateral * uy),
                along.mul_add(uy, lateral * ux),
                height,
            ],
            derivative: [
                along_derivative.mul_add(ux, -lateral_derivative * uy),
                along_derivative.mul_add(uy, lateral_derivative * ux),
                height_derivative,
            ],
        }
    }
}

fn fit(section: Section, side: Real, tolerance: Real) -> Result<NurbsCurve, GeometryError> {
    let mut segments = Vec::new();
    for index in 0..8 {
        fit_interval(
            section,
            side,
            TURN * index as Real / 8.0,
            TURN * (index + 1) as Real / 8.0,
            tolerance,
            0,
            &mut segments,
        )?;
    }
    let first = section.frame.point_at(section.sample(side, 0.0).position)?;
    let mut controls = Vec::with_capacity(3 * segments.len() + 1);
    let mut knots = Vec::with_capacity(3 * segments.len() + 5);
    controls.push(first);
    knots.extend([0.0; 4]);
    let count = segments.len();
    for (index, (end, control)) in segments.into_iter().enumerate() {
        controls.push(section.frame.point_at(control[1])?);
        controls.push(section.frame.point_at(control[2])?);
        controls.push(if index + 1 == count {
            first
        } else {
            section.frame.point_at(control[3])?
        });
        knots.extend([end; 3]);
    }
    knots.push(TURN);
    NurbsCurve::try_new(3, controls, knots)
}

fn fit_interval(
    section: Section,
    side: Real,
    start: Real,
    end: Real,
    tolerance: Real,
    depth: usize,
    segments: &mut Vec<(Real, [[Real; 3]; 4])>,
) -> Result<(), GeometryError> {
    let first = section.sample(side, start);
    let last = section.sample(side, end);
    let handle = (end - start) / 3.0;
    let control = [
        first.position,
        std::array::from_fn(|i| first.derivative[i].mul_add(handle, first.position[i])),
        std::array::from_fn(|i| (-last.derivative[i]).mul_add(handle, last.position[i])),
        last.position,
    ];
    let accurate = [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875]
        .into_iter()
        .all(|fraction| {
            let expected = section
                .sample(side, start + (end - start) * fraction)
                .position;
            let actual = bezier(control, fraction);
            (actual[0] - expected[0])
                .hypot(actual[1] - expected[1])
                .hypot(actual[2] - expected[2])
                <= tolerance * 0.25
        });
    if accurate {
        if segments.len() >= MAX_SEGMENTS {
            return Err(GeometryError::TooManyCurveFitControlPoints {
                maximum: 3 * MAX_SEGMENTS + 1,
            });
        }
        segments.push((end, control));
        return Ok(());
    }
    if depth >= 32 || segments.len() >= MAX_SEGMENTS || end - start <= 16.0 * Real::EPSILON {
        return Err(GeometryError::TooManyCurveFitControlPoints {
            maximum: 3 * MAX_SEGMENTS + 1,
        });
    }
    let middle = 0.5 * (start + end);
    fit_interval(section, side, start, middle, tolerance, depth + 1, segments)?;
    fit_interval(section, side, middle, end, tolerance, depth + 1, segments)
}

fn bezier(control: [[Real; 3]; 4], fraction: Real) -> [Real; 3] {
    let complement = 1.0 - fraction;
    let basis = [
        complement.powi(3),
        3.0 * complement * complement * fraction,
        3.0 * complement * fraction * fraction,
        fraction.powi(3),
    ];
    std::array::from_fn(|i| {
        basis
            .iter()
            .zip(control)
            .map(|(weight, point)| weight * point[i])
            .sum()
    })
}
