//! Checked clipping distances and document bounds in the infinite view frustum.
//!
//! Constraint rules follow the public OpenNURBS ON_Viewport::SetFrustumNearFar
//! adaptation in third_party/opennurbs_rust/viewport_clipping.rs. Document
//! padding and bias are independently calibrated from public Rhino captures.

use super::*;
use viboceros_geometry::BoundingBox3;

const ZERO_TOLERANCE: Real = 1.0 / 4_294_967_296.0;
const DOCUMENT_MIN_NEAR: Real = 0.005;
const DOCUMENT_MIN_RATIO: Real = 0.0005;
// Measured from the near-plane displacement in Rhino 8.32 Wireframe captures.
const DOCUMENT_RELATIVE_BIAS: Real = 7.592899120744e-6;

#[path = "../../third_party/opennurbs_rust/viewport_clipping.rs"]
mod constraints;
pub(super) use constraints::{ClipAdjustment, ClipRequest};

#[derive(PartialEq)]
pub(super) struct ClipRefreshKey {
    camera: CameraSnapshot,
    size: [f32; 2],
    bounds: Option<BoundingBox3>,
}

impl ClipAdjustment {
    fn document(
        minimum: Real,
        maximum: Real,
        perspective: bool,
        distance: Real,
    ) -> Result<Self, &'static str> {
        if !minimum.is_finite() || !maximum.is_finite() || minimum > maximum {
            return Err("invalid document clipping bounds");
        }
        let (mut near, far) = if perspective {
            if maximum - minimum <= ZERO_TOLERANCE {
                (0.675 * minimum, 1.125 * maximum)
            } else {
                (0.99 * minimum, 1.01 * maximum)
            }
        } else {
            let padding = (0.05 * (maximum - minimum)).max(0.5);
            (minimum - padding, maximum + padding)
        };
        if perspective {
            // A fitted tiny model can be closer than Rhino's display minimum.
            // Keep camera pose and optical framing independent of clip metadata.
            if far <= DOCUMENT_MIN_NEAR {
                return Ok(Self {
                    near: DOCUMENT_MIN_NEAR,
                    far: DOCUMENT_MIN_NEAR / DOCUMENT_MIN_RATIO,
                    camera_dolly: 0.0,
                });
            }
            let mut result = Self::constrained(
                ClipRequest {
                    near,
                    far,
                    min_near: DOCUMENT_MIN_NEAR,
                    min_ratio: DOCUMENT_MIN_RATIO,
                    target_distance: distance,
                },
                true,
                0.0,
                0.0,
            )?;
            let bias = 1.001 * DOCUMENT_RELATIVE_BIAS * (result.far - result.near);
            result.near = (result.near - bias)
                .max(0.99 * DOCUMENT_MIN_RATIO * result.far)
                .max(DOCUMENT_MIN_NEAR);
            Ok(result)
        } else {
            let span = far - near;
            let camera_dolly = if near < DOCUMENT_MIN_NEAR {
                DOCUMENT_MIN_NEAR * 1.00001 + DOCUMENT_RELATIVE_BIAS * span - near
            } else {
                0.0
            };
            near = (near + camera_dolly - 1.001 * DOCUMENT_RELATIVE_BIAS * span)
                .max(DOCUMENT_MIN_NEAR);
            let far = far + camera_dolly;
            if ![near, far, camera_dolly].into_iter().all(Real::is_finite) || far <= near {
                return Err("document clipping exceeds the supported range");
            }
            Ok(Self {
                near,
                far,
                camera_dolly,
            })
        }
    }
}

impl Viewport {
    pub(super) fn apply_clip_adjustment(
        &mut self,
        update: ClipAdjustment,
    ) -> Result<(), &'static str> {
        let distance = self.perspective_camera_distance + update.camera_dolly;
        if !distance.is_finite() || distance <= 0.0 {
            return Err("clipping camera exceeds the supported range");
        }
        self.perspective_camera_distance = distance;
        self.frustum_near = update.near;
        self.frustum_far = update.far;
        Ok(())
    }

    /// Bounds outside the infinite view region do not affect clipping. Work
    /// relative to the target before transforming, preserving translated detail.
    pub(super) fn bounding_box_depth(
        &self,
        bounds: BoundingBox3,
        rect: Rect,
    ) -> Option<(Real, Real)> {
        let frame = if self.kind == ViewKind::Perspective {
            None
        } else if self.kind == ViewKind::Plan {
            Some(self.plan_frame)
        } else {
            Some(Self::default_plane(self.kind))
        };
        let (right, up, forward) = frame.map_or_else(
            || self.perspective_basis(),
            |frame| {
                (
                    NaVector3::from(frame.x_axis().as_vector().to_array()),
                    NaVector3::from(frame.y_axis().as_vector().to_array()),
                    -NaVector3::from(frame.z_axis().as_vector().to_array()),
                )
            },
        );
        let minimum = bounds.min().to_array();
        let maximum = bounds.max().to_array();
        let mut corners = [NaVector3::zeros(); 8];
        for (index, corner) in corners.iter_mut().enumerate() {
            let point = Point3::try_from(std::array::from_fn(|axis| {
                if index & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }))
            .ok()?;
            self.gpu_position(point)?;
            let local = NaVector3::from(point.to_array()) - self.target;
            *corner = NaVector3::new(
                local.dot(&right),
                local.dot(&up),
                local.dot(&forward) + self.perspective_camera_distance,
            );
        }
        let perspective = self.kind == ViewKind::Perspective;
        // The public depth query requires at least one corner strictly in
        // front of a perspective camera, including for degenerate boxes.
        if perspective && corners.iter().all(|point| point.z <= 0.0) {
            return None;
        }
        let [sx, sy] = self.perspective_lens_shift;
        let (x0, x1, y0, y1) = if perspective {
            let h = (self.perspective_fov_radians * 0.5).tan();
            let w = h * Real::from(rect.width()) / Real::from(rect.height());
            (
                (sx - 1.0) * w,
                (sx + 1.0) * w,
                (sy - 1.0) * h,
                (sy + 1.0) * h,
            )
        } else {
            let w = Real::from(rect.width()) / (2.0 * self.pixels_per_unit);
            let h = Real::from(rect.height()) / (2.0 * self.pixels_per_unit);
            let x = -Real::from(self.pan.x) / self.pixels_per_unit;
            let y = Real::from(self.pan.y) / self.pixels_per_unit;
            (x - w, x + w, y - h, y + h)
        };
        let planes = if perspective {
            [
                [1., 0., -x0, 0.],
                [-1., 0., x1, 0.],
                [0., 1., -y0, 0.],
                [0., -1., y1, 0.],
                [0., 0., 1., 0.],
            ]
        } else {
            [
                [1., 0., 0., -x0],
                [-1., 0., 0., x1],
                [0., 1., 0., -y0],
                [0., -1., 0., y1],
                [0.; 4],
            ]
        };
        let planes = &planes[..if perspective { 5 } else { 4 }];
        let value = |plane: [Real; 4], point: NaVector3<Real>| {
            plane[0] * point.x + plane[1] * point.y + plane[2] * point.z + plane[3]
        };
        let axis_target = if perspective {
            self.target
        } else {
            self.target - right * (Real::from(self.pan.x) / self.pixels_per_unit)
                + up * (Real::from(self.pan.y) / self.pixels_per_unit)
                - right
                    * (self.parallel_frustum_shift[0] * Real::from(rect.width())
                        / (2. * self.pixels_per_unit))
                - up * (self.parallel_frustum_shift[1] * Real::from(rect.height())
                    / (2. * self.pixels_per_unit))
        };
        let camera = axis_target - forward * self.perspective_camera_distance;
        let tolerance =
            Real::EPSILON.sqrt() * (1. + camera.iter().fold(0.0_f64, |a, b| a.max(b.abs())));
        let thresholds = planes
            .iter()
            .map(|plane| {
                -tolerance
                    * (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt()
            })
            .collect::<Vec<_>>();
        if planes.iter().zip(&thresholds).any(|(plane, threshold)| {
            corners
                .iter()
                .all(|point| value(*plane, *point) < *threshold)
        }) {
            return None;
        }
        if (!perspective || corners.iter().all(|point| point.z > 0.0))
            && planes.iter().zip(&thresholds).all(|(plane, threshold)| {
                corners
                    .iter()
                    .all(|point| value(*plane, *point) >= *threshold)
            })
        {
            return Some((
                corners
                    .iter()
                    .map(|point| point.z)
                    .fold(Real::INFINITY, Real::min),
                corners
                    .iter()
                    .map(|point| point.z)
                    .fold(Real::NEG_INFINITY, Real::max),
            ));
        }
        let mut depths: Option<(Real, Real)> = None;
        let mut include = |depth: Real| {
            if depth.is_finite() {
                depths = Some(depths.map_or((depth, depth), |(a, b)| (a.min(depth), b.max(depth))));
            }
        };
        for face in [
            [0, 1, 3, 2],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 3, 7, 5],
        ] {
            let mut polygon = face
                .into_iter()
                .map(|index| corners[index])
                .collect::<Vec<_>>();
            for &plane in planes {
                if polygon.is_empty() {
                    break;
                }
                let evaluate = |point: NaVector3<Real>| {
                    plane[0] * point.x + plane[1] * point.y + plane[2] * point.z + plane[3]
                };
                let mut clipped = Vec::with_capacity(polygon.len() + 1);
                let mut previous = *polygon.last().unwrap();
                let mut a = evaluate(previous);
                for point in polygon {
                    let b = evaluate(point);
                    if (a >= 0.) != (b >= 0.) {
                        let ratio = a / (a - b);
                        clipped.push(previous * (1. - ratio) + point * ratio);
                    }
                    if b >= 0. {
                        clipped.push(point);
                    }
                    previous = point;
                    a = b;
                }
                polygon = clipped;
            }
            for point in polygon {
                include(point.z);
            }
        }
        // OpenNURBS includes frustum corner rays intersecting a box expanded by
        // its camera-coordinate tolerance. Retain that conservative depth
        // allowance, rather than clipping only the exact box surface.
        for (x, y, side_x, side_y) in [
            (x0, y0, 0, 2),
            (x1, y0, 1, 2),
            (x0, y1, 0, 3),
            (x1, y1, 1, 3),
        ] {
            let crosses = |side: usize| {
                corners
                    .iter()
                    .any(|point| value(planes[side], *point) >= thresholds[side])
                    && corners
                        .iter()
                        .any(|point| value(planes[side], *point) < thresholds[side])
            };
            if !crosses(side_x) || !crosses(side_y) {
                continue;
            }
            let (origin, direction) = if perspective {
                (camera, forward + right * x + up * y)
            } else {
                (
                    self.target + right * x + up * y - forward * self.perspective_camera_distance,
                    forward,
                )
            };
            let mut a = if perspective { 0.0 } else { Real::NEG_INFINITY };
            let mut b = Real::INFINITY;
            for axis in 0..3 {
                let lo = minimum[axis] - tolerance;
                let hi = maximum[axis] + tolerance;
                if direction[axis] == 0.0 {
                    if origin[axis] < lo || origin[axis] > hi {
                        a = 1.;
                        b = 0.;
                        break;
                    }
                } else {
                    let c = (lo - origin[axis]) / direction[axis];
                    let d = (hi - origin[axis]) / direction[axis];
                    a = a.max(c.min(d));
                    b = b.min(c.max(d));
                }
            }
            if a <= b {
                include(a);
                include(b);
            }
        }
        if perspective
            && (0..3).all(|axis| minimum[axis] <= camera[axis] && camera[axis] <= maximum[axis])
        {
            include(0.);
        }
        depths
    }

    pub(super) fn update_clipping_from_document(
        &mut self,
        document: &Document,
        rect: Rect,
    ) -> Result<(), &'static str> {
        self.update_clipping_from_bounds(self.visible_document_bounds(document), rect)
    }

    fn visible_document_bounds(&self, document: &Document) -> Option<BoundingBox3> {
        let mut cache = self.display_cache.borrow_mut();
        document
            .objects()
            .filter(|object| {
                object.attributes().is_visible()
                    && document
                        .layer(object.attributes().layer_id())
                        .is_some_and(|layer| layer.is_visible())
            })
            // Keep unsupported unselected geometry from invalidating a valid
            // fit. The renderer likewise omits unrepresentable local vertices.
            .map(|object| cache.get(object, document.tolerance()).bounds())
            .filter(|bounds| {
                self.gpu_position(bounds.min()).is_some()
                    && self.gpu_position(bounds.max()).is_some()
            })
            .reduce(|a, b| a.union(b).expect("finite document bounds"))
    }

    #[cfg(test)]
    pub(super) fn refresh_clipping(
        &mut self,
        document: &Document,
        rect: Rect,
    ) -> Result<bool, &'static str> {
        self.refresh_clipping_with_transform(document, rect, None)
    }

    #[cfg(test)]
    pub(super) fn refresh_clipping_with_transform(
        &mut self,
        document: &Document,
        rect: Rect,
        transform: Option<super::object_preview::TransformedObjects<'_>>,
    ) -> Result<bool, &'static str> {
        self.refresh_clipping_with_preview(
            document,
            rect,
            transform.map(super::object_preview::ObjectPreview::Affine),
        )
    }

    pub(super) fn refresh_clipping_with_preview(
        &mut self,
        document: &Document,
        rect: Rect,
        preview: Option<super::object_preview::ObjectPreview<'_>>,
    ) -> Result<bool, &'static str> {
        let transform = match preview {
            Some(super::object_preview::ObjectPreview::Affine(map)) => Some(map),
            _ => None,
        };
        let mut bounds = self.visible_document_bounds(document);
        if let Some(transform) = transform {
            let mut cache = self.display_cache.borrow_mut();
            for id in transform.sources {
                let Some(object) = document.object(*id).filter(|object| {
                    object.attributes().is_visible()
                        && document
                            .layer(object.attributes().layer_id())
                            .is_some_and(|l| l.is_visible())
                }) else {
                    continue;
                };
                let source = cache.get(object, document.tolerance()).bounds();
                let min = source.min().to_array();
                let max = source.max().to_array();
                let corners = (0..8)
                    .map(|mask| {
                        let coordinates = std::array::from_fn(|axis| {
                            if mask & (1 << axis) == 0 {
                                min[axis]
                            } else {
                                max[axis]
                            }
                        });
                        transform
                            .transform
                            .transform_point(Point3::try_from(coordinates).unwrap())
                    })
                    .collect::<Result<Vec<_>, _>>();
                if let Ok(corners) = corners
                    && let Ok(transformed) = BoundingBox3::from_points(corners)
                    && self.gpu_position(transformed.min()).is_some()
                    && self.gpu_position(transformed.max()).is_some()
                {
                    bounds = Some(bounds.map_or(transformed, |b| b.union(transformed).unwrap()));
                }
            }
        }
        if let Some(super::object_preview::ObjectPreview::Deformed(objects)) = preview {
            for (id, display) in objects {
                if !document.object(*id).is_some_and(|o| {
                    o.attributes().is_visible()
                        && document
                            .layer(o.attributes().layer_id())
                            .is_some_and(|l| l.is_visible())
                }) {
                    continue;
                }
                let mut posed = display.geometry.bounds();
                if let Some(map) = display.transform {
                    let lo = posed.min().to_array();
                    let hi = posed.max().to_array();
                    let corners = (0..8)
                        .map(|mask| {
                            Point3::try_from(std::array::from_fn(|axis| {
                                if mask & (1 << axis) == 0 {
                                    lo[axis]
                                } else {
                                    hi[axis]
                                }
                            }))
                            .and_then(|p| map.transform_point(p))
                        })
                        .collect::<Result<Vec<_>, _>>();
                    let Ok(corners) = corners else {
                        continue;
                    };
                    let Ok(mapped) = BoundingBox3::from_points(corners) else {
                        continue;
                    };
                    posed = mapped;
                }
                if self.gpu_position(posed.min()).is_some()
                    && self.gpu_position(posed.max()).is_some()
                {
                    bounds = Some(bounds.map_or(posed, |b| b.union(posed).unwrap()));
                }
            }
        }
        let key = ClipRefreshKey {
            camera: self.camera_snapshot(),
            size: [rect.width(), rect.height()],
            bounds,
        };
        if self.cached_clipping.as_ref() == Some(&key) {
            return Ok(false);
        }
        self.update_clipping_from_bounds(bounds, rect)?;
        // Retain the input pose: a parallel dolly changes the tolerance used
        // on the following redraw. Reusing that result prematurely loses the
        // small far-plane update measured in Rhino.
        self.cached_clipping = Some(key);
        Ok(true)
    }

    fn update_clipping_from_bounds(
        &mut self,
        bounds: Option<BoundingBox3>,
        rect: Rect,
    ) -> Result<(), &'static str> {
        if !rect.is_finite() || !rect.is_positive() {
            return Err("invalid viewport dimensions");
        }
        // With no visible geometry Rhino uses a unit box at the world origin.
        let bounds = bounds.unwrap_or_else(|| {
            BoundingBox3::from_points([
                Point3::try_new(-1., -1., -1.).unwrap(),
                Point3::try_new(1., 1., 1.).unwrap(),
            ])
            .unwrap()
        });
        // Rhino intersects the combined scene box with the infinite frustum;
        // an off-screen object can therefore alter the resulting clip interval.
        let (minimum, maximum) = self
            .bounding_box_depth(bounds, rect)
            .unwrap_or((DOCUMENT_MIN_NEAR, 1000.));
        let update = ClipAdjustment::document(
            minimum,
            maximum,
            self.kind == ViewKind::Perspective,
            self.perspective_camera_distance,
        )?;
        self.apply_clip_adjustment(update)?;
        Ok(())
    }
}
