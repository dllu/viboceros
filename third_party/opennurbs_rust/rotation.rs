// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// OpenNURBS, Rhinoceros, and Rhino3D are registered trademarks of Robert
// McNeel & Associates.
//
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
//
// Modified Rust adaptation of ON_Xform::Rotation's cardinal-angle noise
// policy in opennurbs_xform.cpp at the pinned OpenNURBS revision. See README.md
// and LICENSE in this directory. Inputs come from f64::sin_cos, so this port
// omits the native normalization of caller-supplied sine/cosine pairs. It
// returns only snapped components; nonsnapped rotations use the Rust kernel.

const SQRT_EPSILON: f64 = 1.490116119385e-8;

pub(crate) fn snapped_components(angle: f64) -> Option<(f64, f64)> {
    let (sine, cosine) = angle.sin_cos();
    if sine.abs() >= 1.0 - SQRT_EPSILON && cosine.abs() <= SQRT_EPSILON {
        return Some((sine.signum(), 0.0));
    }
    if (cosine.abs() >= 1.0 - SQRT_EPSILON && sine.abs() <= SQRT_EPSILON)
        || cosine.abs() > 1.0 - f64::EPSILON
        || sine.abs() < f64::EPSILON
    {
        return Some((0.0, cosine.signum()));
    }
    if sine.abs() > 1.0 - f64::EPSILON || cosine.abs() < f64::EPSILON {
        return Some((sine.signum(), 0.0));
    }
    None
}
