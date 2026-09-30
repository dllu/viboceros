//! Display-frustum depth clipping for selection, separate from drafting rays.
use super::*;
use viboceros_geometry::interpolate_scalar;

#[derive(Clone, Copy)]
struct Vertex {
    point: Point3,
    depth: Real,
}

impl Vertex {
    fn intersection(self, other: Self, depth: Real) -> Option<Self> {
        let first = self.point.to_array();
        let second = other.point.to_array();
        let coordinate = |axis| {
            interpolate_scalar(
                [first[axis], second[axis]],
                [self.depth, other.depth],
                depth,
            )
            .ok()
        };
        Some(Self {
            point: Point3::try_from([coordinate(0)?, coordinate(1)?, coordinate(2)?]).ok()?,
            depth,
        })
    }
}

impl Viewport {
    pub(super) fn display_depth_interval(&self) -> (Real, Real) {
        if self.kind.is_parallel() {
            (
                self.frustum_near - self.perspective_camera_distance,
                self.frustum_far - self.perspective_camera_distance,
            )
        } else {
            (self.frustum_near, self.frustum_far)
        }
    }

    pub(super) fn point_within_display_depth(&self, point: Point3) -> bool {
        let depth = self.view_depth(point);
        let (near, far) = self.display_depth_interval();
        depth.is_finite() && depth >= near && depth <= far
    }

    pub(super) fn project_selection_point(&self, point: Point3, rect: Rect) -> Option<Pos2> {
        self.point_within_display_depth(point)
            .then(|| self.project(point, rect))
            .flatten()
    }

    pub(super) fn clip_selection_segment(&self, start: Point3, end: Point3) -> Option<[Point3; 2]> {
        let mut vertices = [start, end].map(|point| Vertex {
            point,
            depth: self.view_depth(point),
        });
        if vertices.iter().any(|v| !v.depth.is_finite()) {
            return None;
        }
        let (mut near, far) = self.display_depth_interval();
        if self.kind == ViewKind::Perspective {
            near = near.max(self.primitive_near(&[start, end]));
        }
        for (plane, above) in [(near, true), (far, false)] {
            let inside = vertices.map(|v| {
                if above {
                    v.depth >= plane
                } else {
                    v.depth <= plane
                }
            });
            match inside {
                [false, false] => return None,
                [true, true] => {}
                _ => {
                    let intersection = vertices[0].intersection(vertices[1], plane)?;
                    vertices[usize::from(inside[0])] = intersection;
                }
            }
        }
        Some(vertices.map(|v| v.point))
    }

    pub(super) fn project_selection_segment(
        &self,
        start: Point3,
        end: Point3,
        rect: Rect,
    ) -> Option<[Pos2; 2]> {
        let [start, end] = self.clip_selection_segment(start, end)?;
        Some([self.project(start, rect)?, self.project(end, rect)?])
    }

    /// Rhino tests the full line's closest screen point against the depth
    /// interval. A nearby artificial endpoint at a clip plane is not a new hit.
    pub(super) fn selection_line_distance(
        &self,
        pointer: Pos2,
        start: Point3,
        end: Point3,
        rect: Rect,
    ) -> f32 {
        let Some([start, end]) = self.clip_segment(start, end) else {
            return f32::INFINITY;
        };
        let (Some(a), Some(b)) = (self.project(start, rect), self.project(end, rect)) else {
            return f32::INFINITY;
        };
        let distance = super::screen::point_segment_distance(pointer, a, b);
        if !distance.is_finite() || distance > PICK_CAPTURE_PIXELS {
            return distance;
        }
        let dx = f64::from(b.x) - f64::from(a.x);
        let dy = f64::from(b.y) - f64::from(a.y);
        let span = dx * dx + dy * dy;
        let depths = [self.view_depth(start), self.view_depth(end)];
        let first_dot = (f64::from(pointer.x) - f64::from(a.x))
            .mul_add(dx, (f64::from(pointer.y) - f64::from(a.y)) * dy);
        let last_dot = (f64::from(pointer.x) - f64::from(b.x))
            .mul_add(dx, (f64::from(pointer.y) - f64::from(b.y)) * dy);
        let depth = if span == 0. || first_dot <= 0. {
            depths[0]
        } else if last_dot >= 0. {
            depths[1]
        } else if self.kind.is_parallel() {
            // Retain the closest screen coordinate before interpolating depth.
            // A normalized fraction near 0.5 or 1 can erase local features.
            let area = super::screen::signed_area(a, b, pointer);
            let (parameters, coordinate) = if dx.abs() >= dy.abs() {
                (
                    [Real::from(a.x), Real::from(b.x)],
                    (area / span).mul_add(dy, Real::from(pointer.x)),
                )
            } else {
                (
                    [Real::from(a.y), Real::from(b.y)],
                    (-area / span).mul_add(dx, Real::from(pointer.y)),
                )
            };
            interpolate_scalar(
                depths,
                parameters,
                coordinate.clamp(
                    parameters[0].min(parameters[1]),
                    parameters[0].max(parameters[1]),
                ),
            )
            .unwrap_or(Real::NAN)
        } else {
            let fraction = (first_dot / span).clamp(0., 1.);
            let scale = depths[0].min(depths[1]);
            scale / ((1. - fraction) * (scale / depths[0]) + fraction * (scale / depths[1]))
        };
        let (near, far) = self.display_depth_interval();
        if depth.is_finite() && depth >= near && depth <= far {
            distance
        } else {
            f32::INFINITY
        }
    }

    /// Intersect a triangle with the depth slab. A resulting convex polygon
    /// has at most five vertices, so three triangles suffice without allocation.
    pub(super) fn clip_selection_triangle(&self, points: [Point3; 3]) -> [Option<[Point3; 3]>; 3] {
        let vertices = points.map(|point| Vertex {
            point,
            depth: self.view_depth(point),
        });
        if vertices.iter().any(|v| !v.depth.is_finite()) {
            return [None; 3];
        }
        let (mut near, far) = self.display_depth_interval();
        if self.kind == ViewKind::Perspective {
            near = near.max(self.primitive_near(&points));
        }
        let mut polygon = [vertices[0]; 5];
        polygon[..3].copy_from_slice(&vertices);
        let mut count = 3;
        for (plane, above) in [(near, true), (far, false)] {
            let mut output = [vertices[0]; 5];
            let mut written = 0;
            for end in 0..count {
                let a = polygon[(end + count - 1) % count];
                let b = polygon[end];
                let inside = |v: Vertex| {
                    if above {
                        v.depth >= plane
                    } else {
                        v.depth <= plane
                    }
                };
                if inside(a) != inside(b) {
                    let Some(intersection) = a.intersection(b, plane) else {
                        return [None; 3];
                    };
                    output[written] = intersection;
                    written += 1;
                }
                if inside(b) {
                    output[written] = b;
                    written += 1;
                }
            }
            polygon = output;
            count = written;
            if count < 3 {
                return [None; 3];
            }
        }
        std::array::from_fn(|i| {
            (i + 2 < count).then(|| [polygon[0].point, polygon[i + 1].point, polygon[i + 2].point])
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_segments_retain_distinct_depth_plane_intersections_in_both_directions() {
        let mut view = Viewport::new(ViewKind::Top);
        view.perspective_camera_distance = 100.;
        view.frustum_near = 104.;
        view.frustum_far = 112.;
        for scale in [1e24, 1e200, Real::MAX] {
            let points = [
                Point3::try_new(-scale, 0., scale).unwrap(),
                Point3::try_new(scale, 0., -scale).unwrap(),
            ];
            for order in [[0, 1], [1, 0]] {
                let clipped = view
                    .clip_selection_segment(points[order[0]], points[order[1]])
                    .unwrap();
                let depths = clipped.map(|point| view.view_depth(point));
                let expected = if order[0] == 0 { [4., 12.] } else { [12., 4.] };
                assert_eq!(depths, expected, "scale={scale} order={order:?}");
                assert_eq!(clipped.map(|point| point.x()), expected);
                assert_eq!(clipped.map(|point| point.y()), [0.; 2]);
            }
        }
    }

    #[test]
    fn long_parallel_line_picks_preserve_the_closest_screen_depth() {
        let mut view = Viewport::new(ViewKind::Top);
        view.perspective_camera_distance = 100.;
        view.frustum_near = 104.;
        view.frustum_far = 112.;
        view.pixels_per_unit = 1.;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.));
        // One endpoint is at screen x=128, depth=0. The other is too far
        // away for a fraction near one to retain a depth of eight.
        let points = [
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1e24, 0., -1e24).unwrap(),
        ];
        for order in [[0, 1], [1, 0]] {
            assert_eq!(
                view.selection_line_distance(
                    Pos2::new(136., 128.),
                    points[order[0]],
                    points[order[1]],
                    rect
                ),
                0.
            );
            assert!(
                !view
                    .selection_line_distance(
                        Pos2::new(144., 128.),
                        points[order[0]],
                        points[order[1]],
                        rect
                    )
                    .is_finite()
            );
        }
    }
}
