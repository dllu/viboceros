//! Camera fitting, separate from rendering and interaction routing.

use super::*;
use viboceros_geometry::BoundingBox3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ZoomExtentsBorders {
    pub parallel: Real,
    pub perspective: Real,
}

impl Default for ZoomExtentsBorders {
    fn default() -> Self {
        Self {
            parallel: 1.1,
            perspective: 1.0,
        }
    }
}

impl ZoomExtentsBorders {
    pub(crate) fn valid(self) -> bool {
        [self.parallel, self.perspective]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0 && value.recip().is_finite())
    }

    fn for_kind(self, kind: ViewKind) -> Real {
        if kind.is_parallel() {
            self.parallel
        } else {
            self.perspective
        }
    }
}

struct CameraFit {
    target: NaVector3<Real>,
    scale: f32,
    distance: Real,
}

impl CameraFit {
    fn apply(self, viewport: &mut Viewport) {
        let previous = viewport.camera_snapshot();
        viewport.target = self.target;
        viewport.pan = Vec2::ZERO;
        if viewport.kind == ViewKind::Perspective {
            viewport.perspective_camera_distance = self.distance;
        } else {
            viewport.pixels_per_unit = self.scale;
        }
        viewport.record_camera_change(previous);
    }
}

impl Viewport {
    pub(crate) fn zoom_all(
        viewports: &mut [Self],
        document: &Document,
        selected_only: bool,
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        if !borders.valid() {
            return Err("invalid zoom extents border scale");
        }
        let Some(bounds) = Self::zoom_bounds(document, selected_only) else {
            return Ok(false);
        };
        let fits = viewports
            .iter()
            .map(|viewport| {
                let rect = viewport.last_rect.ok_or("viewport has not been laid out")?;
                viewport.fit_bounds(bounds, rect, borders.for_kind(viewport.kind))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (viewport, fit) in viewports.iter_mut().zip(fits) {
            fit.apply(viewport);
        }
        Ok(!viewports.is_empty())
    }

    pub(crate) fn zoom_extents(
        &mut self,
        document: &Document,
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        self.zoom_objects(document, false, borders)
    }

    pub(crate) fn zoom_selected(
        &mut self,
        document: &Document,
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        self.zoom_objects(document, true, borders)
    }

    fn zoom_objects(
        &mut self,
        document: &Document,
        selected_only: bool,
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        if !borders.valid() {
            return Err("invalid zoom extents border scale");
        }
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        let Some(bounds) = Self::zoom_bounds(document, selected_only) else {
            return Ok(false);
        };
        self.fit_bounds(bounds, rect, borders.for_kind(self.kind))?
            .apply(self);
        Ok(true)
    }

    fn zoom_bounds(document: &Document, selected_only: bool) -> Option<BoundingBox3> {
        document
            .objects()
            .filter(|object| {
                (!selected_only || document.is_selected(object.id()))
                    && object.attributes().is_visible()
                    && document
                        .layer(object.attributes().layer_id())
                        .is_some_and(|layer| layer.is_visible())
            })
            .map(|object| object.geometry().bounds())
            .reduce(|a, b| a.union(b).expect("finite bounds"))
    }

    fn fit_bounds(
        &self,
        bounds: BoundingBox3,
        rect: Rect,
        border: Real,
    ) -> Result<CameraFit, &'static str> {
        if !rect.is_finite()
            || !rect.is_positive()
            || !rect.width().is_finite()
            || !rect.height().is_finite()
        {
            return Err("invalid viewport dimensions");
        }
        if !border.is_finite() || border <= 0.0 || !border.recip().is_finite() {
            return Err("invalid zoom extents border scale");
        }
        let available_half = 0.5 / border;
        let center = bounds.center().map_err(|_| "invalid model bounds")?;
        let target = NaVector3::from(center.to_array());
        let plan_frame = self.plan_frame.with_origin(center);
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
            let local = corner - target;
            let (x, y) = match self.kind {
                ViewKind::Top => (local.x, local.y),
                ViewKind::Bottom => (local.x, -local.y),
                ViewKind::Front => (local.x, local.z),
                ViewKind::Back => (-local.x, local.z),
                ViewKind::Right => (local.y, local.z),
                ViewKind::Left => (-local.y, local.z),
                ViewKind::Plan => {
                    let coordinates = plan_frame
                        .projected_coordinates_of(
                            Point3::try_new(corner.x, corner.y, corner.z)
                                .map_err(|_| "invalid model bounds")?,
                        )
                        .map_err(|_| "model extents exceed the supported camera range")?;
                    (coordinates[0], coordinates[1])
                }
                ViewKind::Perspective => {
                    let x = local.dot(&right);
                    let y = local.dot(&up);
                    let z = local.dot(&forward);
                    distance = distance.max(
                        (x.abs() * focal / (Real::from(rect.width()) * available_half))
                            .max(y.abs() * focal / (Real::from(rect.height()) * available_half))
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
        let scale = (Real::from(rect.width()) * available_half / horizontal)
            .min(Real::from(rect.height()) * available_half / vertical);
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
        staged.plan_frame = self.plan_frame;
        staged.perspective_frame = self.perspective_frame;
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
            if staged.gpu_position(corner).is_none() {
                return Err("fitted model bounds exceed the GPU coordinate range");
            }
            if !staged
                .project(corner, rect)
                .is_some_and(|point| border < 1.0 || rect.expand(1.0).contains(point))
            {
                return Err("model extents cannot be represented by this camera");
            }
        }
        Ok(CameraFit {
            target,
            scale,
            distance,
        })
    }
}
