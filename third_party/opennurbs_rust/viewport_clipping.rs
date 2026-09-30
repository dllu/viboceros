// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// OpenNURBS, Rhinoceros, and Rhino3D are registered trademarks of Robert
// McNeel & Associates.
//
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
//
// Modified Rust adaptation of the public ON_Viewport::SetFrustumNearFar
// constrained overload in opennurbs_viewport.cpp, from the pinned OpenNURBS
// submodule. See README.md and LICENSE in this directory.
// This port returns a camera dolly rather than mutating a viewport. Its invalid
// ratio fallback uses Rhino ViewportInfo's measured default (0.0005), and it
// rejects nonfinite intervals before the original constraint calculation.

const ZERO_TOLERANCE: f64 = 1.0 / 4_294_967_296.0;
const DEFAULT_MIN_NEAR: f64 = 0.0001;
const DOCUMENT_MIN_RATIO: f64 = 0.0005;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ClipRequest {
    pub near: f64,
    pub far: f64,
    pub min_near: f64,
    pub min_ratio: f64,
    pub target_distance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ClipAdjustment {
    pub near: f64,
    pub far: f64,
    pub camera_dolly: f64,
}

impl ClipAdjustment {
    pub(crate) fn constrained(
        request: ClipRequest,
        perspective: bool,
        half_width: f64,
        half_height: f64,
    ) -> Result<Self, &'static str> {
        let ClipRequest {
            mut near,
            mut far,
            min_near,
            min_ratio,
            target_distance: target,
        } = request;
        if !near.is_finite() || !far.is_finite() || near > far {
            return Err("invalid clipping interval");
        }
        let min_near = if min_near.is_finite() && min_near > ZERO_TOLERANCE {
            min_near
        } else {
            DEFAULT_MIN_NEAR
        };
        let ratio = if min_ratio.is_finite()
            && min_ratio > ZERO_TOLERANCE
            && min_ratio < 1.0 - ZERO_TOLERANCE
        {
            min_ratio
        } else {
            DOCUMENT_MIN_RATIO
        };
        let mut camera_dolly = 0.0;
        if perspective {
            near = near.max(min_near);
            if far <= near + ZERO_TOLERANCE {
                far = 100.0 * near;
                if target > near + min_near && far <= target + min_near {
                    far = 2.0 * target - near;
                }
                if near < ratio * far {
                    far = near / ratio;
                }
            }
            if near < 1.0001 * ratio * far {
                if target.is_finite() && near < target && target < far {
                    let mut finished = false;
                    if target / far < ratio {
                        if near / target >= ratio.sqrt() {
                            far = near / ratio;
                            finished = true;
                        } else {
                            far = target / ratio;
                        }
                    }
                    if !finished && near / target < ratio {
                        if target / far <= ratio.sqrt() && far <= 4.0 * target {
                            near = far * ratio;
                            finished = true;
                        } else {
                            near = target * ratio;
                        }
                    }
                    if !finished {
                        let denominator = (far - target) * ratio + (target - near);
                        if denominator > 0.0 {
                            let s = target * (1.0 - ratio) / denominator;
                            let s = if !s.is_finite() || s > 1.0 || s <= ZERO_TOLERANCE {
                                1.0
                            } else {
                                s
                            };
                            let mut n = s * near + target * (1.0 - s);
                            let mut f = s * far + target * (1.0 - s);
                            if n < near || n >= target {
                                n = if target < f && f < far {
                                    ratio * f
                                } else {
                                    near
                                };
                            }
                            if f > far || f <= target {
                                f = if near < n && n < target {
                                    n / ratio
                                } else {
                                    far
                                };
                            }
                            if n < ratio * f {
                                n = ratio * f;
                            } else {
                                f = n / ratio;
                            }
                            near = n;
                            far = f;
                        } else {
                            near = ratio * far;
                        }
                    }
                } else if target.is_finite() && (far - target).abs() > (near - target).abs() {
                    far = near / ratio;
                } else {
                    near = ratio * far;
                }
            }
        } else {
            if far <= near + ZERO_TOLERANCE {
                let padding = near.abs() * 0.125;
                let padding = if padding <= DEFAULT_MIN_NEAR
                    || padding < ZERO_TOLERANCE
                    || padding < min_near
                {
                    1.0
                } else {
                    padding
                };
                near -= padding;
                far += padding;
            }
            if near < min_near || near < DEFAULT_MIN_NEAR {
                let new_near = (3.0 * half_width.max(half_height))
                    .max(2.0 * min_near)
                    .max(2.0 * DEFAULT_MIN_NEAR);
                camera_dolly = new_near - near;
                near = new_near;
                far += camera_dolly;
                if far < near {
                    far = 1.125 * near;
                }
            }
        }
        if ![near, far, camera_dolly].into_iter().all(f64::is_finite)
            || near <= 0.0
            || far <= near
            || far >= 1.0e100
        {
            return Err("clipping distances exceed the supported range");
        }
        Ok(Self {
            near,
            far,
            camera_dolly,
        })
    }
}
