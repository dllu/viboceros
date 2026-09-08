//! Shared screen-space predicates for click and rectangle selection.

use super::{Pos2, Real, Rect};

pub(super) fn signed_area(start: Pos2, end: Pos2, target: Pos2) -> Real {
    (Real::from(end.x) - Real::from(start.x)).mul_add(
        Real::from(target.y) - Real::from(start.y),
        -(Real::from(end.y) - Real::from(start.y)) * (Real::from(target.x) - Real::from(start.x)),
    )
}

pub(super) fn segment_intersects_rect(start: Pos2, end: Pos2, rect: Rect) -> bool {
    clip_line_to_rect(start, end, rect, false).is_some()
}

/// Clip a finite segment or its infinite supporting line to a finite rectangle.
/// Degenerate rectangles are supported; a zero-length segment is a point hit.
pub(super) fn clip_line_to_rect(
    start: Pos2,
    end: Pos2,
    rect: Rect,
    extend: bool,
) -> Option<[Pos2; 2]> {
    if !start.is_finite()
        || !end.is_finite()
        || !rect.is_finite()
        || rect.min.x > rect.max.x
        || rect.min.y > rect.max.y
    {
        return None;
    }
    let a = [f64::from(start.x), f64::from(start.y)];
    let b = [f64::from(end.x), f64::from(end.y)];
    let delta = [b[0] - a[0], b[1] - a[1]];
    if delta == [0.0; 2] {
        return (!extend && rect.contains(start)).then_some([start, start]);
    }
    let axis = usize::from(delta[1].abs() > delta[0].abs());
    let other = 1 - axis;
    let bounds = [
        [f64::from(rect.left()), f64::from(rect.right())],
        [f64::from(rect.top()), f64::from(rect.bottom())],
    ];
    // Clip in a screen coordinate, not a parameter near 0.5 or 1 whose
    // endpoints can round together for distant anchors. Original f32
    // coordinate products are exact in f64, retaining a small intercept
    // when their large products cancel.
    let slope = delta[other] / delta[axis];
    let intercept_numerator = a[other] * b[axis] - b[other] * a[axis];
    let intercept = intercept_numerator / delta[axis];
    let [mut low, mut high] = bounds[axis];
    if !extend {
        low = low.max(a[axis].min(b[axis]));
        high = high.min(a[axis].max(b[axis]));
    }
    if slope == 0.0 {
        if !(bounds[other][0]..=bounds[other][1]).contains(&intercept) {
            return None;
        }
    } else {
        // Solve the implicit line equation before dividing. Dividing both
        // slope and intercept first can round an exact corner contact away.
        let first = bounds[other][0].mul_add(delta[axis], -intercept_numerator) / delta[other];
        let second = bounds[other][1].mul_add(delta[axis], -intercept_numerator) / delta[other];
        low = low.max(first.min(second));
        high = high.min(first.max(second));
    }
    if low > high {
        return None;
    }
    let coordinates = if delta[axis] > 0.0 {
        [low, high]
    } else {
        [high, low]
    };
    Some(coordinates.map(|coordinate| {
        let mut point = [0.0; 2];
        point[axis] = coordinate as f32;
        point[other] = slope
            .mul_add(coordinate, intercept)
            .clamp(bounds[other][0], bounds[other][1]) as f32;
        Pos2::new(point[0], point[1])
    }))
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
    fn rectangle_intersection_matches_integer_orientation_reference() {
        type P = [i32; 2];
        fn orientation(a: P, b: P, c: P) -> i32 {
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        }
        fn intersects(a: P, b: P, c: P, d: P) -> bool {
            (0..2).all(|i| a[i].min(b[i]) <= c[i].max(d[i]) && c[i].min(d[i]) <= a[i].max(b[i]))
                && orientation(a, b, c) * orientation(a, b, d) <= 0
                && orientation(c, d, a) * orientation(c, d, b) <= 0
        }
        let points: Vec<P> = (-3..=3)
            .flat_map(|x| (-3..=3).map(move |y| [x, y]))
            .collect();
        let pos = |p: P| Pos2::new(p[0] as f32, p[1] as f32);
        for (min, max) in [([-1, -1], [1, 1]), ([0, 0], [0, 0]), ([0, -1], [0, 1])] {
            let corners = [min, [max[0], min[1]], max, [min[0], max[1]]];
            let contains = |p: P| (0..2).all(|i| (min[i]..=max[i]).contains(&p[i]));
            let rect = Rect::from_min_max(pos(min), pos(max));
            for &a in &points {
                for &b in &points {
                    let expected = contains(a)
                        || contains(b)
                        || (0..4).any(|i| intersects(a, b, corners[i], corners[(i + 1) % 4]));
                    assert_eq!(
                        segment_intersects_rect(pos(a), pos(b), rect),
                        expected,
                        "a={a:?}, b={b:?}, min={min:?}, max={max:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn distant_diagonal_does_not_cross_a_rectangle_off_its_line() {
        let rect = Rect::from_min_max(Pos2::new(1.0, 1.0), Pos2::new(2.0, 2.0));
        for scale in [1e20, f32::MAX] {
            let start = Pos2::new(-scale, scale);
            let end = Pos2::new(scale, -scale);
            assert!(!segment_intersects_rect(start, end, rect));
            assert!(!segment_intersects_rect(end, start, rect));
        }
    }

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
