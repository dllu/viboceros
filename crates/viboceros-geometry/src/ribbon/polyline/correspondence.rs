//! The source clock before clipping an offset strip at its inside corners.
use crate::{GeometryError, LineSegment, Point3, Real, Tolerance, UnitVector3};

pub(super) fn sharp_clip_at(index: usize, inside: &[bool], outside: &[bool], closed: bool) -> bool {
    if !inside[index] {
        return false;
    }
    let n = inside.len();
    // A closing seam does not extend either adjacent strip. Other outside
    // joins affect only their two neighboring inside corners.
    let outside_at = |i| outside[i] && !(closed && i == 0);
    outside_at((index + n - 1) % n) || outside_at((index + 1) % n)
}

pub(super) enum SegmentClock {
    Original,
    Projected,
    /// Sharp extensions belong to the strip before inside intersections clip
    /// it. Measure the clip in that extended strip's original affine clock.
    Clipped {
        distance: Real,
        normal: UnitVector3,
    },
}

pub(super) fn fractions(
    segment: LineSegment,
    offset: [Point3; 2],
    inside: [bool; 2],
    clipped: [bool; 2],
    clock: SegmentClock,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    let reference = match clock {
        SegmentClock::Original => return Ok([0., 1.]),
        SegmentClock::Projected => segment,
        SegmentClock::Clipped { distance, normal } => {
            let shift = normal
                .as_vector()
                .cross(segment.direction(tolerance)?.as_vector())?
                .scaled(distance)?;
            let raw_start = if inside[0] {
                segment.start().translated(shift)?
            } else {
                offset[0]
            };
            let raw_end = if inside[1] {
                segment.end().translated(shift)?
            } else {
                offset[1]
            };
            LineSegment::try_new(raw_start, raw_end, tolerance)?
        }
    };
    Ok([
        if clipped[0] {
            reference.closest_parameter(offset[0], tolerance)?
        } else {
            0.
        },
        if clipped[1] {
            reference.closest_parameter(offset[1], tolerance)?
        } else {
            1.
        },
    ])
}
