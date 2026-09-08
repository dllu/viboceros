//! Shared screen-space predicates for click and rectangle selection.

use super::{Pos2, Real, Rect};

pub(super) fn signed_area(start: Pos2, end: Pos2, target: Pos2) -> Real {
    (Real::from(end.x) - Real::from(start.x)).mul_add(
        Real::from(target.y) - Real::from(start.y),
        -(Real::from(end.y) - Real::from(start.y)) * (Real::from(target.x) - Real::from(start.x)),
    )
}

pub(super) fn segment_intersects_rect(start: Pos2, end: Pos2, rect: Rect) -> bool {
    if rect.contains(start) || rect.contains(end) {
        return true;
    }
    // Promote before subtracting: opposite finite f32 endpoints may have an
    // unrepresentable f32 difference and collapse distinct clip parameters.
    let delta_x = Real::from(end.x) - Real::from(start.x);
    let delta_y = Real::from(end.y) - Real::from(start.y);
    let mut minimum = 0.0_f64;
    let mut maximum = 1.0_f64;
    for (direction, distance) in [
        (-delta_x, Real::from(start.x) - Real::from(rect.left())),
        (delta_x, Real::from(rect.right()) - Real::from(start.x)),
        (-delta_y, Real::from(start.y) - Real::from(rect.top())),
        (delta_y, Real::from(rect.bottom()) - Real::from(start.y)),
    ] {
        if direction == 0.0 {
            if distance < 0.0 {
                return false;
            }
            continue;
        }
        let parameter = distance / direction;
        if direction < 0.0 {
            minimum = minimum.max(parameter);
        } else {
            maximum = maximum.min(parameter);
        }
        if minimum > maximum {
            return false;
        }
    }
    true
}

pub(super) fn rect_corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

pub(super) fn point_segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let start_x = f64::from(start.x);
    let start_y = f64::from(start.y);
    let delta_x = f64::from(end.x) - start_x;
    let delta_y = f64::from(end.y) - start_y;
    let length_squared = delta_x.mul_add(delta_x, delta_y * delta_y);
    let parameter = if length_squared > 0.0 && length_squared.is_finite() {
        ((f64::from(point.x) - start_x).mul_add(delta_x, (f64::from(point.y) - start_y) * delta_y)
            / length_squared)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest_x = delta_x.mul_add(parameter, start_x);
    let closest_y = delta_y.mul_add(parameter, start_y);
    let distance = (f64::from(point.x) - closest_x).hypot(f64::from(point.y) - closest_y);
    if distance.is_finite() && distance <= f64::from(f32::MAX) {
        distance as f32
    } else {
        f32::INFINITY
    }
}

pub(super) fn point_in_triangle(point: Pos2, first: Pos2, second: Pos2, third: Pos2) -> bool {
    let area = signed_area(first, second, third);
    if !area.is_finite() || area.abs() <= f64::EPSILON {
        return false;
    }
    let tolerance = area.abs().max(1.0) * 1.0e-12;
    let signs = [
        signed_area(first, second, point),
        signed_area(second, third, point),
        signed_area(third, first, point),
    ];
    signs.iter().all(|value| *value >= -tolerance) || signs.iter().all(|value| *value <= tolerance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_intersection_includes_boundary_and_degenerate_hits() {
        let rect = Rect::from_min_max(Pos2::new(-1.0, -1.0), Pos2::new(1.0, 1.0));
        for (start, end, expected) in [
            ((0.0, 0.0), (0.0, 0.0), true),
            ((2.0, 0.0), (2.0, 0.0), false),
            ((-2.0, 1.0), (2.0, 1.0), true),
            ((-2.0, 2.0), (2.0, 2.0), false),
            ((-2.0, 0.0), (0.0, 2.0), true),
            ((-2.0, 0.0), (0.0, 3.0), false),
        ] {
            let start = Pos2::new(start.0, start.1);
            let end = Pos2::new(end.0, end.1);
            assert_eq!(segment_intersects_rect(start, end, rect), expected);
            assert_eq!(segment_intersects_rect(end, start, rect), expected);
        }
    }

    #[test]
    fn rectangle_intersection_does_not_overflow_finite_screen_endpoints() {
        let rect = Rect::from_min_max(Pos2::new(-1.0, -1.0), Pos2::new(1.0, 1.0));
        let start = Pos2::new(-f32::MAX, f32::MAX);
        let miss = Pos2::new(f32::MAX, -f32::MAX / 2.0);
        let hit = Pos2::new(f32::MAX, -f32::MAX);
        for (end, expected) in [(miss, false), (hit, true)] {
            assert_eq!(segment_intersects_rect(start, end, rect), expected);
            assert_eq!(segment_intersects_rect(end, start, rect), expected);
        }
    }
}
