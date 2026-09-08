//! Camera fitting, separate from rendering and interaction routing.

use super::*;
use viboceros_geometry::BoundingBox3;

impl Viewport {
    pub(crate) fn zoom_extents(&mut self, document: &Document) -> Result<bool, &'static str> {
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        let bounds = document
            .objects()
            .filter(|object| {
                object.attributes().is_visible()
                    && document
                        .layer(object.attributes().layer_id())
                        .is_some_and(|layer| layer.is_visible())
            })
            .map(|object| object.geometry().bounds())
            .reduce(|a, b| a.union(b).expect("finite bounds"));
        let Some(bounds) = bounds else {
            return Ok(false);
        };
        self.fit_bounds(bounds, rect)?;
        Ok(true)
    }

    fn fit_bounds(&mut self, bounds: BoundingBox3, rect: Rect) -> Result<(), &'static str> {
        if !rect.is_finite()
            || !rect.is_positive()
            || !rect.width().is_finite()
            || !rect.height().is_finite()
        {
            return Err("invalid viewport dimensions");
        }
        let center = bounds.center().map_err(|_| "invalid model bounds")?;
        let target = NaVector3::from(center.to_array());
        let minimum = bounds.min().to_array();
        let maximum = bounds.max().to_array();
        let mut horizontal = 0.0_f64;
        let mut vertical = 0.0_f64;
        let mut distance = MIN_PERSPECTIVE_CAMERA_DISTANCE;
        let (right, up, forward) = self.perspective_basis();
        let focal = self.perspective_focal_length_pixels(rect);
        for index in 0..8 {
            let corner = NaVector3::from(std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }));
            if corner
                .iter()
                .any(|value| value.abs() > Real::from(f32::MAX))
            {
                return Err("model bounds exceed the GPU coordinate range");
            }
            let local = corner - target;
            let (x, y) = match self.kind {
                ViewKind::Top => (local.x, local.y),
                ViewKind::Front => (local.x, local.z),
                ViewKind::Right => (local.y, local.z),
                ViewKind::Perspective => {
                    let x = local.dot(&right);
                    let y = local.dot(&up);
                    let z = local.dot(&forward);
                    distance = distance.max(
                        (x.abs() * focal / (Real::from(rect.width()) * 0.45))
                            .max(y.abs() * focal / (Real::from(rect.height()) * 0.45))
                            - z,
                    );
                    distance = distance.max(-z + MIN_PERSPECTIVE_CAMERA_DISTANCE);
                    (x, y)
                }
            };
            horizontal = horizontal.max(x.abs());
            vertical = vertical.max(y.abs());
        }
        if bounds.min() == bounds.max() {
            distance = DEFAULT_PERSPECTIVE_CAMERA_DISTANCE;
        }
        let scale = (Real::from(rect.width()) * 0.45 / horizontal)
            .min(Real::from(rect.height()) * 0.45 / vertical);
        let scale = if scale.is_infinite() {
            40.0
        } else {
            scale.min(2_000.0) as f32
        };
        if !scale.is_finite()
            || scale < f32::MIN_POSITIVE
            || !distance.is_finite()
            || (self.kind == ViewKind::Perspective && distance > MAX_PERSPECTIVE_CAMERA_DISTANCE)
        {
            return Err("model extents exceed the supported camera range");
        }
        let mut staged = Viewport::new(self.kind);
        staged.target = target;
        staged.orbit_yaw = self.orbit_yaw;
        staged.orbit_pitch = self.orbit_pitch;
        staged.pixels_per_unit = scale;
        staged.perspective_camera_distance = distance;
        if staged
            .gpu_view_uniform(rect, None)
            .view_projection
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err("fitted camera exceeds the GPU matrix range");
        }
        for index in 0..8 {
            let corner = Point3::try_from(std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }))
            .expect("finite bounds");
            if !staged
                .project(corner, rect)
                .is_some_and(|point| rect.contains(point))
            {
                return Err("model extents cannot be represented by this camera");
            }
        }
        self.target = target;
        self.pan = Vec2::ZERO;
        if self.kind == ViewKind::Perspective {
            self.perspective_camera_distance = distance;
        } else {
            self.pixels_per_unit = scale;
        }
        Ok(())
    }
}
