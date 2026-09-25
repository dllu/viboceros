//! Shared camera projection, drafting rays, and checked navigation.

use super::*;
use nalgebra::{Matrix4 as NaMatrix4, Unit, UnitQuaternion};

#[derive(Clone, Copy)]
struct ParallelAxes {
    right: (usize, Real),
    up: (usize, Real),
    forward: (usize, Real),
}

impl ViewKind {
    fn parallel_axes(self) -> Option<ParallelAxes> {
        let axes = match self {
            Self::Top => ParallelAxes {
                right: (0, 1.0),
                up: (1, 1.0),
                forward: (2, -1.0),
            },
            Self::Bottom => ParallelAxes {
                right: (0, 1.0),
                up: (1, -1.0),
                forward: (2, 1.0),
            },
            Self::Front => ParallelAxes {
                right: (0, 1.0),
                up: (2, 1.0),
                forward: (1, 1.0),
            },
            Self::Back => ParallelAxes {
                right: (0, -1.0),
                up: (2, 1.0),
                forward: (1, -1.0),
            },
            Self::Right => ParallelAxes {
                right: (1, 1.0),
                up: (2, 1.0),
                forward: (0, -1.0),
            },
            Self::Left => ParallelAxes {
                right: (1, -1.0),
                up: (2, 1.0),
                forward: (0, 1.0),
            },
            Self::Plan | Self::Perspective => return None,
        };
        Some(axes)
    }
}

fn real_to_gpu(value: Real) -> Option<f32> {
    (value.is_finite() && value.abs() <= Real::from(f32::MAX)).then_some(value as f32)
}

impl Viewport {
    /// Direction from the model toward the camera for face-angle commands.
    pub(crate) fn viewward_direction(&self) -> Vector3 {
        match self.kind {
            ViewKind::Perspective => {
                let (_, _, forward) = self.perspective_basis();
                Vector3::try_new(-forward.x, -forward.y, -forward.z)
                    .expect("perspective camera direction is finite")
            }
            ViewKind::Plan => self.plan_frame.z_axis().as_vector(),
            kind => {
                let (axis, sign) = kind.parallel_axes().expect("orthographic view").forward;
                let mut direction = [0.0; 3];
                direction[axis] = -sign;
                Vector3::try_from(direction).expect("orthographic camera direction is finite")
            }
        }
    }

    pub(super) fn point_cloud_projection(
        &self,
    ) -> Option<viboceros_geometry::PointCloudProjection> {
        use viboceros_geometry::PointCloudProjection;
        match self.kind {
            ViewKind::Top | ViewKind::Bottom => Some(PointCloudProjection::Xy),
            ViewKind::Front | ViewKind::Back => Some(PointCloudProjection::Xz),
            ViewKind::Right | ViewKind::Left => Some(PointCloudProjection::Yz),
            ViewKind::Plan | ViewKind::Perspective => None,
        }
    }

    pub(super) fn parallel_query_offset(&self, pointer: Pos2, rect: Rect) -> Option<[Real; 2]> {
        let signs = if self.kind == ViewKind::Plan {
            [1.0, 1.0]
        } else {
            let axes = self.kind.parallel_axes()?;
            [axes.right.1, axes.up.1]
        };
        let origin = self.world_origin(rect);
        let scale = Real::from(self.pixels_per_unit);
        Some([
            (Real::from(pointer.x) - Real::from(origin.x)) / scale * signs[0],
            (Real::from(origin.y) - Real::from(pointer.y)) / scale * signs[1],
        ])
    }

    /// Preserve local features before the f64-to-f32 GPU boundary. Parallel
    /// views also apply their uniform model-to-pixel scale here so GPU matrix
    /// coefficients do not become subnormal merely because the model is large.
    pub(super) fn gpu_position(&self, point: Point3) -> Option<[f32; 3]> {
        if self.kind == ViewKind::Plan {
            let local = self
                .plan_target_frame()?
                .projected_coordinates_of(point)
                .ok()?;
            let scale = Real::from(self.pixels_per_unit);
            return Some([
                real_to_gpu(local[0] * scale)?,
                real_to_gpu(local[1] * scale)?,
                0.0,
            ]);
        }
        let mut local = [
            point.x() - self.target.x,
            point.y() - self.target.y,
            point.z() - self.target.z,
        ];
        if let Some((axis, _)) = self.parallel_depth_axis() {
            if !local[axis].is_finite() {
                return None;
            }
            // Encode depth only after the complete scene's f64 range is known.
            local[axis] = 0.0;
        }
        let scale = if self.kind.is_parallel() {
            Real::from(self.pixels_per_unit)
        } else {
            1.0
        };
        Some([
            real_to_gpu(local[0] * scale)?,
            real_to_gpu(local[1] * scale)?,
            real_to_gpu(local[2] * scale)?,
        ])
    }

    fn parallel_depth_axis(&self) -> Option<(usize, f32)> {
        if self.kind == ViewKind::Plan {
            return Some((2, -1.0));
        }
        self.kind
            .parallel_axes()
            .map(|axes| (axes.forward.0, axes.forward.1 as f32))
    }

    pub(super) fn plan_target_frame(&self) -> Option<Frame3> {
        Some(
            self.plan_frame
                .with_origin(Point3::try_from([self.target.x, self.target.y, self.target.z]).ok()?),
        )
    }

    pub(super) fn encode_gpu_depth(
        &self,
        position: &mut [f32],
        depth: Real,
        range: Option<(Real, Real)>,
    ) {
        let Some((axis, sign)) = self.parallel_depth_axis() else {
            return;
        };
        let encoded = if let Some((minimum, maximum)) = range
            && minimum < maximum
        {
            let span = maximum - minimum;
            let fraction = if span.is_finite() {
                (depth - minimum) / span
            } else {
                (depth * 0.5 - minimum * 0.5) / (maximum * 0.5 - minimum * 0.5)
            };
            // Leave five percent at either end for clipping/bias, without
            // adding an absolute padding that can round away at large depths.
            (fraction * 0.9 + 0.05) as f32
        } else {
            0.5
        };
        position[axis] = sign * encoded;
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
            let origin = if self.kind == ViewKind::Plan {
                self.plan_frame
                    .with_origin(Point3::try_new(0.0, 0.0, 0.0).ok()?)
                    .point_at([horizontal, vertical, 0.0])
                    .ok()?
                    .to_array()
            } else {
                let axes = self.kind.parallel_axes().expect("parallel view");
                let mut origin = [0.0; 3];
                origin[axes.right.0] = horizontal * axes.right.1;
                origin[axes.up.0] = vertical * axes.up.1;
                origin
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
            if self.kind == ViewKind::Perspective {
                let Some(rect) = self
                    .last_rect
                    .filter(|rect| rect.is_finite() && rect.is_positive())
                else {
                    return;
                };
                let (right, up, _) = self.perspective_basis();
                let scale =
                    self.perspective_camera_distance / self.perspective_focal_length_pixels(rect);
                let target = self.target - right * (Real::from(delta.x) * scale)
                    + up * (Real::from(delta.y) * scale);
                if target.iter().all(|value| value.is_finite()) {
                    self.target = target;
                }
            } else {
                let pan = self.pan + delta;
                if pan.is_finite() {
                    self.pan = pan;
                }
            }
        } else if button == PointerButton::Secondary {
            if let Some(frame) = self.perspective_frame {
                let (right, up, _) = self.perspective_basis();
                let yaw = UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(up),
                    -Real::from(delta.x) * 0.01,
                );
                let pitch = UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(yaw.transform_vector(&right)),
                    Real::from(delta.y) * 0.01,
                );
                let rotation = pitch * yaw;
                let new_right = rotation.transform_vector(&right);
                let new_up = rotation.transform_vector(&up);
                if let Ok(camera_frame) = Frame3::try_from_directions(
                    frame.origin(),
                    Vector3::try_from([new_right.x, new_right.y, new_right.z])
                        .expect("finite camera axis"),
                    Vector3::try_from([new_up.x, new_up.y, new_up.z]).expect("finite camera axis"),
                    Tolerance::DEFAULT,
                ) {
                    self.perspective_frame = Some(camera_frame);
                    self.cplane_direction = None;
                }
            } else {
                self.orbit_yaw -= Real::from(delta.x) * 0.01;
                self.orbit_pitch = (self.orbit_pitch + Real::from(delta.y) * 0.01)
                    .clamp(-1.553_343_034_274_953_2, 1.553_343_034_274_953_2);
            }
        }
    }

    pub(super) fn world_origin(&self, rect: Rect) -> Pos2 {
        if self.kind == ViewKind::Perspective {
            rect.center()
                + egui::vec2(
                    (-self.perspective_lens_shift[0] * Real::from(rect.width()) * 0.5) as f32,
                    (self.perspective_lens_shift[1] * Real::from(rect.height()) * 0.5) as f32,
                )
        } else {
            rect.center() + self.pan
        }
    }

    pub(super) fn project(&self, point: Point3, rect: Rect) -> Option<Pos2> {
        self.project_precise(point, rect)
            .map(|[x, y]| Pos2::new(x as f32, y as f32))
    }

    /// Anchor a view-based fence click on the plane through the camera target.
    /// This plane is perpendicular to the viewing direction, including in
    /// perspective, so accepted vertices follow the model through navigation.
    pub(crate) fn fence_anchor(&self, pointer: Pos2) -> Option<Point3> {
        let rect = self.last_rect?;
        if !pointer.is_finite() || !rect.contains(pointer) {
            return None;
        }
        let origin = self.world_origin(rect);
        let dx = Real::from(pointer.x) - Real::from(origin.x);
        let dy = Real::from(origin.y) - Real::from(pointer.y);
        match self.kind {
            ViewKind::Perspective => {
                let (right, up, _) = self.perspective_basis();
                let scale =
                    self.perspective_camera_distance / self.perspective_focal_length_pixels(rect);
                let local = right * (dx * scale) + up * (dy * scale);
                Point3::try_from([
                    self.target.x + local.x,
                    self.target.y + local.y,
                    self.target.z + local.z,
                ])
                .ok()
            }
            ViewKind::Plan => {
                let scale = Real::from(self.pixels_per_unit);
                self.plan_target_frame()?
                    .point_at([dx / scale, dy / scale, 0.0])
                    .ok()
            }
            _ => {
                let scale = Real::from(self.pixels_per_unit);
                let axes = self.kind.parallel_axes()?;
                let mut point = [self.target.x, self.target.y, self.target.z];
                point[axes.right.0] += dx / scale * axes.right.1;
                point[axes.up.0] += dy / scale * axes.up.1;
                Point3::try_from(point).ok()
            }
        }
    }

    pub(crate) fn project_fence(&self, points: &[Point3]) -> Option<Vec<Pos2>> {
        let rect = self.last_rect?;
        points
            .iter()
            .map(|point| self.project(*point, rect))
            .collect()
    }

    /// Keep model-point query minimization independent of egui's f32 raster coordinates.
    pub(super) fn project_precise(&self, point: Point3, rect: Rect) -> Option<[Real; 2]> {
        let origin = self.world_origin(rect);
        let (horizontal_pixels, vertical_pixels) = match self.kind {
            ViewKind::Plan => {
                let coordinates = self
                    .plan_target_frame()?
                    .projected_coordinates_of(point)
                    .ok()?;
                (
                    coordinates[0] * f64::from(self.pixels_per_unit),
                    coordinates[1] * f64::from(self.pixels_per_unit),
                )
            }
            ViewKind::Perspective => {
                let local = NaVector3::new(point.x(), point.y(), point.z()) - self.target;
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
            _ => {
                let local = NaVector3::new(point.x(), point.y(), point.z()) - self.target;
                let axes = self.kind.parallel_axes().expect("parallel view");
                (
                    local[axes.right.0] * axes.right.1 * f64::from(self.pixels_per_unit),
                    local[axes.up.0] * axes.up.1 * f64::from(self.pixels_per_unit),
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
        Some([x, y])
    }

    // Coordinate-plane reference used by projection tests. Interactive drafting
    // uses unproject_drafting_plane; camera-space capture needs no world cursor.
    #[cfg(test)]
    pub(super) fn unproject(&self, position: Pos2, rect: Rect, elevation: Real) -> Option<Point3> {
        let origin = self.world_origin(rect);
        if self.kind == ViewKind::Plan {
            let scale = Real::from(self.pixels_per_unit);
            let horizontal = (Real::from(position.x) - Real::from(origin.x)) / scale;
            let vertical = (Real::from(origin.y) - Real::from(position.y)) / scale;
            return self
                .plan_target_frame()?
                .point_at([horizontal, vertical, elevation])
                .ok();
        }
        match self.kind.parallel_axes() {
            Some(axes) => {
                let scale = Real::from(self.pixels_per_unit);
                let horizontal = (Real::from(position.x) - Real::from(origin.x)) / scale;
                let vertical = (Real::from(origin.y) - Real::from(position.y)) / scale;
                let mut point = [self.target.x, self.target.y, self.target.z];
                point[axes.right.0] += horizontal * axes.right.1;
                point[axes.up.0] += vertical * axes.up.1;
                point[axes.forward.0] = elevation;
                Point3::try_from(point).ok()
            }
            None => {
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
        if let Some(frame) = self.perspective_frame {
            return (
                NaVector3::from(frame.x_axis().as_vector().to_array()),
                NaVector3::from(frame.y_axis().as_vector().to_array()),
                -NaVector3::from(frame.z_axis().as_vector().to_array()),
            );
        }
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
        viewport_height / (2.0 * (self.perspective_fov_radians / 2.0).tan())
    }

    pub(super) fn pixels_per_model_unit_at_origin(&self, rect: Rect) -> f32 {
        match self.kind {
            ViewKind::Perspective => {
                (self.perspective_focal_length_pixels(rect) / self.perspective_camera_distance)
                    as f32
            }
            _ => self.pixels_per_unit,
        }
    }

    pub(super) fn zoom_by(&mut self, factor: f32, pointer: Option<Pos2>, rect: Rect) {
        let _ = self.zoom_by_factor(Real::from(factor), pointer, rect);
    }

    pub(crate) fn zoom_factor(&mut self, factor: Real) -> Result<bool, &'static str> {
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        self.zoom_by_factor(factor, Some(rect.center()), rect)
    }

    pub(crate) fn zoom_in(&mut self, scale: Real) -> Result<bool, &'static str> {
        if !scale.is_finite() || scale <= 0.0 || !scale.recip().is_finite() {
            return Err("invalid view zoom scale factor");
        }
        self.zoom_factor(scale.recip())
    }

    pub(crate) fn zoom_out(&mut self, scale: Real) -> Result<bool, &'static str> {
        if !scale.is_finite() || scale <= 0.0 || !scale.recip().is_finite() {
            return Err("invalid view zoom scale factor");
        }
        self.zoom_factor(scale)
    }

    /// Fit a screen-space rectangle while keeping its center on the target-depth
    /// plane at the center of the viewport. Commit only after all values check.
    pub(crate) fn zoom_window(&mut self, window: Rect, rect: Rect) -> Result<bool, &'static str> {
        if !rect.is_finite()
            || !rect.is_positive()
            || !window.is_finite()
            || window.width() < 2.0
            || window.height() < 2.0
            || !rect.contains_rect(window)
        {
            return Err("invalid zoom window or viewport coordinates");
        }
        let factor = (Real::from(rect.width()) / Real::from(window.width()))
            .min(Real::from(rect.height()) / Real::from(window.height()));
        if !factor.is_finite() || factor <= 0.0 {
            return Err("invalid zoom window or viewport coordinates");
        }
        let previous = self.camera_snapshot();
        let center = window.center();
        let viewport_center = rect.center();
        if self.kind == ViewKind::Perspective {
            let old_distance = self.perspective_camera_distance;
            let new_distance = (old_distance / factor).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            let focal = self.perspective_focal_length_pixels(rect);
            let (right, up, _) = self.perspective_basis();
            let principal_point = self.world_origin(rect);
            let horizontal = (Real::from(center.x) - Real::from(principal_point.x)) / focal;
            let vertical = (Real::from(principal_point.y) - Real::from(center.y)) / focal;
            let target = self.target + (right * horizontal + up * vertical) * old_distance;
            if !target.iter().all(|value| value.is_finite()) {
                return Err("zoom exceeds the model-coordinate range");
            }
            let changed = target != self.target || new_distance != old_distance;
            self.target = target;
            self.perspective_camera_distance = new_distance;
            self.record_camera_change(previous);
            return Ok(changed);
        }
        let old_scale = self.pixels_per_unit;
        let new_scale =
            (Real::from(old_scale) * factor).clamp(Real::from(f32::MIN_POSITIVE), 2_000.0) as f32;
        let actual_factor = Real::from(new_scale) / Real::from(old_scale);
        let pan_x = (Real::from(viewport_center.x) + Real::from(self.pan.x) - Real::from(center.x))
            * actual_factor;
        let pan_y = (Real::from(viewport_center.y) + Real::from(self.pan.y) - Real::from(center.y))
            * actual_factor;
        let (Some(x), Some(y)) = (real_to_gpu(pan_x), real_to_gpu(pan_y)) else {
            return Err("zoom exceeds the screen-coordinate range");
        };
        let pan = Vec2::new(x, y);
        let changed = pan != self.pan || new_scale != old_scale;
        self.pan = pan;
        self.pixels_per_unit = new_scale;
        self.record_camera_change(previous);
        Ok(changed)
    }

    pub(super) fn zoom_target_window_rect(
        &self,
        target: Point3,
        corner: Pos2,
        rect: Rect,
    ) -> Result<Rect, &'static str> {
        if !rect.is_finite() || !rect.is_positive() || !corner.is_finite() {
            return Err("invalid target window coordinates");
        }
        let center = self
            .project(target, rect)
            .ok_or("target is outside the visible camera range")?;
        let aspect = rect.width() / rect.height();
        let horizontal = (corner.x - center.x).abs();
        let vertical = (corner.y - center.y).abs();
        let half_width = horizontal.max(vertical * aspect);
        let half_height = half_width / aspect;
        if !half_width.is_finite()
            || !half_height.is_finite()
            || (horizontal < 2.0 && vertical < 2.0)
        {
            return Err("target window must extend at least two pixels from its center");
        }
        let window = Rect::from_center_size(center, Vec2::new(2.0 * half_width, 2.0 * half_height));
        if !window.is_finite() {
            return Err("target window exceeds the screen-coordinate range");
        }
        Ok(window)
    }

    pub(crate) fn zoom_target_from_point(
        &mut self,
        target: Point3,
        corner: Point3,
    ) -> Result<bool, &'static str> {
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        let pixel = self
            .project(corner, rect)
            .ok_or("window corner is outside the visible camera range")?;
        self.zoom_target(target, pixel, rect)
    }

    pub(crate) fn zoom_target(
        &mut self,
        target: Point3,
        corner: Pos2,
        rect: Rect,
    ) -> Result<bool, &'static str> {
        let window = self.zoom_target_window_rect(target, corner, rect)?;
        let factor = Real::from(rect.width()) / Real::from(window.width());
        if !factor.is_finite() || factor <= 0.0 {
            return Err("invalid target window scale");
        }
        let new_target = NaVector3::new(target.x(), target.y(), target.z());
        let previous = self.camera_snapshot();
        if self.kind == ViewKind::Perspective {
            let depth = self.view_depth(target);
            if !depth.is_finite() || depth <= self.perspective_near_floor() {
                return Err("target is behind the perspective camera");
            }
            let distance = (depth / factor).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            self.target = new_target;
            self.perspective_camera_distance = distance;
        } else {
            let scale = (Real::from(self.pixels_per_unit) * factor)
                .clamp(Real::from(f32::MIN_POSITIVE), 2_000.0) as f32;
            self.target = new_target;
            self.pan = Vec2::ZERO;
            self.pixels_per_unit = scale;
        }
        let changed = self.camera_snapshot() != previous;
        self.record_camera_change(previous);
        Ok(changed)
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
        let previous = self.camera_snapshot();
        if self.kind == ViewKind::Perspective {
            let old_distance = self.perspective_camera_distance;
            let new_distance = (old_distance / factor).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            if new_distance == old_distance {
                return Ok(false);
            }
            // Dolly along the cursor ray. Translating both camera and orbit
            // target laterally pins the target-depth point under the cursor,
            // while the lens and principal point stay fixed. A projection
            // offset instead produces an increasingly oblique, distorted view.
            let pointer = pointer.unwrap_or(rect.center());
            let focal = self.perspective_focal_length_pixels(rect);
            let (right, up, _) = self.perspective_basis();
            let travel = old_distance - new_distance;
            let principal_point = self.world_origin(rect);
            let horizontal = (Real::from(pointer.x) - Real::from(principal_point.x)) / focal;
            let vertical = (Real::from(principal_point.y) - Real::from(pointer.y)) / focal;
            let target = self.target + (right * horizontal + up * vertical) * travel;
            if !target.iter().all(|value| value.is_finite()) {
                return Err("zoom exceeds the model-coordinate range");
            }
            self.target = target;
            self.perspective_camera_distance = new_distance;
            self.record_camera_change(previous);
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
        self.record_camera_change(previous);
        Ok(true)
    }

    pub(super) fn gpu_view_uniform(
        &self,
        rect: Rect,
        depth_range: Option<(Real, Real)>,
    ) -> GpuViewUniform {
        let width = Real::from(rect.width().max(1.0));
        let height = Real::from(rect.height().max(1.0));
        let pan = if self.kind.is_parallel() {
            self.pan
        } else {
            Vec2::ZERO
        };
        let offset_x = 2.0 * Real::from(pan.x) / width;
        let offset_y = -2.0 * Real::from(pan.y) / height;
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
                    offset_x - self.perspective_lens_shift[0],
                    0.0,
                    0.0,
                    2.0 * focal_length / height,
                    offset_y - self.perspective_lens_shift[1],
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
            ViewKind::Plan => {
                // Plan vertices are already in camera-local coordinates.
                NaMatrix4::new(
                    2.0 / width,
                    0.0,
                    0.0,
                    offset_x,
                    0.0,
                    2.0 / height,
                    0.0,
                    offset_y,
                    0.0,
                    0.0,
                    -1.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                )
            }
            _ => {
                let axes = self.kind.parallel_axes().expect("parallel view");
                let basis = |(axis, sign): (usize, Real)| {
                    NaVector3::from(std::array::from_fn(
                        |index| {
                            if index == axis { sign } else { 0.0 }
                        },
                    ))
                };
                let right = basis(axes.right);
                let up = basis(axes.up);
                let forward = basis(axes.forward);
                // Parallel vertex positions already contain pixels_per_unit.
                let horizontal_scale = 2.0 / width;
                let vertical_scale = 2.0 / height;
                NaMatrix4::new(
                    horizontal_scale * right.x,
                    horizontal_scale * right.y,
                    horizontal_scale * right.z,
                    offset_x,
                    vertical_scale * up.x,
                    vertical_scale * up.y,
                    vertical_scale * up.z,
                    offset_y,
                    forward.x,
                    forward.y,
                    forward.z,
                    0.0,
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
        if self.kind == ViewKind::Plan {
            return self
                .plan_target_frame()
                .and_then(|frame| frame.coordinates_of(point).ok())
                .map_or(Real::NAN, |coordinates| -coordinates[2]);
        }
        match self.kind.parallel_axes() {
            Some(axes) => {
                (point.to_array()[axes.forward.0] - self.target[axes.forward.0]) * axes.forward.1
            }
            None => {
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
    fn fence_anchors_follow_the_model_through_pan_zoom_and_resize() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let pointer = Pos2::new(450.0, 260.0);
        for kind in [
            ViewKind::Top,
            ViewKind::Bottom,
            ViewKind::Front,
            ViewKind::Back,
            ViewKind::Right,
            ViewKind::Left,
            ViewKind::Plan,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport::new(kind);
            view.last_rect = Some(rect);
            let anchor = view.fence_anchor(pointer).unwrap();
            assert!(view.project(anchor, rect).unwrap().distance(pointer) < 0.001);
            view.apply_navigation_drag(
                PointerButton::Middle,
                egui::Modifiers::NONE,
                Vec2::new(35.0, -20.0),
            );
            let after_pan = view.project_fence(&[anchor]).unwrap()[0];
            assert!(after_pan.distance(pointer) > 1.0, "{kind:?}");
            view.zoom_by(1.3, Some(rect.center()), rect);
            let after_zoom = view.project_fence(&[anchor]).unwrap()[0];
            assert!(after_zoom.distance(after_pan) > 1.0, "{kind:?}");
            let resized = Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 650.0));
            view.last_rect = Some(resized);
            let after_resize = view.project_fence(&[anchor]).unwrap()[0];
            assert!(after_resize.distance(after_zoom) > 1.0, "{kind:?}");
            let recovered = view.fence_anchor(after_resize).unwrap();
            assert!(
                anchor
                    .to_array()
                    .into_iter()
                    .zip(recovered.to_array())
                    .all(|(a, b)| (a - b).abs() < 1e-5),
                "{kind:?}: anchor={anchor:?}, recovered={recovered:?}"
            );
        }
    }

    #[test]
    fn zoom_window_centers_the_chosen_region_in_each_view() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let window = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 300.0));
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport::new(kind);
            let old_scale = view.pixels_per_unit;
            let old_distance = view.perspective_camera_distance;
            let point = if kind == ViewKind::Perspective {
                let (right, up, _) = view.perspective_basis();
                let focal = view.perspective_focal_length_pixels(rect);
                let target = view.target
                    + right
                        * (Real::from(window.center().x - rect.center().x) / focal * old_distance)
                    + up * (Real::from(rect.center().y - window.center().y) / focal * old_distance);
                Point3::try_new(target.x, target.y, target.z).unwrap()
            } else {
                view.unproject(window.center(), rect, 0.0).unwrap()
            };
            let before = view.project(point, rect).unwrap();
            assert!((before - window.center()).length() < 0.01);
            assert_eq!(view.zoom_window(window, rect), Ok(true));
            assert!((view.project(point, rect).unwrap() - rect.center()).length() < 0.01);
            if kind == ViewKind::Perspective {
                assert!((view.perspective_camera_distance - old_distance / 3.0).abs() < 1e-10);
            } else {
                assert!((view.pixels_per_unit - old_scale * 3.0).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn invalid_zoom_window_leaves_camera_unchanged() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut view = Viewport::new(ViewKind::Perspective);
        let before = (view.target, view.perspective_camera_distance);
        for window in [
            Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(101.0, 200.0)),
            Rect::from_min_max(Pos2::new(-10.0, 100.0), Pos2::new(300.0, 300.0)),
        ] {
            assert!(view.zoom_window(window, rect).is_err());
            assert_eq!((view.target, view.perspective_camera_distance), before);
        }
        let mut parallel = Viewport::new(ViewKind::Top);
        parallel.pan.x = f32::MAX;
        let before = (parallel.pan, parallel.pixels_per_unit);
        let window = Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 300.0));
        assert!(parallel.zoom_window(window, rect).is_err());
        assert_eq!((parallel.pan, parallel.pixels_per_unit), before);
    }

    #[test]
    fn camera_history_branches_after_undo_and_skips_failed_zooms() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut view = Viewport::new(ViewKind::Top);
        view.last_rect = Some(rect);
        let original = view.camera_snapshot();
        assert_eq!(view.zoom_factor(2.0), Ok(true));
        let first = view.camera_snapshot();
        assert_eq!(view.zoom_factor(3.0), Ok(true));
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), first);
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), original);
        assert!(!view.undo_view());
        assert!(view.redo_view());
        assert_eq!(view.camera_snapshot(), first);
        assert_eq!(view.zoom_factor(1.0), Ok(false));
        assert!(view.redo_view());
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), first);
        assert!(
            view.zoom_window(
                Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 300.0)),
                rect,
            )
            .unwrap()
        );
        assert!(!view.redo_view());
        let changed = view.camera_snapshot();
        assert!(view.zoom_factor(-1.0).is_err());
        assert_eq!(view.camera_snapshot(), changed);
        assert!(view.undo_view());
        assert_eq!(view.camera_snapshot(), first);
    }

    #[test]
    fn zoom_target_centers_the_picked_world_point_and_records_one_view_step() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let target = Point3::try_new(1.0, 2.0, 3.0).unwrap();
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport::new(kind);
            let before = view.camera_snapshot();
            let original_depth = view.view_depth(target);
            let center = view.project(target, rect).unwrap();
            let corner = center + Vec2::new(100.0, 75.0);
            assert_eq!(view.zoom_target(target, corner, rect), Ok(true));
            assert!((view.project(target, rect).unwrap() - rect.center()).length() < 0.01);
            assert_eq!(view.target, NaVector3::new(1.0, 2.0, 3.0));
            if kind == ViewKind::Perspective {
                assert!((view.perspective_camera_distance - original_depth / 4.0).abs() < 1e-5);
            } else {
                assert!((view.pixels_per_unit - 160.0).abs() < 1e-5);
                assert_eq!(view.pan, Vec2::ZERO);
            }
            let after = view.camera_snapshot();
            assert!(view.undo_view());
            assert_eq!(view.camera_snapshot(), before);
            assert!(view.redo_view());
            assert_eq!(view.camera_snapshot(), after);
        }
    }

    #[test]
    fn zoom_target_rejects_tiny_windows_without_changing_camera() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut view = Viewport::new(ViewKind::Perspective);
        let target = Point3::try_new(0.0, 0.0, 0.0).unwrap();
        let before = view.camera_snapshot();
        assert!(view.zoom_target(target, rect.center(), rect).is_err());
        assert_eq!(view.camera_snapshot(), before);
        assert!(!view.undo_view());
        let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 800.0));
        let mut parallel = Viewport::new(ViewKind::Top);
        assert_eq!(
            parallel.zoom_target(target, tall.center() + Vec2::new(0.0, 3.0), tall),
            Ok(true)
        );
    }

    #[test]
    fn parallel_depth_encoding_handles_singletons_subnormals_and_overflowing_spans() {
        for kind in [ViewKind::Top, ViewKind::Front, ViewKind::Right] {
            let viewport = Viewport::new(kind);
            let (axis, sign) = viewport.parallel_depth_axis().unwrap();
            for (minimum, maximum) in [
                (1e24, 1e24),
                (0.0, Real::from_bits(1)),
                (-Real::MAX, Real::MAX),
                (2.0_f64.powi(80), 2.0_f64.powi(80) + 2.0_f64.powi(40)),
            ] {
                for (depth, expected) in [(minimum, 0.05), (maximum, 0.95)] {
                    let mut position = [7.0, 8.0, 9.0, 10.0];
                    viewport.encode_gpu_depth(&mut position, depth, Some((minimum, maximum)));
                    let expected = if minimum == maximum { 0.5 } else { expected };
                    assert_eq!(position[axis], sign * expected);
                    for other in 0..4 {
                        if other != axis {
                            assert_eq!(position[other], [7.0, 8.0, 9.0, 10.0][other]);
                        }
                    }
                }
            }
        }
        let viewport = Viewport::new(ViewKind::Perspective);
        let mut position = [1.0, 2.0, 3.0];
        viewport.encode_gpu_depth(&mut position, 1e24, Some((0.0, 2e24)));
        assert_eq!(position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn zoom_in_out_take_inverse_view_center_steps() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let mut view = Viewport {
                last_rect: Some(rect),
                ..Viewport::new(kind)
            };
            let before = view.pixels_per_model_unit_at_origin(rect);
            let target = view.target;
            assert_eq!(view.zoom_in(0.9), Ok(true));
            let after = view.pixels_per_model_unit_at_origin(rect);
            assert!((f64::from(after / before) - 1.0 / 0.9).abs() < 1e-6);
            assert_eq!(view.target, target);
            assert_eq!(view.zoom_out(0.9), Ok(true));
            assert!(
                (f64::from(view.pixels_per_model_unit_at_origin(rect) / before) - 1.0).abs() < 1e-6
            );
            assert_eq!(view.target, target);
            assert_eq!(view.zoom_in(1.25), Ok(true));
            assert!(
                (f64::from(view.pixels_per_model_unit_at_origin(rect) / before) - 0.8).abs() < 1e-6
            );
            assert_eq!(view.zoom_out(1.25), Ok(true));
            assert!(
                (f64::from(view.pixels_per_model_unit_at_origin(rect) / before) - 1.0).abs() < 1e-6
            );
            for invalid in [0.0, f64::NAN, f64::INFINITY, f64::from_bits(1)] {
                assert!(view.zoom_in(invalid).is_err());
                assert!(view.zoom_out(invalid).is_err());
            }
        }
    }

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
            assert_eq!(
                view.pan,
                if kind.is_parallel() {
                    before.0 * 2.0
                } else {
                    before.0
                }
            );
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
            if kind.is_parallel() {
                assert!(view.zoom_factor(2.0).is_err());
            }
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
        for kind in [
            ViewKind::Top,
            ViewKind::Bottom,
            ViewKind::Front,
            ViewKind::Back,
            ViewKind::Right,
            ViewKind::Left,
        ] {
            let view = Viewport {
                pan: Vec2::new(f32::MAX, -f32::MAX),
                ..Viewport::new(kind)
            };
            let actual = view.unproject(pointer, rect, 0.0).unwrap();
            let expected = match kind {
                ViewKind::Top => [horizontal, horizontal, 0.0],
                ViewKind::Bottom => [horizontal, -horizontal, 0.0],
                ViewKind::Front => [horizontal, 0.0, horizontal],
                ViewKind::Back => [-horizontal, 0.0, horizontal],
                ViewKind::Right => [0.0, horizontal, horizontal],
                ViewKind::Left => [0.0, -horizontal, horizontal],
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
        let expected_y = -Real::from(f32::MAX) / view.perspective_focal_length_pixels(rect)
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

#[cfg(test)]
mod navigation_tests {
    use super::*;

    #[test]
    fn repeated_off_center_zoom_is_a_translation_along_a_fixed_cursor_ray() {
        let rect = Rect::from_min_size(Pos2::new(19., 37.), Vec2::new(900., 600.));
        let cursor = rect.min + Vec2::new(760., 100.);
        for offset in [0., 1e9] {
            let mut view = Viewport::new(ViewKind::Perspective);
            view.target = NaVector3::new(offset, -offset, 2. * offset);
            view.perspective_camera_distance = 1e5;
            let initial_target = view.target;
            let initial_distance = view.perspective_camera_distance;
            let focal = view.perspective_focal_length_pixels(rect);
            let (_, ray) = view.perspective_local_ray(cursor, rect);
            let (_, _, forward) = view.perspective_basis();
            let anchor = view.target - forward * initial_distance + ray * initial_distance;
            let anchor = Point3::try_new(anchor.x, anchor.y, anchor.z).unwrap();
            for _ in 0..80 {
                let old_camera = view.target - forward * view.perspective_camera_distance;
                view.zoom_by(1.1, Some(cursor), rect);
                let camera = view.target - forward * view.perspective_camera_distance;
                assert!(
                    (camera - old_camera)
                        .normalize()
                        .cross(&ray.normalize())
                        .norm()
                        < 1e-6
                );
                assert_eq!(view.world_origin(rect), rect.center());
                assert_eq!(view.perspective_focal_length_pixels(rect), focal);
                assert_eq!(view.pan, Vec2::ZERO);
                assert!(view.project(anchor, rect).unwrap().distance(cursor) < 0.02);
                let (_, next_ray) = view.perspective_local_ray(cursor, rect);
                assert_eq!(ray, next_ray);
                let center_point = view.target + forward * 10.;
                assert!(
                    view.project(
                        Point3::try_new(center_point.x, center_point.y, center_point.z).unwrap(),
                        rect
                    )
                    .unwrap()
                    .distance(rect.center())
                        < 0.001
                );
            }
            for _ in 0..80 {
                view.zoom_by_factor(1. / Real::from(1.1_f32), Some(cursor), rect)
                    .unwrap();
            }
            assert!((view.target - initial_target).norm() < 1e-4);
            assert!((view.perspective_camera_distance - initial_distance).abs() < 1e-6);
        }
    }

    #[test]
    fn perspective_pan_translates_camera_and_preserves_centered_projection() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let mut view = Viewport::new(ViewKind::Perspective);
        view.last_rect = Some(rect);
        let (right, _, forward) = view.perspective_basis();
        let distance = view.perspective_camera_distance;
        let near = Point3::try_new(0., 0., 0.).unwrap();
        let far = forward * distance;
        let far = Point3::try_new(far.x, far.y, far.z).unwrap();
        view.apply_navigation_drag(
            PointerButton::Middle,
            egui::Modifiers::NONE,
            Vec2::new(200., 0.),
        );
        let scale = distance / view.perspective_focal_length_pixels(rect);
        assert!((view.target + right * (200. * scale)).norm() < 1e-12);
        assert_eq!(view.world_origin(rect), rect.center());
        assert_eq!(view.pan, Vec2::ZERO);
        assert!(
            view.project(near, rect)
                .unwrap()
                .distance(rect.center() + Vec2::new(200., 0.))
                < 0.001
        );
        assert!(
            view.project(far, rect)
                .unwrap()
                .distance(rect.center() + Vec2::new(100., 0.))
                < 0.001
        );
    }
}
