//! Modified Rust adaptation of ON_Length3d from opennurbs_point.cpp at
//! 23fc677ba06e49212296ca75fab7fb6c2851b4ce. The caller supplies finite components;
//! this port retains operation order, maximum-component scaling and denormal
//! handling. This is altered source, not an upstream OpenNURBS component.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
// Permission and MIT disclaimer: third_party/opennurbs_rust/LICENSE.
pub(crate) fn length3d([x, y, z]: [f64; 3]) -> f64 {
    let [mut x, mut y, mut z] = [x.abs(), y.abs(), z.abs()];
    if y >= x && y >= z {
        std::mem::swap(&mut x, &mut y);
    } else if z >= x && z >= y {
        std::mem::swap(&mut x, &mut z);
    }
    if x > f64::MIN_POSITIVE {
        y /= x;
        z /= x;
        x * (1. + y * y + z * z).sqrt()
    } else if x > 0. && x.is_finite() {
        x
    } else {
        0.
    }
}
