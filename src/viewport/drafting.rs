//! Drafting cursor resolution and construction-plane overlays.

use super::*;
#[cfg(test)]
use viboceros_drafting::ObjectSnapModes;
use viboceros_drafting::{ObjectSnap, OrthogonalTrack, TrackAxis};

#[derive(Clone, Copy, Debug)]
pub(super) struct DraftingCursor {
    pub(super) pointer: Pos2,
    pub(super) source_point: Point3,
    pub(super) point: Point3,
    pub(super) object_snap: Option<ObjectSnap>,
    pub(super) track: Option<OrthogonalTrack>,
    pub(super) ortho: bool,
    pub(super) ortho_z: bool,
    pub(super) grid_snapped: bool,
}

impl Viewport {
    pub(super) fn filtered_drafting_cursor(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        input: DraftingInput,
        filter: Option<viboceros_drafting::PointFilterSession>,
    ) -> Option<DraftingCursor> {
        let mut cursor = self.drafting_cursor(pointer, rect, document, input)?;
        if let Some(filter) = filter {
            cursor.point = filter.preview_point(cursor.source_point).ok()?;
        }
        Some(cursor)
    }

    /// Shared camera-space feature capture for ordinary and constrained prompts.
    /// Parallel views retain indexed point-cloud queries and local-origin precision.
    pub(super) fn object_snap(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        options: impl Into<viboceros_drafting::ObjectSnapOptions>,
    ) -> Option<ObjectSnap> {
        let options = options.into();
        if let Some(projection) = self.point_cloud_projection() {
            let scale = Real::from(self.pixels_per_unit);
            let target = Point3::try_new(self.target.x, self.target.y, self.target.z).ok()?;
            self.object_snap_cache
                .borrow_mut()
                .nearest_axis_aligned_with_options(
                    document,
                    projection,
                    target,
                    self.parallel_query_offset(pointer, rect)?,
                    Real::from(OSNAP_CAPTURE_PIXELS) / scale,
                    options,
                )
                .ok()
                .flatten()
        } else if self.kind == ViewKind::Plan {
            self.object_snap_cache
                .borrow_mut()
                .nearest_in_frame_with_options(
                    document,
                    self.plan_target_frame()?,
                    self.parallel_query_offset(pointer, rect)?,
                    Real::from(OSNAP_CAPTURE_PIXELS) / Real::from(self.pixels_per_unit),
                    options,
                )
                .ok()
                .flatten()
        } else {
            self.object_snap_cache
                .borrow_mut()
                .nearest_projected_with_options(
                    document,
                    [Real::from(pointer.x), Real::from(pointer.y)],
                    Real::from(OSNAP_CAPTURE_PIXELS),
                    |point| self.project_precise(point, rect),
                    options,
                )
                .ok()
                .flatten()
        }
    }

    pub(super) fn drafting_cursor(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        input: DraftingInput,
    ) -> Option<DraftingCursor> {
        // Free picks use the CPlane origin elevation. Planar and Ortho instead
        // resolve the cursor through the previous picked point's elevation.
        let ortho_active = input.ortho ^ input.shift_inverts_ortho;
        let plane_anchor = if input.planar || ortho_active {
            input.anchor
        } else {
            None
        };
        let raw_point = self.unproject_drafting_plane(pointer, rect, plane_anchor);
        // Object snaps are a camera-space query. They remain available even
        // when the construction plane is edge-on or behind the camera.
        let object_snap = self.object_snap(pointer, rect, document, input.snap_options());
        let xy_ortho = if object_snap.is_none() && ortho_active {
            raw_point.zip(input.anchor).and_then(|(cursor, anchor)| {
                viboceros_drafting::plane::ortho_point(
                    cursor,
                    anchor,
                    self.construction_plane(),
                    input.ortho_angle_degrees,
                )
            })
        } else {
            None
        };
        let z_ortho = if object_snap.is_none() && ortho_active && input.ortho_snap_to_cplane_z {
            input.anchor.and_then(|anchor| {
                viboceros_drafting::plane::ortho_z_projected(
                    anchor,
                    self.construction_plane(),
                    [Real::from(pointer.x), Real::from(pointer.y)],
                    |point| {
                        let [x, y] = self.project_precise(point, rect)?;
                        let depth = if self.kind == ViewKind::Perspective {
                            self.view_depth(point)
                        } else {
                            1.0
                        };
                        Some([x, y, depth])
                    },
                )
            })
        } else {
            None
        };
        let ortho_z = z_ortho.is_some_and(|(_, z_distance)| {
            xy_ortho
                .and_then(|point| self.project_precise(point, rect))
                .is_none_or(|[x, y]| {
                    z_distance + 1e-6 < (x - Real::from(pointer.x)).hypot(y - Real::from(pointer.y))
                })
        });
        let ortho_point = if ortho_z {
            z_ortho.map(|(point, _)| point)
        } else {
            xy_ortho
        };
        let track = if object_snap.is_none() && ortho_point.is_none() && input.smart_track {
            raw_point.zip(input.anchor).and_then(|(cursor, anchor)| {
                viboceros_drafting::plane::orthogonal_track_projected(
                    cursor,
                    anchor,
                    self.construction_plane(),
                    [Real::from(pointer.x), Real::from(pointer.y)],
                    Real::from(TRACK_CAPTURE_PIXELS),
                    |point| {
                        self.project(point, rect)
                            .map(|p| [Real::from(p.x), Real::from(p.y)])
                    },
                )
                .ok()
                .flatten()
            })
        } else {
            None
        };
        let grid_point = if input.grid_snap {
            raw_point.and_then(|p| self.snap_to_grid(p))
        } else {
            None
        };
        let point = object_snap
            .map(ObjectSnap::point)
            .or(ortho_point)
            .or_else(|| track.map(OrthogonalTrack::point))
            .or(grid_point)
            .or(raw_point)?;
        Some(DraftingCursor {
            pointer,
            source_point: point,
            point,
            object_snap,
            track,
            ortho: ortho_point.is_some(),
            ortho_z,
            grid_snapped: object_snap.is_none()
                && ortho_point.is_none()
                && track.is_none()
                && grid_point.is_some(),
        })
    }

    pub(super) fn snap_to_grid(&self, point: Point3) -> Option<Point3> {
        viboceros_drafting::plane::snap_to_grid(
            point,
            self.construction_plane(),
            self.grid.snap_spacing,
        )
        .ok()
    }

    pub(super) fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        let pixels_per_model_unit = self.pixels_per_model_unit_at_origin(rect);
        let spacing = self.grid.minor_spacing;
        let pixel_spacing = Real::from(pixels_per_model_unit) * spacing;
        let line_count = i64::from(self.grid.line_count);
        let extent = line_count as Real * spacing;
        if self.grid.show_grid && pixel_spacing >= 8.0 && extent.is_finite() {
            let center = self
                .unproject_drafting_plane(rect.center(), rect, None)
                .and_then(|point| self.construction_plane().coordinates_of(point).ok())
                .unwrap_or([0.0; 3]);
            let half_count = (Real::from(rect.width().max(rect.height())) / pixel_spacing * 1.5)
                .ceil()
                .clamp(10.0, 250.0) as i64;
            let range = |coordinate: Real| {
                let center = (coordinate / spacing).round() as i64;
                (
                    center.saturating_sub(half_count).max(-line_count),
                    center.saturating_add(half_count).min(line_count),
                )
            };
            let minor_stroke = Stroke::new(1.0, Color32::from_gray(218));
            let major_stroke = Stroke::new(1.25, Color32::from_gray(170));
            for (axis, coordinate) in center[..2].iter().copied().enumerate() {
                let (first, last) = range(coordinate);
                for index in first..=last {
                    if index == 0 {
                        continue;
                    }
                    let coordinate = index as Real * spacing;
                    let stroke = if index.unsigned_abs() % u64::from(self.grid.major_interval) == 0
                    {
                        major_stroke
                    } else {
                        minor_stroke
                    };
                    let (start, end) = if axis == 0 {
                        (
                            self.grid_point(coordinate, -extent),
                            self.grid_point(coordinate, extent),
                        )
                    } else {
                        (
                            self.grid_point(-extent, coordinate),
                            self.grid_point(extent, coordinate),
                        )
                    };
                    self.paint_grid_line(painter, rect, start, end, stroke);
                }
            }
        }

        if self.grid.show_axes && extent.is_finite() {
            let horizontal_color = Color32::from_rgb(190, 65, 65);
            let vertical_color = Color32::from_rgb(60, 145, 75);
            self.paint_grid_line(
                painter,
                rect,
                self.grid_point(-extent, 0.0),
                self.grid_point(extent, 0.0),
                Stroke::new(1.5, horizontal_color),
            );
            self.paint_grid_line(
                painter,
                rect,
                self.grid_point(0.0, -extent),
                self.grid_point(0.0, extent),
                Stroke::new(1.5, vertical_color),
            );
        }
        if self.grid.show_world_axes {
            self.paint_world_axes_icon(painter, rect);
        }
    }

    fn paint_world_axes_icon(&self, painter: &egui::Painter, rect: Rect) {
        let anchor = Pos2::new(rect.left() + 22.0, rect.bottom() - 22.0);
        for (label, direction, color) in [
            (
                "X",
                Vector3::try_new(1.0, 0.0, 0.0).unwrap(),
                Color32::from_rgb(190, 65, 65),
            ),
            (
                "Y",
                Vector3::try_new(0.0, 1.0, 0.0).unwrap(),
                Color32::from_rgb(60, 145, 75),
            ),
            (
                "Z",
                Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
                Color32::from_rgb(65, 100, 185),
            ),
        ] {
            if let Some(tip) = self.world_axis_icon_tip(anchor, direction, rect) {
                painter.line_segment([anchor, tip], Stroke::new(1.5, color));
                painter.text(
                    tip,
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(9.0),
                    color,
                );
            } else {
                painter.circle_stroke(anchor, 3.0, Stroke::new(1.5, color));
                painter.text(
                    anchor + Vec2::new(0.0, -9.0),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(9.0),
                    color,
                );
            }
        }
    }

    fn world_axis_icon_tip(&self, anchor: Pos2, direction: Vector3, rect: Rect) -> Option<Pos2> {
        let target = Point3::try_new(self.target.x, self.target.y, self.target.z).ok();
        let projected = target.and_then(|target| {
            let shifted = target.translated(direction).ok()?;
            Some((
                self.project_precise(target, rect)?,
                self.project_precise(shifted, rect)?,
            ))
        });
        projected
            .map(|(from, to)| egui::vec2((to[0] - from[0]) as f32, (to[1] - from[1]) as f32))
            .filter(|offset| offset.is_finite() && offset.length_sq() > 1.0)
            .map(|offset| offset.normalized() * 13.0)
            .map(|offset| anchor + offset)
    }

    pub(super) fn grid_point(&self, horizontal: Real, vertical: Real) -> Option<Point3> {
        self.construction_plane()
            .point_at([horizontal, vertical, 0.0])
            .ok()
    }

    fn paint_grid_line(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        start: Option<Point3>,
        end: Option<Point3>,
        stroke: Stroke,
    ) {
        if let (Some(start), Some(end)) = (start, end)
            && let Some(segment) = self.grid_segment(rect, start, end)
        {
            painter.line_segment(segment, stroke);
        }
    }

    // Clip in model space first: a camera-crossing grid line still has a
    // visible part. Then bound its screen coordinates before egui tessellates
    // the stroke, avoiding enormous near-plane endpoints and lost precision.
    fn grid_segment(&self, rect: Rect, start: Point3, end: Point3) -> Option<[Pos2; 2]> {
        let [start, end] = self.project_segment(start, end, rect)?;
        clip_drafting_line(start, end, rect, false)
    }

    pub(super) fn paint_draft_points(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        points: &[Point3],
    ) {
        for vertices in points.windows(2) {
            if let (Some(start), Some(end)) = (
                self.project(vertices[0], rect),
                self.project(vertices[1], rect),
            ) {
                painter.line_segment([start, end], Stroke::new(1.5, Color32::from_gray(80)));
            }
        }
        // Keep even a single accepted point visible while typing elsewhere.
        for point in points {
            if let Some(position) = self.project(*point, rect) {
                painter.circle_filled(position, 2.5, Color32::from_gray(80));
            }
        }
    }

    pub(super) fn paint_drafting(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: DraftingInput,
        cursor: DraftingCursor,
    ) {
        const TRACK_COLOR: Color32 = Color32::from_rgb(15, 155, 190);
        const GRID_COLOR: Color32 = Color32::from_rgb(80, 120, 45);

        if let Some(track) = cursor.track
            && let Some(anchor_point) = input.anchor
            && let Some(anchor) = self.project(anchor_point, rect)
        {
            let stroke = Stroke::new(1.0, TRACK_COLOR);
            let plane = self.construction_plane().with_origin(anchor_point);
            for (axis, coordinate) in [
                (TrackAxis::Horizontal, [1.0, 0.0, 0.0]),
                (TrackAxis::Vertical, [0.0, 1.0, 0.0]),
            ] {
                if (track.axis() == axis || track.axis() == TrackAxis::Both)
                    && let Ok(point) = plane.point_at(coordinate)
                    && let Some(projected) = self.project(point, rect)
                    && let Some(segment) = clip_drafting_line(anchor, projected, rect, true)
                {
                    painter.extend(egui::Shape::dashed_line(&segment, stroke, 6.0, 4.0));
                }
            }
        }

        if let Some(anchor) = input.anchor.and_then(|point| self.project(point, rect))
            && let Some(target) = self.project(cursor.point, rect)
            && let Some(segment) = clip_drafting_line(anchor, target, rect, false)
        {
            painter.extend(egui::Shape::dashed_line(
                &segment,
                Stroke::new(1.25, Color32::from_gray(80)),
                7.0,
                4.0,
            ));
        }

        if let (Some(anchor), Some(reference)) = (
            input.anchor.and_then(|point| self.project(point, rect)),
            input.reference.and_then(|point| self.project(point, rect)),
        ) {
            const REFERENCE_COLOR: Color32 = Color32::from_rgb(125, 80, 180);
            if let Some(segment) = clip_drafting_line(anchor, reference, rect, false) {
                painter.extend(egui::Shape::dashed_line(
                    &segment,
                    Stroke::new(1.5, REFERENCE_COLOR),
                    5.0,
                    3.0,
                ));
            }
            painter.circle_stroke(reference, 5.0, Stroke::new(1.25, REFERENCE_COLOR));
            painter.text(
                reference + Vec2::new(8.0, -8.0),
                Align2::LEFT_BOTTOM,
                "Reference",
                FontId::proportional(11.0),
                REFERENCE_COLOR,
            );
        }

        let Some(target) = self.project(cursor.point, rect) else {
            return;
        };
        let marker_color = if cursor.object_snap.is_some() {
            SNAP_COLOR
        } else if cursor.track.is_some() || cursor.ortho {
            TRACK_COLOR
        } else if cursor.grid_snapped {
            GRID_COLOR
        } else {
            Color32::from_gray(80)
        };
        painter.line_segment(
            [target - Vec2::new(6.0, 0.0), target + Vec2::new(6.0, 0.0)],
            Stroke::new(1.25, marker_color),
        );
        painter.line_segment(
            [target - Vec2::new(0.0, 6.0), target + Vec2::new(0.0, 6.0)],
            Stroke::new(1.25, marker_color),
        );

        let snap_label = cursor
            .object_snap
            .map(|snap| snap.kind().label())
            .or_else(|| cursor.track.map(|track| track.axis().label()))
            .or(cursor.ortho_z.then_some("Ortho Z"))
            .or(cursor.ortho.then_some("Ortho"))
            .or(cursor.grid_snapped.then_some("Grid"));
        if let Some(label) = snap_label {
            painter.text(
                target + Vec2::new(9.0, -8.0),
                Align2::LEFT_BOTTOM,
                label,
                FontId::proportional(12.0),
                marker_color,
            );
        }
        painter.text(
            cursor.pointer + Vec2::new(12.0, 14.0),
            Align2::LEFT_TOP,
            format!(
                "{:.3}, {:.3}, {:.3}",
                cursor.point.x(),
                cursor.point.y(),
                cursor.point.z()
            ),
            FontId::monospace(11.0),
            Color32::from_gray(75),
        );
    }
}

/// Bound dash tessellation to the viewport; point-sized guides are not drawn.
pub(super) fn clip_drafting_line(
    start: Pos2,
    end: Pos2,
    rect: Rect,
    extend: bool,
) -> Option<[Pos2; 2]> {
    if start == end || !rect.is_positive() {
        return None;
    }
    super::screen::clip_line_to_rect(start, end, rect, extend)
}

#[cfg(test)]
mod center_tests;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod mid_tests;
#[cfg(test)]
mod near_tests;
#[cfg(test)]
mod grid_tests {
    use super::*;

    fn grid_shapes(view: &Viewport) -> Vec<egui::Shape> {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let output = egui::Context::default().run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| view.paint_grid(ui.painter(), rect),
        );
        let shapes = output
            .shapes
            .iter()
            .map(|shape| shape.shape.clone())
            .collect();
        output.drop_without_applying_deltas();
        shapes
    }

    #[test]
    fn grid_settings_control_line_count_major_strokes_axes_and_icon() {
        let mut view = Viewport::new(ViewKind::Top);
        view.grid = GridSettings {
            minor_spacing: 2.0,
            major_interval: 2,
            line_count: 2,
            show_grid: true,
            show_axes: false,
            show_world_axes: false,
            ..GridSettings::default()
        };
        let shapes = grid_shapes(&view);
        let lines = shapes
            .iter()
            .filter_map(|shape| match shape {
                egui::Shape::LineSegment { stroke, .. } => Some(stroke.color),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 8);
        assert_eq!(
            lines
                .iter()
                .filter(|color| **color == Color32::from_gray(170))
                .count(),
            4
        );
        view.grid.show_grid = false;
        assert!(grid_shapes(&view).is_empty());
        view.grid.show_axes = true;
        assert_eq!(
            grid_shapes(&view)
                .iter()
                .filter(|shape| matches!(shape, egui::Shape::LineSegment { .. }))
                .count(),
            2
        );
        view.grid.show_axes = false;
        view.grid.show_world_axes = true;
        assert_eq!(
            grid_shapes(&view)
                .iter()
                .filter(|shape| matches!(shape, egui::Shape::LineSegment { .. }))
                .count(),
            2
        );
        view.grid.show_world_axes = false;
        view.grid.show_grid = true;
        view.pan = Vec2::new(10_000.0, 0.0);
        assert!(grid_shapes(&view).is_empty());
    }

    #[test]
    fn large_configured_grid_submits_only_visible_nearby_lines() {
        let mut view = Viewport::new(ViewKind::Top);
        view.grid.line_count = 100_000;
        view.grid.show_axes = false;
        let count = grid_shapes(&view)
            .iter()
            .filter(|shape| matches!(shape, egui::Shape::LineSegment { .. }))
            .count();
        assert!(count > 0 && count <= 1_000, "submitted {count} lines");
        view.pan = Vec2::new(40_000.0, -40_000.0);
        let count = grid_shapes(&view)
            .iter()
            .filter(|shape| matches!(shape, egui::Shape::LineSegment { .. }))
            .count();
        assert!(
            count > 0 && count <= 1_000,
            "submitted {count} lines after pan"
        );
    }

    #[test]
    fn perspective_grid_crossing_camera_is_clipped_in_both_endpoint_orders() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let mut view = Viewport::new(ViewKind::Perspective);
        for yaw in [-0.6, 0., 0.6] {
            for pitch in [-0.5, 0.0001, 0.5] {
                view.orbit_yaw = yaw;
                view.orbit_pitch = pitch;
                let a = view.grid_point(-200., 0.).unwrap();
                let b = view.grid_point(200., 0.).unwrap();
                assert!(view.project(a, rect).is_some());
                assert!(view.project(b, rect).is_none());
                let clipped = view
                    .grid_segment(rect, a, b)
                    .expect("visible part of camera-crossing grid");
                let reversed = view.grid_segment(rect, b, a).unwrap();
                for point in clipped {
                    assert!(point.is_finite() && rect.contains(point));
                }
                for (a, b) in clipped.into_iter().zip(reversed.into_iter().rev()) {
                    assert!(a.distance(b) < 0.05, "{yaw} {pitch}: {a:?} != {b:?}");
                }
                let origin = view
                    .project(view.grid_point(0., 0.).unwrap(), rect)
                    .unwrap();
                assert!(
                    super::super::screen::point_segment_distance(origin, clipped[0], clipped[1])
                        < 0.05
                );
            }
        }
    }

    #[test]
    fn grid_strokes_are_bounded_and_fully_behind_camera_lines_are_omitted() {
        let rect = Rect::from_min_size(Pos2::new(15., 25.), Vec2::new(800., 600.));
        let mut view = Viewport::new(ViewKind::Perspective);
        view.orbit_yaw = 0.;
        view.orbit_pitch = 0.5;
        assert!(
            view.grid_segment(
                rect,
                view.grid_point(200., -20.).unwrap(),
                view.grid_point(200., 20.).unwrap()
            )
            .is_none()
        );
        let context = egui::Context::default();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ui| {
                view.paint_grid(ui.painter(), rect);
            },
        );
        let mut lines = 0;
        for shape in &output.shapes {
            if let egui::Shape::LineSegment { points, .. } = &shape.shape {
                lines += 1;
                assert!(
                    points
                        .iter()
                        .all(|point| point.is_finite() && rect.contains(*point))
                );
            }
        }
        assert!(lines > 10);
        output.drop_without_applying_deltas();
    }
}
