// Adapted from ON_Brep::ShrinkSurface in opennurbs_brep.cpp,
// OpenNURBS 23fc677ba06e49212296ca75fab7fb6c2851b4ce.
// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// See LICENSE and README.md. This is a modified Rust port of the interval
// policy only; surface cropping and topology validation belong to the kernel.
use crate::{GeometryError, Real, require_finite};

/// Domain and outer trim bounds in native surface parameters.
/// Side order is West, South, East, North. `iso_ends` is the union of the
/// endpoints of all outer isoparametric trims, including interior ones.
pub(crate) fn intervals(
    domain: [[Real; 2]; 2],
    mut outer: [[Real; 2]; 2],
    all_iso: bool,
    iso_ends: Option<[[Real; 2]; 2]>,
    surface_sides: [bool; 4],
    margin: bool,
) -> Result<Option<[[Real; 2]; 2]>, GeometryError> {
    if margin && !all_iso {
        for axis in 0..2 {
            let padding = (outer[axis][1] - outer[axis][0]) * 0.01;
            if iso_ends.is_none_or(|ends| outer[axis][0] < ends[axis][0]) && !surface_sides[axis] {
                outer[axis][0] -= padding;
            }
            if iso_ends.is_none_or(|ends| outer[axis][1] > ends[axis][1])
                && !surface_sides[axis + 2]
            {
                outer[axis][1] += padding;
            }
        }
    }
    for axis in 0..2 {
        outer[axis][0] = outer[axis][0].max(domain[axis][0]);
        outer[axis][1] = outer[axis][1].min(domain[axis][1]);
    }
    require_finite(outer.into_iter().flatten(), "shrunken surface domains")?;
    let lengths = outer.map(|range| range[1] - range[0]);
    let old_lengths = domain.map(|range| range[1] - range[0]);
    require_finite(
        lengths.into_iter().chain(old_lengths),
        "surface domain lengths",
    )?;
    // ON_ZERO_TOLERANCE = 2^-32. This relative stopping rule avoids tiny
    // repeated replacements; it is independent of model-space tolerance.
    const ZERO_TOLERANCE: Real = 2.3283064365386963e-10;
    Ok((lengths.iter().all(|&length| length > 0.)
        && (0..2).any(|axis| lengths[axis] * ZERO_TOLERANCE < old_lengths[axis] - lengths[axis]))
    .then_some(outer))
}
