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
    camera: CameraSnapshot,
}

impl CameraFit {
    fn with_document_clipping(
        self,
        viewport: &Viewport,
        document: &Document,
        rect: Rect,
    ) -> Result<Self, &'static str> {
        let mut staged = Viewport::new(viewport.kind);
        staged.restore_camera(self.camera);
        staged.update_clipping_from_document(document, rect)?;
        Ok(Self {
            camera: staged.camera_snapshot(),
        })
    }

    fn apply(self, viewport: &mut Viewport) {
        let previous = viewport.camera_snapshot();
        viewport.restore_camera(self.camera);
        viewport.record_camera_change(previous);
    }
}

impl Viewport {
    #[cfg(test)]
    pub(super) fn zoom_bounding_box(&mut self, bounds: BoundingBox3) -> Result<(), &'static str> {
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        self.fit_bounds(bounds, rect, 1.0)?.apply(self);
        Ok(())
    }

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
                viewport
                    .fit_bounds(bounds, rect, borders.for_kind(viewport.kind))?
                    .with_document_clipping(viewport, document, rect)
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

    pub(crate) fn zoom_curve_ends(
        &mut self,
        document: &Document,
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        let markers = collect_end_markers(
            document,
            document.selected_object_ids(),
            EndMarkerOptions::default(),
        )?;
        self.zoom_end_markers(&markers, borders)
    }

    pub(crate) fn zoom_end_markers(
        &mut self,
        markers: &[EndMarker],
        borders: ZoomExtentsBorders,
    ) -> Result<bool, &'static str> {
        if !borders.valid() {
            return Err("invalid zoom extents border scale");
        }
        let rect = self.last_rect.ok_or("viewport has not been laid out")?;
        let mut bounds: Option<BoundingBox3> = None;
        for point in markers.iter().map(|marker| marker.point) {
            let marker = BoundingBox3::from_points([point]).map_err(|_| "invalid curve end")?;
            bounds = Some(match bounds {
                Some(previous) => previous.union(marker).map_err(|_| "invalid curve end")?,
                None => marker,
            });
        }
        let Some(bounds) = bounds else {
            return Ok(false);
        };
        self.fit_bounds(bounds, rect, borders.for_kind(self.kind))?
            .apply(self);
        Ok(true)
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
            .with_document_clipping(self, document, rect)?
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
        if self.kind == ViewKind::Perspective
            && (!self.perspective_fov_radians.is_finite()
                || self.perspective_fov_radians <= 0.0
                || self.perspective_fov_radians >= std::f64::consts::PI)
        {
            return Err("invalid perspective field of view");
        }
        // Rhino retains smaller border settings but does not shrink the fit.
        let border = border.max(1.0);
        let center = bounds.center().map_err(|_| "invalid model bounds")?;
        let target = NaVector3::from(center.to_array());
        let plan_frame = self.plan_frame.with_origin(center);
        let minimum = bounds.min().to_array();
        let maximum = bounds.max().to_array();
        let mut horizontal = 0.0_f64;
        let mut vertical = 0.0_f64;
        let mut half_depth = 0.0_f64;
        let (right, up, forward) = self.perspective_basis();
        for index in 0..8 {
            let corner = NaVector3::from(std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }));
            let local = corner - target;
            let (x, y, z) = match self.kind {
                ViewKind::Top => (local.x, local.y, -local.z),
                ViewKind::Bottom => (local.x, -local.y, local.z),
                ViewKind::Front => (local.x, local.z, local.y),
                ViewKind::Back => (-local.x, local.z, -local.y),
                ViewKind::Right => (local.y, local.z, -local.x),
                ViewKind::Left => (-local.y, local.z, local.x),
                ViewKind::Plan => {
                    let coordinates = plan_frame
                        .projected_coordinates_of(
                            Point3::try_new(corner.x, corner.y, corner.z)
                                .map_err(|_| "invalid model bounds")?,
                        )
                        .map_err(|_| "model extents exceed the supported camera range")?;
                    (
                        coordinates[0],
                        coordinates[1],
                        local.dot(&NaVector3::from(plan_frame.z_axis().as_vector().to_array())),
                    )
                }
                ViewKind::Perspective => (local.dot(&right), local.dot(&up), local.dot(&forward)),
            };
            if ![x, y, z].into_iter().all(Real::is_finite) {
                return Err("model extents exceed the supported camera range");
            }
            horizontal = horizontal.max(x.abs());
            vertical = vertical.max(y.abs());
            half_depth = half_depth.max(z.abs());
        }
        // Rhino fits the bounding box in camera coordinates. This matches the
        // public OpenNURBS ON_DollyExtents algorithm and saved Rhino captures.
        // The border expands its screen dimensions; depth padding is separate.
        // A degenerate screen box receives a one-unit square before aspect fit.
        if horizontal <= Real::EPSILON.sqrt() && vertical <= Real::EPSILON.sqrt() {
            horizontal = 0.5;
            vertical = 0.5;
        } else {
            horizontal *= border;
            vertical *= border;
        }
        let aspect = Real::from(rect.width()) / Real::from(rect.height());
        horizontal = horizontal.max(vertical * aspect);
        vertical = horizontal / aspect;
        let near_padding = (2.0 * half_depth / 256.0).max(if self.kind.is_parallel() {
            0.125
        } else {
            1.0e-6
        });
        let far_padding = near_padding.max(0.125);
        let depth_span = 2.0 * half_depth + near_padding + far_padding;
        let near = if self.kind.is_parallel() {
            0.125 * depth_span
        } else {
            vertical / (self.perspective_fov_radians * 0.5).tan()
        };
        let near = if near <= Real::EPSILON.sqrt() {
            1.0
        } else {
            near
        };
        let distance = near + half_depth + near_padding;
        let far = near + depth_span;
        let scale = Real::from(rect.height()) / (2.0 * vertical);
        if ![horizontal, vertical, near, far, distance, scale]
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
            || far <= near
            || (self.kind.is_parallel()
                && (scale < Real::from(f32::MIN_POSITIVE) || !(scale as f32).is_finite()))
            || (self.kind == ViewKind::Perspective && distance > MAX_PERSPECTIVE_CAMERA_DISTANCE)
        {
            return Err("model extents exceed the supported camera range");
        }
        // Stage the complete current camera, so validation uses the same lens,
        // axes, and projection locks that will be committed after fitting.
        let mut staged = Viewport::new(self.kind);
        staged.restore_camera(self.camera_snapshot());
        staged.target = target;
        staged.camera_target_offset = NaVector3::zeros();
        staged.pan = Vec2::ZERO;
        staged.perspective_lens_shift = [0.0; 2];
        staged.parallel_frustum_shift = [0.0; 2];
        staged.pixels_per_unit = if self.kind.is_parallel() {
            scale
        } else {
            self.pixels_per_unit
        };
        staged.perspective_camera_distance = distance;
        // These are the explicit bounding-box fit's clipping distances. Rhino
        // may subsequently update document clipping using other displayed data.
        staged.frustum_near = near;
        staged.frustum_far = far;
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
                .is_some_and(|point| rect.expand(1.0).contains(point))
            {
                return Err("model extents cannot be represented by this camera");
            }
        }
        Ok(CameraFit {
            camera: staged.camera_snapshot(),
        })
    }
}
