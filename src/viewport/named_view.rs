//! Translation between the app camera and OpenNURBS named viewport records.

use super::*;
use viboceros_io::{ThreeDmNamedView, ThreeDmProjection};

fn point(position: NaVector3<Real>) -> Result<Point3, GeometryError> {
    Point3::try_from([position.x, position.y, position.z])
}

fn vector(direction: NaVector3<Real>) -> Result<Vector3, GeometryError> {
    Vector3::try_from([direction.x, direction.y, direction.z])
}

impl Viewport {
    pub(super) fn named_view_port_size(&self) -> [i32; 2] {
        self.last_rect.map_or([800, 600], |rect| {
            [rect.width().max(1.0) as i32, rect.height().max(1.0) as i32]
        })
    }

    pub(crate) fn named_view_to_3dm(
        saved: NamedViewSnapshot,
        name: String,
    ) -> Result<ThreeDmNamedView, GeometryError> {
        let camera = saved.camera;
        let [width, height] = saved.port_size;
        let aspect = f64::from(width) / f64::from(height);
        let (projection, location, direction, up, target, frustum) =
            if camera.kind == ViewKind::Perspective {
                let mut view = Self::new(ViewKind::Perspective);
                view.restore_camera(camera);
                let (_, up, forward) = view.perspective_basis();
                let location = camera.target - forward * camera.perspective_camera_distance;
                let half_height = (camera.perspective_fov_radians * 0.5).tan();
                let half_width = half_height * aspect;
                let [shift_x, shift_y] = camera.perspective_lens_shift;
                (
                    ThreeDmProjection::Perspective,
                    location,
                    forward,
                    up,
                    camera.target,
                    [
                        (shift_x - 1.0) * half_width,
                        (shift_x + 1.0) * half_width,
                        (shift_y - 1.0) * half_height,
                        (shift_y + 1.0) * half_height,
                        1.0,
                        1.0e9,
                    ],
                )
            } else {
                let frame = if camera.kind == ViewKind::Plan {
                    camera.plan_frame
                } else {
                    Self::default_plane(camera.kind)
                };
                let right = NaVector3::from(frame.x_axis().as_vector().to_array());
                let up = NaVector3::from(frame.y_axis().as_vector().to_array());
                let forward = -NaVector3::from(frame.z_axis().as_vector().to_array());
                let scale = f64::from(camera.pixels_per_unit);
                let center = camera.target - right * (f64::from(camera.pan.x) / scale)
                    + up * (f64::from(camera.pan.y) / scale);
                let half_width = f64::from(width) / (2.0 * scale);
                let half_height = f64::from(height) / (2.0 * scale);
                (
                    ThreeDmProjection::Parallel,
                    center - forward * 50.0,
                    forward,
                    up,
                    center,
                    [
                        -half_width,
                        half_width,
                        -half_height,
                        half_height,
                        1.0,
                        1.0e9,
                    ],
                )
            };
        Ok(ThreeDmNamedView {
            name,
            projection,
            camera_location: point(location)?,
            camera_direction: vector(direction)?,
            camera_up: vector(up)?,
            target: Some(point(target)?),
            construction_plane: saved.plane,
            frustum,
            screen_port: [0, width, height, 0],
        })
    }

    pub(crate) fn named_view_from_3dm(
        source: &ThreeDmNamedView,
    ) -> Result<NamedViewSnapshot, GeometryError> {
        let [left, right, bottom, top, near, _] = source.frustum;
        let width = right - left;
        let height = top - bottom;
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || (source.projection == ThreeDmProjection::Perspective && near <= 0.0)
        {
            return Err(GeometryError::Degenerate {
                context: "named view frustum",
            });
        }
        let port_extent = |a: i32, b: i32| {
            (i64::from(a) - i64::from(b))
                .abs()
                .clamp(1, i64::from(i32::MAX)) as i32
        };
        let port_size = [
            port_extent(source.screen_port[1], source.screen_port[0]),
            port_extent(source.screen_port[2], source.screen_port[3]),
        ];
        let forward = NaVector3::from(source.camera_direction.to_array()).normalize();
        let up_hint = NaVector3::from(source.camera_up.to_array());
        let right_axis = forward.cross(&up_hint).normalize();
        let up_axis = right_axis.cross(&forward).normalize();
        let location = NaVector3::from(source.camera_location.to_array());
        let nominal_target = source.target.map_or(location + forward * 50.0, |target| {
            NaVector3::from(target.to_array())
        });
        let distance = (nominal_target - location).dot(&forward);
        let distance = if distance.is_finite() && distance > 0.01 {
            distance
        } else {
            50.0
        };
        let mut view = Self::new(ViewKind::Plan);
        view.plane.set(source.construction_plane);
        view.cplane_direction = None;
        view.pan = Vec2::ZERO;
        if source.projection == ThreeDmProjection::Perspective {
            view.kind = ViewKind::Perspective;
            // The app's orbit target lies on the camera axis. Rhino targets may
            // be off axis, so preserve the camera location when choosing it.
            view.target = location + forward * distance;
            view.perspective_frame = Some(Frame3::try_from_directions(
                point(view.target)?,
                vector(right_axis)?,
                vector(up_axis)?,
                Tolerance::DEFAULT,
            )?);
            view.perspective_camera_distance = distance;
            view.perspective_fov_radians = 2.0 * (height / (2.0 * near)).atan();
            view.perspective_lens_shift = [(left + right) / width, (bottom + top) / height];
        } else {
            view.kind = ViewKind::Plan;
            view.target = nominal_target
                + right_axis * ((left + right) * 0.5)
                + up_axis * ((bottom + top) * 0.5);
            view.plan_frame = Frame3::try_from_directions(
                point(view.target)?,
                vector(right_axis)?,
                vector(up_axis)?,
                Tolerance::DEFAULT,
            )?;
            view.pixels_per_unit = (f64::from(port_size[1]) / height) as f32;
            if !view.pixels_per_unit.is_finite() || view.pixels_per_unit <= 0.0 {
                return Err(GeometryError::Degenerate {
                    context: "named view scale",
                });
            }
        }
        let mut snapshot = view.named_view_snapshot();
        snapshot.port_size = port_size;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_view_preserves_center_scale_and_construction_plane() {
        let mut view = Viewport::new(ViewKind::Front);
        view.pan = Vec2::new(67.0, -33.0);
        view.pixels_per_unit = 17.5;
        view.target = NaVector3::new(3.0, 5.0, 7.0);
        view.last_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0)));
        view.plane.set(WorldPlane::Left.frame());
        let encoded =
            Viewport::named_view_to_3dm(view.named_view_snapshot(), "Front".into()).unwrap();
        let decoded = Viewport::named_view_from_3dm(&encoded).unwrap();
        let again = Viewport::named_view_to_3dm(decoded, "Front".into()).unwrap();
        assert_eq!(again.projection, ThreeDmProjection::Parallel);
        assert_eq!(again.screen_port, encoded.screen_port);
        assert_eq!(again.construction_plane, encoded.construction_plane);
        for (a, b) in again
            .camera_location
            .to_array()
            .into_iter()
            .zip(encoded.camera_location.to_array())
        {
            assert!((a - b).abs() < 1.0e-7);
        }
        for (a, b) in again.frustum.into_iter().zip(encoded.frustum) {
            assert!((a - b).abs() < 1.0e-7);
        }
    }

    #[test]
    fn perspective_view_preserves_off_axis_lens_and_fov() {
        let mut view = Viewport::new(ViewKind::Perspective);
        view.perspective_fov_radians = 0.9;
        view.perspective_lens_shift = [0.2, -0.15];
        view.target = NaVector3::new(11.0, -4.0, 2.0);
        view.last_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0)));
        let encoded =
            Viewport::named_view_to_3dm(view.named_view_snapshot(), "Lens".into()).unwrap();
        let decoded = Viewport::named_view_from_3dm(&encoded).unwrap();
        let again = Viewport::named_view_to_3dm(decoded, "Lens".into()).unwrap();
        assert_eq!(again.projection, ThreeDmProjection::Perspective);
        for (a, b) in again.frustum.into_iter().zip(encoded.frustum) {
            assert!((a - b).abs() < 1.0e-10);
        }
        for (a, b) in again
            .camera_location
            .to_array()
            .into_iter()
            .zip(encoded.camera_location.to_array())
        {
            assert!((a - b).abs() < 1.0e-8);
        }
        let mut restored = Viewport::new(ViewKind::Top);
        restored.restore_named_view(decoded);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0));
        let target = point(restored.target).unwrap();
        let projected = restored.project(target, rect).unwrap();
        assert!((projected.x - 360.0).abs() < 0.01);
        assert!((projected.y - 255.0).abs() < 0.01);
        let gpu = restored.gpu_view_uniform(rect, None);
        let target_w = gpu.view_projection[3][3];
        assert!((gpu.view_projection[3][0] / target_w + 0.2).abs() < 1.0e-6);
        assert!((gpu.view_projection[3][1] / target_w - 0.15).abs() < 1.0e-6);
    }
}
