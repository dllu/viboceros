//! Altered Rust adaptation of ON_Xform::Rotation and ON_3dPoint::Transform
//! from the pinned public OpenNURBS opennurbs_xform.cpp/opennurbs_point.cpp.
//! Retains the native finite endpoint arithmetic for command reference lines.
//! Inputs use validated unit axes and stdlib sine/cosine pairs. Invalid finite
//! results return errors instead of storing invalid point coordinates.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED. See LICENSE in this directory.
use viboceros_geometry::{GeometryError, Point3, UnitVector3};
#[path = "rotation.rs"]
mod cleanup;

pub(crate) fn reference_line(
    from: Point3,
    to: Point3,
    axis: UnitVector3,
    angle: f64,
) -> Result<(Point3, Point3), GeometryError> {
    let (s, c) = cleanup::snapped_components(angle).unwrap_or_else(|| angle.sin_cos());
    let [x, y, z] = axis.as_vector().to_array();
    let d = 1. - c;
    let rows = [
        [x * x * d + c, x * y * d - z * s, x * z * d + y * s],
        [y * x * d + z * s, y * y * d + c, y * z * d - x * s],
        [z * x * d - y * s, z * y * d + x * s, z * z * d + c],
    ];
    let center = from.to_array();
    let offset: [f64; 3] = std::array::from_fn(|i| {
        let mut r = rows[i];
        r[i] -= 1.;
        -(r[0] * center[0] + r[1] * center[1] + r[2] * center[2])
    });
    let apply = |point: Point3| {
        let p = point.to_array();
        Point3::try_from(std::array::from_fn(|i| {
            rows[i][0] * p[0] + rows[i][1] * p[1] + rows[i][2] * p[2] + offset[i]
        }))
    };
    Ok((apply(from)?, apply(to)?))
}
