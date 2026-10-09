// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// Altered Rust adaptation for Viboceros; not the original OpenNURBS source.
// ON_Circle::Transform radius policy, opennurbs_circle.cpp, OpenNURBS 8.x.
// See LICENSE in this directory and https://www.opennurbs.org/.
//
// This command compatibility adapter retains a circular approximation after
// nonuniform scaling. General kernel affine transforms retain exact conics.
use viboceros_geometry::{
    AffineTransform3, Circle3, CircularArc3, Frame3, GeometryError, Tolerance,
};

fn normalize_pair(mut primary: f64, mut secondary: f64) -> (f64, f64, f64) {
    let mut scale;
    if primary.abs() >= secondary.abs() {
        scale = primary.abs();
        if scale > 0. {
            primary = primary.signum();
            secondary /= scale;
            if secondary.abs() <= 1e-12 {
                secondary = 0.;
                if (1. - scale).abs() <= 1e-12 {
                    scale = 1.;
                }
            }
        }
    } else {
        scale = secondary.abs();
        secondary = secondary.signum();
        primary /= scale;
        if primary.abs() <= 1e-12 {
            primary = 0.;
            if (1. - scale).abs() <= 1e-12 {
                scale = 1.;
            }
        }
    }
    (primary, secondary, scale)
}

pub fn circle(circle: Circle3, transform: AffineTransform3) -> Result<Circle3, GeometryError> {
    let tx = transform.transform_vector(circle.x_axis().as_vector())?;
    let ty = transform.transform_vector(circle.y_axis().as_vector())?;
    let x = tx.normalized_nonzero()?;
    let normal = x
        .as_vector()
        .cross(ty.normalized_nonzero()?.as_vector())?
        .normalized_nonzero()?;
    let tolerance = Tolerance::NUMERICAL_VALIDATION;
    let frame = Frame3::try_from_x_and_normal(
        transform.transform_point(circle.center())?,
        x.as_vector(),
        normal.as_vector(),
        tolerance,
    )?;
    let (a, b, r1) = normalize_pair(
        tx.dot(frame.x_axis().as_vector())?,
        tx.dot(frame.y_axis().as_vector())?,
    );
    let (d, c, r2) = normalize_pair(
        ty.dot(frame.y_axis().as_vector())?,
        ty.dot(frame.x_axis().as_vector())?,
    );
    let sqrt_epsilon = f64::EPSILON.sqrt();
    let scale = if b == 0. && c == 0. && (r1 - r2).abs() <= sqrt_epsilon * (r1 + r2) {
        if r1 == r2 { r1 } else { 0.5 * (r1 + r2) }
    } else {
        (r1 * r2 * (a * d - b * c)).abs().sqrt()
    };
    let radius = if scale > 0. && (scale - 1.).abs() > sqrt_epsilon {
        circle.radius() * scale
    } else {
        circle.radius()
    };
    Circle3::try_from_frame(
        frame.origin(),
        radius,
        frame.x_axis(),
        frame.z_axis(),
        tolerance,
    )?
    .try_reparameterized(circle.domain())
}

pub fn arc(arc: CircularArc3, transform: AffineTransform3) -> Result<CircularArc3, GeometryError> {
    let support = Circle3::try_from_frame(
        arc.center(),
        arc.radius(),
        arc.plane_x_axis(),
        arc.normal()?,
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    CircularArc3::try_from_circle_angles(circle(support, transform)?, arc.angle_domain())?
        .try_reparameterized(arc.domain())
}
