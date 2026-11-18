//! Reuse staged vertices while the original f32 origin still resolves screen detail.

use super::*;

pub(super) struct Reprojection {
    camera: CameraSnapshot,
    min: NaVector3<Real>,
    max: NaVector3<Real>,
    depths: (Real, Real),
    forward: NaVector3<Real>,
    coordinate_scale: Real,
}

impl Reprojection {
    pub(super) fn new(
        view: &Viewport,
        objects: &[DisplayObject],
        depths: Option<(Real, Real)>,
    ) -> Option<Self> {
        let depths = depths?;
        let mut bounds: Option<viboceros_geometry::BoundingBox3> = None;
        for object in objects {
            if matches!(&*object.geometry.geometry, Geometry::EmptyMesh) {
                continue;
            }
            if object.transform.is_some()
                || object.overlay
                || object.draft.is_some()
                || object.curvature.is_some()
                || object.face_packs.is_some()
                || matches!(&*object.geometry.geometry, Geometry::PointCloud(_))
            {
                return None;
            }
            let current = object.geometry.bounds()?;
            bounds = Some(match bounds {
                Some(bounds) => bounds.union(current).ok()?,
                None => current,
            });
        }
        let bounds = bounds?;
        let lo = NaVector3::from(bounds.min().to_array());
        let hi = NaVector3::from(bounds.max().to_array());
        Some(Self {
            camera: view.camera_snapshot(),
            min: lo - view.target,
            max: hi - view.target,
            depths,
            forward: view.screen_axes().2,
            coordinate_scale: lo.amax().max(hi.amax()),
        })
    }

    pub(super) fn uniform(&self, view: &Viewport, rect: Rect) -> Option<GpuViewUniform> {
        if view.kind != self.camera.kind {
            return None;
        }
        // This limits the absolute f32 roundoff in viewport coordinates to a
        // small fraction of a point. Rebase geometry at deeper zoom levels.
        const POINT_LIMIT: Real = 65_536.0;
        let extent = self
            .min
            .map(Real::abs)
            .zip_map(&self.max.map(Real::abs), Real::max);
        if view.kind.is_parallel() {
            if view.target != self.camera.target
                || (view.kind == ViewKind::Plan && view.plan_frame != self.camera.plan_frame)
            {
                return None;
            }
            let (right, up, _) = view.screen_axes();
            let bound = extent.dot(&right.abs()).max(extent.dot(&up.abs()));
            let zoom = view.pixels_per_unit / self.camera.pixels_per_unit;
            if !(bound * view.pixels_per_unit + Real::from(view.pan.abs().max_elem())
                <= POINT_LIMIT
                && (f32::MIN_POSITIVE.into()..=f32::MAX.into()).contains(&zoom))
            {
                return None;
            }
            return Some(view.gpu_view_uniform_rebased(
                rect,
                Some(self.depths),
                NaVector3::zeros(),
                zoom,
            ));
        }

        // Ghosted surfaces are sorted back to front. A new camera angle
        // needs new ordering even when all vertices remain visible.
        if view.display_mode == DisplayMode::Ghosted {
            return None;
        }
        let delta = self.camera.target - view.target;
        let (right, up, forward) = view.perspective_basis();
        let depth_origin = view.perspective_camera_distance + forward.dot(&delta);
        let mut bound_depths = (depth_origin, depth_origin);
        for axis in 0..3 {
            let a = forward[axis] * self.min[axis];
            let b = forward[axis] * self.max[axis];
            bound_depths.0 += a.min(b);
            bound_depths.1 += a.max(b);
        }
        let near_guard = view.perspective_near_floor()
            + 64.0
                * Real::EPSILON
                * self
                    .coordinate_scale
                    .max(view.target.amax())
                    .max(view.perspective_camera_distance);
        if !(bound_depths.0 > near_guard && bound_depths.1.is_finite()) {
            return None;
        }
        let shift = |axis: NaVector3<Real>| extent.dot(&axis.abs()) + axis.dot(&delta).abs();
        let screen_bound = shift(right).max(shift(up)).max(
            view.perspective_camera_distance.abs()
                + extent.dot(&forward.abs())
                + forward.dot(&delta).abs(),
        ) * view.perspective_focal_length_pixels(rect)
            / bound_depths.0;
        if screen_bound > POINT_LIMIT || !screen_bound.is_finite() {
            return None;
        }
        let depth_range = if forward == self.forward
            && delta == NaVector3::zeros()
            && view.perspective_camera_distance == self.camera.perspective_camera_distance
        {
            self.depths
        } else {
            bound_depths
        };
        Some(view.gpu_view_uniform_rebased(rect, Some(depth_range), delta, 1.0))
    }
}
