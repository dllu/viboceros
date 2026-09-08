//! Drafting cursor resolution and construction-plane overlays.

use super::*;
use viboceros_drafting::{
    ObjectSnap, OrthogonalTrack, TrackAxis, nearest_object_snap_projected,
    nearest_object_snap_relative,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct DraftingCursor {
    pub(super) pointer: Pos2,
    pub(super) point: Point3,
    pub(super) object_snap: Option<ObjectSnap>,
    pub(super) track: Option<OrthogonalTrack>,
    pub(super) grid_snapped: bool,
}

impl Viewport {
    pub(super) fn drafting_cursor(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        input: DraftingInput,
    ) -> Option<DraftingCursor> {
        let raw_point = self.unproject_drafting_plane(pointer, rect, input.anchor);
        // Object snaps are a camera-space query. They remain available even
        // when the construction plane is edge-on or behind the camera.
        let object_snap = if input.osnap {
            if self.kind == ViewKind::Top {
                let origin = self.world_origin(rect);
                let scale = Real::from(self.pixels_per_unit);
                Point3::try_new(self.target.x, self.target.y, 0.0)
                    .ok()
                    .and_then(|target| {
                        nearest_object_snap_relative(
                            document,
                            target,
                            [
                                (Real::from(pointer.x) - Real::from(origin.x)) / scale,
                                (Real::from(origin.y) - Real::from(pointer.y)) / scale,
                            ],
                            Real::from(OSNAP_CAPTURE_PIXELS) / scale,
                        )
                        .ok()
                        .flatten()
                    })
            } else {
                nearest_object_snap_projected(
                    document,
                    [Real::from(pointer.x), Real::from(pointer.y)],
                    Real::from(OSNAP_CAPTURE_PIXELS),
                    |point| {
                        self.project(point, rect)
                            .map(|p| [Real::from(p.x), Real::from(p.y)])
                    },
                )
                .ok()
                .flatten()
            }
        } else {
            None
        };
        let track = if object_snap.is_none() && input.smart_track {
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
            .or_else(|| track.map(OrthogonalTrack::point))
            .or(grid_point)
            .or(raw_point)?;
        Some(DraftingCursor {
            pointer,
            point,
            object_snap,
            track,
            grid_snapped: object_snap.is_none() && track.is_none() && grid_point.is_some(),
        })
    }

    pub(super) fn snap_to_grid(&self, point: Point3) -> Option<Point3> {
        viboceros_drafting::plane::snap_to_grid(point, self.construction_plane(), GRID_SPACING).ok()
    }

    pub(super) fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        let pixels_per_model_unit = self.pixels_per_model_unit_at_origin(rect);
        let half_count = ((rect.width().max(rect.height()) / pixels_per_model_unit * 1.5).ceil()
            as i32)
            .clamp(10, 250);
        let extent = Real::from(half_count) * GRID_SPACING;
        if pixels_per_model_unit * GRID_SPACING as f32 >= 8.0 {
            let grid_stroke = Stroke::new(1.0, Color32::from_gray(218));
            for index in -half_count..=half_count {
                if index == 0 {
                    continue;
                }
                let coordinate = Real::from(index) * GRID_SPACING;
                self.paint_grid_line(
                    painter,
                    rect,
                    self.grid_point(coordinate, -extent),
                    self.grid_point(coordinate, extent),
                    grid_stroke,
                );
                self.paint_grid_line(
                    painter,
                    rect,
                    self.grid_point(-extent, coordinate),
                    self.grid_point(extent, coordinate),
                    grid_stroke,
                );
            }
        }

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
        if let (Some(start), Some(end)) = (
            start.and_then(|p| self.project(p, rect)),
            end.and_then(|p| self.project(p, rect)),
        ) {
            painter.line_segment([start, end], stroke);
        }
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
        const SNAP_COLOR: Color32 = Color32::from_rgb(210, 45, 145);
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
        } else if cursor.track.is_some() {
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

/// Clip before dash tessellation: painter clipping alone would still allocate
/// dashes all the way to a distant anchor. `extend` clips the infinite line.
pub(super) fn clip_drafting_line(
    start: Pos2,
    end: Pos2,
    rect: Rect,
    extend: bool,
) -> Option<[Pos2; 2]> {
    if !start.is_finite() || !end.is_finite() || !rect.is_finite() || !rect.is_positive() {
        return None;
    }
    // Subtract in f64: finite f32 screen coordinates can overflow f32 deltas.
    let origin = [f64::from(start.x), f64::from(start.y)];
    let direction = [f64::from(end.x) - origin[0], f64::from(end.y) - origin[1]];
    if direction == [0.0; 2] {
        return None;
    }
    let bounds = [
        [f64::from(rect.left()), f64::from(rect.right())],
        [f64::from(rect.top()), f64::from(rect.bottom())],
    ];
    let (mut low, mut high) = if extend {
        (f64::NEG_INFINITY, f64::INFINITY)
    } else {
        (0.0, 1.0)
    };
    for axis in 0..2 {
        if direction[axis] == 0.0 {
            if !(bounds[axis][0]..=bounds[axis][1]).contains(&origin[axis]) {
                return None;
            }
        } else {
            let a = (bounds[axis][0] - origin[axis]) / direction[axis];
            let b = (bounds[axis][1] - origin[axis]) / direction[axis];
            low = low.max(a.min(b));
            high = high.min(a.max(b));
            if low > high {
                return None;
            }
        }
    }
    Some([low, high].map(|t| {
        let point: [f32; 2] = std::array::from_fn(|axis| {
            direction[axis]
                .mul_add(t, origin[axis])
                .clamp(bounds[axis][0], bounds[axis][1]) as f32
        });
        Pos2::new(point[0], point[1])
    }))
}
