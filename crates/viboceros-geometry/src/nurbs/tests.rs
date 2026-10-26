//! Legacy NURBS regressions grouped by the operation they exercise.
use super::*;

use crate::Tolerance;

fn point(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.0).unwrap()
}

fn assert_point_near(actual: Point3, expected: Point3) {
    assert!(
        actual.is_near(
            expected,
            Tolerance::try_new(1.0e-12, 1.0e-12, 1.0e-12).unwrap()
        ),
        "actual {actual:?}, expected {expected:?}"
    );
}

mod conics;
mod controls;
mod degree_weights;
mod evaluation;
mod extension;
mod extension_boundaries;
mod extension_pieces;
mod intersections;
mod knots;
mod projection;
mod topology;
mod trimming;
