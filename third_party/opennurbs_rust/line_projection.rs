//! Modified Rust adaptation of ON_Line::ClosestPointTo and ON_Line::PointAt
//! from opennurbs_line.cpp at 23fc677ba06e49212296ca75fab7fb6c2851b4ce.
//! This is altered source, not an upstream OpenNURBS component. Finite validated
//! Rust points/vectors replace native types; unrepresentable/degenerate lines
//! return errors. The projection remains unbounded.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
// Permission and MIT disclaimer: third_party/opennurbs_rust/LICENSE.
use viboceros_geometry::{GeometryError, Point3};

pub(crate) fn closest_point(
    from: Point3,
    to: Point3,
    point: Point3,
) -> Result<(Point3, f64), GeometryError> {
    let d = from.vector_to(to)?;
    // OpenNURBS uses ordinary left-associated products here. The kernel's
    // compensated dot product can change a projected remote endpoint by an ulp.
    let dot = |a: viboceros_geometry::Vector3, b: viboceros_geometry::Vector3| {
        a.x() * b.x() + a.y() * b.y() + a.z() * b.z()
    };
    let squared = dot(d, d);
    if !squared.is_finite() || squared <= 0. {
        return Err(GeometryError::Degenerate {
            context: "line projection endpoints",
        });
    }
    let a = from.vector_to(point)?;
    let b = to.vector_to(point)?;
    let t = if dot(a, a) <= dot(b, b) {
        dot(a, d) / squared
    } else {
        1. + dot(b, d) / squared
    };
    let projected = if t < 0.5 {
        from.translated(d.scaled(t)?)?
    } else {
        to.translated(d.scaled(t - 1.)?)?
    };
    Ok((projected, t))
}
