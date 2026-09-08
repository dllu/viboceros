//! Shared camera projection, drafting rays, and checked navigation.

use super::*;
use nalgebra::Matrix4 as NaMatrix4;

impl Viewport {
    /// Preserve small local features before the f64-to-f32 GPU boundary.
    pub(super) fn gpu_position(&self, point: Point3) -> Option<[f32; 3]> {
        Some([
            real_to_gpu(point.x() - self.target.x)?,
            real_to_gpu(point.y() - self.target.y)?,
            real_to_gpu(point.z() - self.target.z)?,
        ])
    }

    fn perspective_near_floor(&self) -> Real {
        (self.perspective_camera_distance * 1e-6).max(1e-6)
    }

    /// Clip the invisible part of a perspective segment before projecting its
    /// endpoints. The guard covers rounding when reconstructing a model point
    /// close to the camera plane; it is not a model-space geometry edit.
    pub(super) fn clip_segment(&self, start: Point3, end: Point3) -> Option<[Point3; 2]> {
        if self.kind.is_parallel() {
            return Some([start, end]);
        }
        self.clip_segment_at_near(start, end, self.primitive_near(&[start, end]))
    }

    fn primitive_near(&self, points: &[Point3]) -> Real {
        let scale = points
            .iter()
            .flat_map(|point| point.to_array())
            .chain(self.target.iter().copied())
            .fold(self.perspective_camera_distance.max(1.0), |scale, value| {
                scale.max(value.abs())
            });
        self.perspective_near_floor() + 64.0 * Real::EPSILON * scale
    }

    fn clip_segment_at_near(&self, start: Point3, end: Point3, near: Real) -> Option<[Point3; 2]> {
        let depths = [self.view_depth(start), self.view_depth(end)];
        if depths.iter().any(|depth| !depth.is_finite()) {
            return None;
        }
        if depths.iter().all(|depth| *depth < near) {
            return None;
        }
        if depths.iter().all(|depth| *depth >= near) {
            return Some([start, end]);
        }
        let fraction = (near - depths[0]) / (depths[1] - depths[0]);
        if !fraction.is_finite() {
            return None;
        }
        let a = start.to_array();
        let b = end.to_array();
        let clipped = Point3::try_from(std::array::from_fn(|axis| {
            (1.0 - fraction).mul_add(a[axis], fraction * b[axis])
        }))
        .ok()?;
        Some(if depths[0] < near {
            [clipped, end]
        } else {
            [start, clipped]
        })
    }

    /// A triangle clipped against one plane has at most four vertices. Keep
    /// its winding and triangulate the resulting polygon without allocating.
    pub(super) fn clip_triangle(&self, points: [Point3; 3]) -> [Option<[Point3; 3]>; 2] {
        if self.kind.is_parallel() {
            return [Some(points), None];
        }
        let near = self.primitive_near(&points);
        let depths = points.map(|point| self.view_depth(point));
        if depths.iter().any(|depth| !depth.is_finite()) {
            return [None, None];
        }
        if depths.iter().all(|depth| *depth >= near) {
            return [Some(points), None];
        }
        if depths.iter().all(|depth| *depth < near) {
            return [None, None];
        }
        let mut polygon = [points[0]; 4];
        let mut count = 0;
        for end in 0..3 {
            let start = (end + 2) % 3;
            let start_inside = depths[start] >= near;
            let end_inside = depths[end] >= near;
            if start_inside != end_inside {
                let Some(clipped) = self.clip_segment_at_near(points[start], points[end], near)
                else {
                    return [None, None];
                };
                polygon[count] = clipped[usize::from(start_inside)];
                count += 1;
            }
            if end_inside {
                polygon[count] = points[end];
                count += 1;
            }
        }
        match count {
            3 => [Some([polygon[0], polygon[1], polygon[2]]), None],
            4 => [
                Some([polygon[0], polygon[1], polygon[2]]),
                Some([polygon[0], polygon[2], polygon[3]]),
            ],
            _ => [None, None],
        }
    }

    pub(super) fn project_segment(
        &self,
        start: Point3,
        end: Point3,
        rect: Rect,
    ) -> Option<[Pos2; 2]> {
        let [start, end] = self.clip_segment(start, end)?;
        Some([self.project(start, rect)?, self.project(end, rect)?])
    }

    pub(super) fn unproject_drafting_plane(
        &self,
        pointer: Pos2,
        rect: Rect,
        anchor: Option<Point3>,
    ) -> Option<Point3> {
        let plane = self.construction_plane();
        let local_origin =
            NaVector3::from(anchor.unwrap_or(plane.origin()).to_array()) - self.target;
        let plane = plane
            .with_origin(Point3::try_from([local_origin.x, local_origin.y, local_origin.z]).ok()?);
        let (origin, direction, forward_only) = if self.kind == ViewKind::Perspective {
            let (camera, ray) = self.perspective_local_ray(pointer, rect);
            (
                Point3::try_new(camera.x, camera.y, camera.z).ok()?,
                Vector3::try_new(ray.x, ray.y, ray.z).ok()?,
                true,
            )
        } else {
            let screen_origin = self.world_origin(rect);
            let scale = Real::from(self.pixels_per_unit);
            let horizontal = (Real::from(pointer.x) - Real::from(screen_origin.x)) / scale;
            let vertical = (Real::from(screen_origin.y) - Real::from(pointer.y)) / scale;
            let origin = match self.kind {
                ViewKind::Top => [horizontal, vertical, 0.0],
                ViewKind::Front => [horizontal, 0.0, vertical],
                ViewKind::Right => [0.0, horizontal, vertical],
                ViewKind::Perspective => unreachable!(),
            };
            (
                Point3::try_from(origin).ok()?,
                self.apparent_intersection_normal(),
                false,
            )
        };
        let local =
            viboceros_drafting::plane::intersect_view_line(origin, direction, plane, forward_only)
                .ok()
                .flatten()?;
        Point3::try_new(
            local.x() + self.target.x,
            local.y() + self.target.y,
            local.z() + self.target.z,
        )
        .ok()
    }

    /// Camera origin and ray direction in the target-relative model frame.
    fn perspective_local_ray(
        &self,
        pointer: Pos2,
        rect: Rect,
    ) -> (NaVector3<Real>, NaVector3<Real>) {
        let (right, up, forward) = self.perspective_basis();
        let origin = self.world_origin(rect);
        let focal = self.perspective_focal_length_pixels(rect);
        let horizontal = (Real::from(pointer.x) - Real::from(origin.x)) / focal;
        let vertical = (Real::from(origin.y) - Real::from(pointer.y)) / focal;
        (
            -forward * self.perspective_camera_distance,
            forward + right * horizontal + up * vertical,
        )
    }

    pub(super) fn apply_navigation_drag(
        &mut self,
        button: PointerButton,
        modifiers: egui::Modifiers,
        delta: Vec2,
    ) {
        if !delta.is_finite() {
            return;
        }
        if button == PointerButton::Middle
            || (button == PointerButton::Secondary && (self.kind.is_parallel() || modifiers.shift))
        {
            let pan = self.pan + delta;
            if pan.is_finite() {
                self.pan = pan;
            }
        } else if button == PointerButton::Secondary {
            self.orbit_yaw -= Real::from(delta.x) * 0.01;
            self.orbit_pitch = (self.orbit_pitch + Real::from(delta.y) * 0.01)
                .clamp(-1.553_343_034_274_953_2, 1.553_343_034_274_953_2);
        }
    }

    pub(super) fn world_origin(&self, rect: Rect) -> Pos2 {
        rect.center() + self.pan
    }

    pub(super) fn project(&self, point: Point3, rect: Rect) -> Option<Pos2> {
        let origin = self.world_origin(rect);
        let local = NaVector3::new(point.x(), point.y(), point.z()) - self.target;
        let (horizontal_pixels, vertical_pixels) = match self.kind {
            ViewKind::Top => (
                local.x * f64::from(self.pixels_per_unit),
                local.y * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Front => (
                local.x * f64::from(self.pixels_per_unit),
                local.z * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Right => (
                local.y * f64::from(self.pixels_per_unit),
                local.z * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Perspective => {
                let (right, up, forward) = self.perspective_basis();
                let depth = local.dot(&forward) + self.perspective_camera_distance;
                if !depth.is_finite() || depth <= 1.0e-6 {
                    return None;
                }
                let focal_length = self.perspective_focal_length_pixels(rect);
                (
                    local.dot(&right) / depth * focal_length,
                    local.dot(&up) / depth * focal_length,
                )
            }
        };
        let x = f64::from(origin.x) + horizontal_pixels;
        let y = f64::from(origin.y) - vertical_pixels;
        if !x.is_finite()
            || !y.is_finite()
            || x.abs() > f64::from(f32::MAX)
            || y.abs() > f64::from(f32::MAX)
        {
            return None;
        }
        Some(Pos2::new(x as f32, y as f32))
    }

    pub(super) fn unproject(&self, position: Pos2, rect: Rect, elevation: Real) -> Option<Point3> {
        let origin = self.world_origin(rect);
        match self.kind {
            ViewKind::Top | ViewKind::Front | ViewKind::Right => {
                let scale = Real::from(self.pixels_per_unit);
                let horizontal = (Real::from(position.x) - Real::from(origin.x)) / scale;
                let vertical = (Real::from(origin.y) - Real::from(position.y)) / scale;
                match self.kind {
                    ViewKind::Top => Point3::try_new(
                        horizontal + self.target.x,
                        vertical + self.target.y,
                        elevation,
                    )
                    .ok(),
                    ViewKind::Front => Point3::try_new(
                        horizontal + self.target.x,
                        elevation,
                        vertical + self.target.z,
                    )
                    .ok(),
                    ViewKind::Right => Point3::try_new(
                        elevation,
                        horizontal + self.target.y,
                        vertical + self.target.z,
                    )
                    .ok(),
                    ViewKind::Perspective => unreachable!(),
                }
            }
            ViewKind::Perspective => {
                let (camera, ray) = self.perspective_local_ray(position, rect);
                if !ray.z.is_finite() || ray.z.abs() <= 1.0e-12 {
                    return None;
                }
                let parameter = ((elevation - self.target.z) - camera.z) / ray.z;
                if !parameter.is_finite() || parameter < 0.0 {
                    return None;
                }
                let point = camera + ray * parameter;
                Point3::try_new(point.x + self.target.x, point.y + self.target.y, elevation).ok()
            }
        }
    }

    pub(super) fn perspective_basis(&self) -> (NaVector3<Real>, NaVector3<Real>, NaVector3<Real>) {
        let outward = NaVector3::new(
            self.orbit_pitch.cos() * self.orbit_yaw.cos(),
            self.orbit_pitch.cos() * self.orbit_yaw.sin(),
            self.orbit_pitch.sin(),
        );
        let forward = -outward;
        let world_up = NaVector3::new(0.0, 0.0, 1.0);
        let right = forward.cross(&world_up).normalize();
        let up = right.cross(&forward).normalize();
        (right, up, forward)
    }

    pub(super) fn perspective_focal_length_pixels(&self, rect: Rect) -> Real {
        let viewport_height = Real::from(rect.height().max(1.0));
        viewport_height / (2.0 * (PERSPECTIVE_VERTICAL_FOV_RADIANS / 2.0).tan())
    }

    pub(super) fn pixels_per_model_unit_at_origin(&self, rect: Rect) -> f32 {
        match self.kind {
            ViewKind::Perspective => {
                (self.perspective_focal_length_pixels(rect) / self.perspective_camera_distance)
                    as f32
            }
            ViewKind::Top | ViewKind::Front | ViewKind::Right => self.pixels_per_unit,
        }
    }

    pub(super) fn zoom_by(&mut self, factor: f32, pointer: Option<Pos2>, rect: Rect) {
        let _ = self.zoom_by_factor(Real::from(factor), pointer, rect);
    }

    pub(crate) fn zoom_factor(&mut self, factor: Real) -> Result<bool, &'static str> {
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        self.zoom_by_factor(factor, Some(rect.center()), rect)
    }

    fn zoom_by_factor(
        &mut self,
        factor: Real,
        pointer: Option<Pos2>,
        rect: Rect,
    ) -> Result<bool, &'static str> {
        if !factor.is_finite()
            || factor <= 0.0
            || !rect.is_finite()
            || !rect.is_positive()
            || !rect.width().is_finite()
            || !rect.height().is_finite()
            || pointer.is_some_and(|pointer| !pointer.is_finite())
        {
            return Err("invalid zoom factor or viewport coordinates");
        }
        if self.kind == ViewKind::Perspective {
            let old_distance = self.perspective_camera_distance;
            let new_distance = (old_distance / factor).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            if new_distance == old_distance {
                return Ok(false);
            }
            // At the target plane, screen offsets scale by old/new distance.
            // This needs no world-plane intersection or large model subtraction.
            if let Some(pointer) = pointer {
                let Some(pan) = zoom_pan(self.pan, pointer, rect, old_distance / new_distance)
                else {
                    return Err("zoom exceeds the screen-coordinate range");
                };
                self.pan = pan;
            }
            self.perspective_camera_distance = new_distance;
            return Ok(true);
        }
        let old_scale = self.pixels_per_unit;
        let new_scale =
            (Real::from(old_scale) * factor).clamp(Real::from(f32::MIN_POSITIVE), 2_000.0) as f32;
        if new_scale == old_scale {
            return Ok(false);
        }
        if let Some(pointer) = pointer {
            let Some(pan) = zoom_pan(
                self.pan,
                pointer,
                rect,
                Real::from(new_scale) / Real::from(old_scale),
            ) else {
                return Err("zoom exceeds the screen-coordinate range");
            };
            self.pan = pan;
        }
        self.pixels_per_unit = new_scale;
        Ok(true)
    }

    pub(super) fn gpu_view_uniform(
        &self,
        rect: Rect,
        depth_range: Option<(Real, Real)>,
    ) -> GpuViewUniform {
        let width = Real::from(rect.width().max(1.0));
        let height = Real::from(rect.height().max(1.0));
        let offset_x = 2.0 * Real::from(self.pan.x) / width;
        let offset_y = -2.0 * Real::from(self.pan.y) / height;
        let view_projection = match self.kind {
            ViewKind::Perspective => {
                let (right, up, forward) = self.perspective_basis();
                let view = NaMatrix4::new(
                    right.x,
                    right.y,
                    right.z,
                    0.0,
                    up.x,
                    up.y,
                    up.z,
                    0.0,
                    forward.x,
                    forward.y,
                    forward.z,
                    self.perspective_camera_distance,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                let (minimum_depth, maximum_depth) = depth_range.unwrap_or((
                    self.perspective_camera_distance * 0.5,
                    self.perspective_camera_distance * 1.5,
                ));
                let near = (minimum_depth * 0.5).max(self.perspective_near_floor());
                let far = (maximum_depth * 1.5)
                    .max(self.perspective_camera_distance * 2.0)
                    .max(near + 1.0);
                let focal_length = self.perspective_focal_length_pixels(rect);
                let projection = NaMatrix4::new(
                    2.0 * focal_length / width,
                    0.0,
                    offset_x,
                    0.0,
                    0.0,
                    2.0 * focal_length / height,
                    offset_y,
                    0.0,
                    0.0,
                    0.0,
                    far / (far - near),
                    -far * near / (far - near),
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                );
                projection * view
            }
            ViewKind::Top | ViewKind::Front | ViewKind::Right => {
                let (right, up, forward) = match self.kind {
                    ViewKind::Top => (
                        NaVector3::new(1.0, 0.0, 0.0),
                        NaVector3::new(0.0, 1.0, 0.0),
                        NaVector3::new(0.0, 0.0, -1.0),
                    ),
                    ViewKind::Front => (
                        NaVector3::new(1.0, 0.0, 0.0),
                        NaVector3::new(0.0, 0.0, 1.0),
                        NaVector3::new(0.0, 1.0, 0.0),
                    ),
                    ViewKind::Right => (
                        NaVector3::new(0.0, 1.0, 0.0),
                        NaVector3::new(0.0, 0.0, 1.0),
                        NaVector3::new(-1.0, 0.0, 0.0),
                    ),
                    ViewKind::Perspective => unreachable!(),
                };
                let (minimum_depth, maximum_depth) = depth_range.unwrap_or((-1.0, 1.0));
                let span = (maximum_depth - minimum_depth).abs().max(1.0);
                let near = minimum_depth - span * 0.05 - 1.0e-3;
                let far = maximum_depth + span * 0.05 + 1.0e-3;
                let depth_span = far - near;
                let horizontal_scale = 2.0 * Real::from(self.pixels_per_unit) / width;
                let vertical_scale = 2.0 * Real::from(self.pixels_per_unit) / height;
                NaMatrix4::new(
                    horizontal_scale * right.x,
                    horizontal_scale * right.y,
                    horizontal_scale * right.z,
                    offset_x,
                    vertical_scale * up.x,
                    vertical_scale * up.y,
                    vertical_scale * up.z,
                    offset_y,
                    forward.x / depth_span,
                    forward.y / depth_span,
                    forward.z / depth_span,
                    -near / depth_span,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                )
            }
        };

        GpuViewUniform {
            view_projection: matrix_to_gpu(view_projection),
            viewport_size: [rect.width().max(1.0), rect.height().max(1.0)],
            padding: [0.0; 2],
        }
    }

    pub(super) fn view_depth(&self, point: Point3) -> Real {
        match self.kind {
            ViewKind::Top => self.target.z - point.z(),
            ViewKind::Front => point.y() - self.target.y,
            ViewKind::Right => self.target.x - point.x(),
            ViewKind::Perspective => {
                let (_, _, forward) = self.perspective_basis();
                (NaVector3::new(point.x(), point.y(), point.z()) - self.target).dot(&forward)
                    + self.perspective_camera_distance
            }
        }
    }
}

// Compute in f64 before converting the final pan: f32 screen deltas and
// scale ratios can overflow even when their final combination is representable.
pub(super) fn zoom_pan(pan: Vec2, pointer: Pos2, rect: Rect, ratio: Real) -> Option<Vec2> {
    let center = rect.center();
    let component = |pan: f32, pointer: f32, center: f32| {
        let origin = Real::from(center) + Real::from(pan);
        (Real::from(pointer) - (Real::from(pointer) - origin) * ratio - Real::from(center)) as f32
    };
    let result = Vec2::new(
        component(pan.x, pointer.x, center.x),
        component(pan.y, pointer.y, center.y),
    );
    result.is_finite().then_some(result)
}

fn matrix_to_gpu(matrix: NaMatrix4<Real>) -> [[f32; 4]; 4] {
    std::array::from_fn(|column| std::array::from_fn(|row| matrix[(row, column)] as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_zoom_factor_pins_view_center_and_stages_failures() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport {
                last_rect: Some(rect),
                pan: Vec2::new(17.0, -23.0),
                ..Viewport::new(kind)
            };
            let plane = view.plane.clone();
            let before = (
                view.pan,
                view.pixels_per_unit,
                view.perspective_camera_distance,
            );
            assert_eq!(view.zoom_factor(2.0), Ok(true));
            assert_eq!(view.pan, before.0 * 2.0);
            if kind.is_parallel() {
                assert_eq!(view.pixels_per_unit, before.1 * 2.0);
            } else {
                assert_eq!(view.perspective_camera_distance, before.2 / 2.0);
            }
            assert_eq!(view.zoom_factor(0.5), Ok(true));
            assert_eq!(
                (
                    view.pan,
                    view.pixels_per_unit,
                    view.perspective_camera_distance
                ),
                before
            );
            assert_eq!(view.plane, plane);
            assert_eq!(view.zoom_factor(1.0), Ok(false));
            for factor in [0.0, -1.0, Real::NAN, Real::INFINITY] {
                assert!(view.zoom_factor(factor).is_err());
                assert_eq!(
                    (
                        view.pan,
                        view.pixels_per_unit,
                        view.perspective_camera_distance
                    ),
                    before
                );
            }
            view.pan = Vec2::splat(f32::MAX);
            assert!(view.zoom_factor(2.0).is_err());
            assert_eq!(view.pan, Vec2::splat(f32::MAX));
            assert_eq!(
                (view.pixels_per_unit, view.perspective_camera_distance),
                (before.1, before.2)
            );
            view.pan = Vec2::ZERO;
            assert_eq!(view.zoom_factor(Real::MAX), Ok(true));
            assert_eq!(view.zoom_factor(Real::MAX), Ok(false));
            assert_eq!(view.zoom_factor(Real::from_bits(1)), Ok(true));
            assert_eq!(view.zoom_factor(Real::from_bits(1)), Ok(false));
        }
        assert!(Viewport::default().zoom_factor(2.0).is_err());
    }

    #[test]
    fn perspective_unprojection_rounds_world_translation_only_at_the_end() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let view = Viewport::new(ViewKind::Perspective);
        for pointer in [
            rect.center(),
            Pos2::new(437.0, 279.0),
            Pos2::new(211.0, 367.0),
        ] {
            let local = view.unproject(pointer, rect, 0.0).unwrap();
            for offset in [1e9, 1e12, 281474976710656.0] {
                let target = NaVector3::new(offset, -2.0 * offset, 3.0 * offset);
                let translated = Viewport {
                    target,
                    ..Viewport::new(ViewKind::Perspective)
                };
                let expected =
                    Point3::try_new(local.x() + target.x, local.y() + target.y, target.z).unwrap();
                assert_eq!(
                    translated.unproject(pointer, rect, target.z),
                    Some(expected),
                    "offset={offset}, pointer={pointer:?}"
                );
            }
        }
    }

    #[test]
    fn drafting_plane_unprojection_is_translation_covariant() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let pointer = Pos2::new(437.0, 279.0);
        let target = NaVector3::new(1e12, -2e12, 3e12);
        let translate = |p: Point3| {
            Point3::try_new(p.x() + target.x, p.y() + target.y, p.z() + target.z).unwrap()
        };
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport::new(kind);
            let default_frame = view
                .construction_plane()
                .with_origin(Point3::try_new(4.0, 5.0, 6.0).unwrap());
            let tilted_frame = viboceros_geometry::Frame3::try_from_normal(
                default_frame.origin(),
                Vector3::try_new(1.0, 2.0, 3.0).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            for frame in [default_frame, tilted_frame] {
                view.plane.set(frame);
                let mut translated = Viewport {
                    target,
                    ..Viewport::new(kind)
                };
                translated
                    .plane
                    .set(frame.with_origin(translate(frame.origin())));
                for anchor in [None, Some(Point3::try_new(1.0, 2.0, 3.0).unwrap())] {
                    let expected = view
                        .unproject_drafting_plane(pointer, rect, anchor)
                        .map(translate);
                    assert!(expected.is_some());
                    assert_eq!(
                        translated.unproject_drafting_plane(pointer, rect, anchor.map(translate)),
                        expected,
                        "{kind:?} anchor={anchor:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn local_drafting_rays_still_reject_edge_on_and_behind_camera_planes() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        for offset in [0.0, 1e12] {
            let mut view = Viewport {
                target: NaVector3::repeat(offset),
                orbit_yaw: 0.0,
                orbit_pitch: 0.0,
                ..Viewport::new(ViewKind::Perspective)
            };
            for (origin, normal) in [
                ([offset, offset, offset], [0.0, 1.0, 0.0]),
                ([offset + 60.0, offset, offset], [1.0, 0.0, 0.0]),
            ] {
                view.plane.set(
                    viboceros_geometry::Frame3::try_from_normal(
                        Point3::try_from(origin).unwrap(),
                        Vector3::try_new(normal[0], normal[1], normal[2]).unwrap(),
                        Tolerance::DEFAULT,
                    )
                    .unwrap(),
                );
                assert!(
                    view.unproject_drafting_plane(rect.center(), rect, None)
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn unprojection_subtracts_finite_screen_coordinates_without_f32_overflow() {
        let rect = Rect::from_center_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let pointer = Pos2::new(-f32::MAX, f32::MAX);
        let horizontal = -2.0 * Real::from(f32::MAX) / 40.0;
        for kind in [ViewKind::Top, ViewKind::Front, ViewKind::Right] {
            let view = Viewport {
                pan: Vec2::new(f32::MAX, -f32::MAX),
                ..Viewport::new(kind)
            };
            let actual = view.unproject(pointer, rect, 0.0).unwrap();
            let expected = match kind {
                ViewKind::Top => [horizontal, horizontal, 0.0],
                ViewKind::Front => [horizontal, 0.0, horizontal],
                ViewKind::Right => [0.0, horizontal, horizontal],
                _ => unreachable!(),
            };
            assert_eq!(actual.to_array(), expected);
            assert_eq!(
                view.unproject_drafting_plane(pointer, rect, None).unwrap(),
                actual
            );
        }
        let pointer = Pos2::new(-f32::MAX, 0.0);
        let mut view = Viewport {
            pan: Vec2::new(f32::MAX, 0.0),
            orbit_yaw: 0.0,
            ..Viewport::new(ViewKind::Perspective)
        };
        let actual = view.unproject(pointer, rect, 0.0).unwrap();
        let expected_y = -2.0 * Real::from(f32::MAX) / view.perspective_focal_length_pixels(rect)
            * view.perspective_camera_distance;
        assert!((actual.y() / expected_y - 1.0).abs() < 1e-14);
        assert!(actual.z().abs() < 1e-12);
        // The XY plane is genuinely near-parallel at this extreme screen
        // offset. Retain drafting's angular rejection policy.
        assert!(view.unproject_drafting_plane(pointer, rect, None).is_none());
        view.plane.set(
            WorldPlane::Front
                .frame()
                .with_origin(Point3::try_new(0.0, -1.0, 0.0).unwrap()),
        );
        let drafted = view.unproject_drafting_plane(pointer, rect, None).unwrap();
        assert!((drafted.y() + 1.0).abs() < 1e-12);
    }
}
