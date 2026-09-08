//! Shared camera projection, drafting rays, and checked navigation.

use super::*;
use nalgebra::Matrix4 as NaMatrix4;

impl Viewport {
    pub(super) fn unproject_drafting_plane(
        &self,
        pointer: Pos2,
        rect: Rect,
        anchor: Option<Point3>,
    ) -> Option<Point3> {
        let plane = self.construction_plane();
        let plane = plane.with_origin(anchor.unwrap_or(plane.origin()));
        let (origin, direction, forward_only) = if self.kind == ViewKind::Perspective {
            let (right, up, forward) = self.perspective_basis();
            let camera = self.target - forward * self.perspective_camera_distance;
            let origin = self.world_origin(rect);
            let focal = self.perspective_focal_length_pixels(rect);
            let ray = forward
                + right * ((Real::from(pointer.x) - Real::from(origin.x)) / focal)
                + up * ((Real::from(origin.y) - Real::from(pointer.y)) / focal);
            (
                Point3::try_new(camera.x, camera.y, camera.z).ok()?,
                Vector3::try_new(ray.x, ray.y, ray.z).ok()?,
                true,
            )
        } else {
            (
                self.unproject(pointer, rect, 0.0)?,
                self.apparent_intersection_normal(),
                false,
            )
        };
        viboceros_drafting::plane::intersect_view_line(origin, direction, plane, forward_only)
            .ok()
            .flatten()
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
                let camera = self.target - forward * self.perspective_camera_distance;
                let relative = NaVector3::new(point.x(), point.y(), point.z()) - camera;
                let depth = relative.dot(&forward);
                if !depth.is_finite() || depth <= 1.0e-6 {
                    return None;
                }
                let focal_length = self.perspective_focal_length_pixels(rect);
                (
                    relative.dot(&right) / depth * focal_length,
                    relative.dot(&up) / depth * focal_length,
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
                let (right, up, forward) = self.perspective_basis();
                let camera = self.target - forward * self.perspective_camera_distance;
                let focal_length = self.perspective_focal_length_pixels(rect);
                let horizontal = (Real::from(position.x) - Real::from(origin.x)) / focal_length;
                let vertical = (Real::from(origin.y) - Real::from(position.y)) / focal_length;
                let ray = forward + right * horizontal + up * vertical;
                if !ray.z.is_finite() || ray.z.abs() <= 1.0e-12 {
                    return None;
                }
                let parameter = (elevation - camera.z) / ray.z;
                if !parameter.is_finite() || parameter < 0.0 {
                    return None;
                }
                let point = camera + ray * parameter;
                Point3::try_new(point.x, point.y, point.z).ok()
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
        if !factor.is_finite()
            || factor <= 0.0
            || !rect.is_finite()
            || !rect.is_positive()
            || pointer.is_some_and(|pointer| !pointer.is_finite())
        {
            return;
        }
        if self.kind == ViewKind::Perspective {
            let old_distance = self.perspective_camera_distance;
            let new_distance = (old_distance / Real::from(factor)).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            if new_distance == old_distance {
                return;
            }
            // At the target plane, screen offsets scale by old/new distance.
            // This needs no world-plane intersection or large model subtraction.
            if let Some(pointer) = pointer {
                let Some(pan) = zoom_pan(self.pan, pointer, rect, old_distance / new_distance)
                else {
                    return;
                };
                self.pan = pan;
            }
            self.perspective_camera_distance = new_distance;
            return;
        }
        let old_scale = self.pixels_per_unit;
        let new_scale = (old_scale * factor).clamp(f32::MIN_POSITIVE, 2_000.0);
        if new_scale == old_scale {
            return;
        }
        if let Some(pointer) = pointer {
            let Some(pan) = zoom_pan(
                self.pan,
                pointer,
                rect,
                Real::from(new_scale) / Real::from(old_scale),
            ) else {
                return;
            };
            self.pan = pan;
        }
        self.pixels_per_unit = new_scale;
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
                let camera = self.target - forward * self.perspective_camera_distance;
                let view = NaMatrix4::new(
                    right.x,
                    right.y,
                    right.z,
                    -right.dot(&camera),
                    up.x,
                    up.y,
                    up.z,
                    -up.dot(&camera),
                    forward.x,
                    forward.y,
                    forward.z,
                    -forward.dot(&camera),
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                let (minimum_depth, maximum_depth) = depth_range.unwrap_or((
                    self.perspective_camera_distance * 0.5,
                    self.perspective_camera_distance * 1.5,
                ));
                let near = (minimum_depth * 0.5)
                    .max(self.perspective_camera_distance * 1.0e-6)
                    .max(1.0e-6);
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
                    offset_x - horizontal_scale * right.dot(&self.target),
                    vertical_scale * up.x,
                    vertical_scale * up.y,
                    vertical_scale * up.z,
                    offset_y - vertical_scale * up.dot(&self.target),
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
            ViewKind::Top => -point.z(),
            ViewKind::Front => point.y(),
            ViewKind::Right => -point.x(),
            ViewKind::Perspective => {
                let (_, _, forward) = self.perspective_basis();
                let camera = self.target - forward * self.perspective_camera_distance;
                (NaVector3::new(point.x(), point.y(), point.z()) - camera).dot(&forward)
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
