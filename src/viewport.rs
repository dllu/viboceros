use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use viboceros_document::{Document, Geometry};
use viboceros_drafting::{
    ObjectSnap, OrthogonalTrack, TrackAxis, nearest_object_snap, orthogonal_track,
};
use viboceros_geometry::{NurbsCurve, Point3, Real, TriangleMesh, UnitVector3};

const OSNAP_CAPTURE_PIXELS: f32 = 12.0;
const TRACK_CAPTURE_PIXELS: f32 = 8.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode {
    Wireframe,
    Shaded,
    Ghosted,
}

impl DisplayMode {
    fn label(self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::Shaded => "Shaded",
            Self::Ghosted => "Ghosted",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DraftingInput {
    pub active: bool,
    pub osnap: bool,
    pub smart_track: bool,
    pub anchor: Option<Point3>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportOutput {
    pub picked_point: Option<Point3>,
    pub cancelled: bool,
}

#[derive(Clone, Copy, Debug)]
struct DraftingCursor {
    pointer: Pos2,
    point: Point3,
    object_snap: Option<ObjectSnap>,
    track: Option<OrthogonalTrack>,
}

pub struct Viewport {
    pub display_mode: DisplayMode,
    pixels_per_unit: f32,
    pan: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::Wireframe,
            pixels_per_unit: 40.0,
            pan: Vec2::ZERO,
        }
    }
}

impl Viewport {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        drafting: DraftingInput,
    ) -> ViewportOutput {
        let desired_size = ui.available_size().max(Vec2::splat(1.0));
        let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());
        let rect = response.rect;

        let panning = response.dragged_by(PointerButton::Middle)
            || (!drafting.active && response.dragged_by(PointerButton::Primary));
        if panning {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let zoom = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom != 0.0 {
                self.zoom_by((zoom * 0.002).exp(), response.hover_pos(), rect);
            }
        }

        let drafting_cursor = if drafting.active {
            response
                .hover_pos()
                .and_then(|pointer| self.drafting_cursor(pointer, rect, document, drafting))
        } else {
            None
        };
        if drafting.active && response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }

        painter.rect_filled(rect, 0.0, self.background_color());
        self.paint_grid(&painter, rect);
        self.paint_objects(&painter, rect, document);
        if let Some(cursor) = drafting_cursor {
            self.paint_drafting(&painter, rect, drafting, cursor);
        }
        painter.text(
            rect.left_top() + Vec2::new(10.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "Top · {} · {} object(s)",
                self.display_mode.label(),
                document.objects().len()
            ),
            FontId::proportional(13.0),
            Color32::from_gray(100),
        );

        ViewportOutput {
            picked_point: response
                .clicked_by(PointerButton::Primary)
                .then(|| drafting_cursor.map(|cursor| cursor.point))
                .flatten(),
            cancelled: drafting.active && response.clicked_by(PointerButton::Secondary),
        }
    }

    fn background_color(&self) -> Color32 {
        match self.display_mode {
            DisplayMode::Wireframe => Color32::from_rgb(250, 250, 250),
            DisplayMode::Shaded => Color32::from_rgb(226, 232, 240),
            DisplayMode::Ghosted => Color32::from_rgb(242, 246, 250),
        }
    }

    fn world_origin(&self, rect: Rect) -> Pos2 {
        rect.center() + self.pan
    }

    fn project(&self, point: Point3, rect: Rect) -> Option<Pos2> {
        let origin = self.world_origin(rect);
        let x = f64::from(origin.x) + point.x() * f64::from(self.pixels_per_unit);
        let y = f64::from(origin.y) - point.y() * f64::from(self.pixels_per_unit);
        if !x.is_finite()
            || !y.is_finite()
            || x.abs() > f64::from(f32::MAX)
            || y.abs() > f64::from(f32::MAX)
        {
            return None;
        }
        Some(Pos2::new(x as f32, y as f32))
    }

    fn unproject(&self, position: Pos2, rect: Rect, elevation: Real) -> Option<Point3> {
        let origin = self.world_origin(rect);
        let scale = Real::from(self.pixels_per_unit);
        Point3::try_new(
            Real::from(position.x - origin.x) / scale,
            Real::from(origin.y - position.y) / scale,
            elevation,
        )
        .ok()
    }

    fn zoom_by(&mut self, factor: f32, pointer: Option<Pos2>, rect: Rect) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let old_scale = self.pixels_per_unit;
        let new_scale = (old_scale * factor).clamp(2.0, 2_000.0);
        if new_scale == old_scale {
            return;
        }
        if let Some(pointer) = pointer {
            let old_origin = self.world_origin(rect);
            let ratio = new_scale / old_scale;
            let new_origin = pointer - (pointer - old_origin) * ratio;
            self.pan = new_origin - rect.center();
        }
        self.pixels_per_unit = new_scale;
    }

    fn drafting_cursor(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        input: DraftingInput,
    ) -> Option<DraftingCursor> {
        let elevation = input.anchor.map_or(0.0, Point3::z);
        let raw_point = self.unproject(pointer, rect, elevation)?;
        let object_snap = input
            .osnap
            .then(|| {
                nearest_object_snap(
                    document,
                    raw_point,
                    Real::from(OSNAP_CAPTURE_PIXELS / self.pixels_per_unit),
                )
                .ok()
                .flatten()
            })
            .flatten();
        let track = if object_snap.is_none() && input.smart_track {
            input.anchor.and_then(|anchor| {
                orthogonal_track(
                    raw_point,
                    anchor,
                    Real::from(TRACK_CAPTURE_PIXELS / self.pixels_per_unit),
                )
                .ok()
                .flatten()
            })
        } else {
            None
        };
        let point = object_snap
            .map(ObjectSnap::point)
            .or_else(|| track.map(OrthogonalTrack::point))
            .unwrap_or(raw_point);
        Some(DraftingCursor {
            pointer,
            point,
            object_snap,
            track,
        })
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        let origin = self.world_origin(rect);
        let spacing = self.pixels_per_unit;
        if spacing >= 8.0 {
            let start_x = origin.x - ((origin.x - rect.left()) / spacing).ceil() * spacing;
            let mut x = start_x;
            while x <= rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(1.0, Color32::from_gray(218)),
                );
                x += spacing;
            }
            let start_y = origin.y - ((origin.y - rect.top()) / spacing).ceil() * spacing;
            let mut y = start_y;
            while y <= rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(1.0, Color32::from_gray(218)),
                );
                y += spacing;
            }
        }

        painter.line_segment(
            [
                Pos2::new(rect.left(), origin.y),
                Pos2::new(rect.right(), origin.y),
            ],
            Stroke::new(1.5, Color32::from_rgb(190, 65, 65)),
        );
        painter.line_segment(
            [
                Pos2::new(origin.x, rect.top()),
                Pos2::new(origin.x, rect.bottom()),
            ],
            Stroke::new(1.5, Color32::from_rgb(60, 145, 75)),
        );
    }

    fn paint_objects(&self, painter: &egui::Painter, rect: Rect, document: &Document) {
        for object in document.objects() {
            let attributes = object.attributes();
            let Some(layer) = document.layer(attributes.layer_id()) else {
                continue;
            };
            if !attributes.is_visible() || !layer.is_visible() {
                continue;
            }

            let layer_color = layer.color();
            let mut color = Color32::from_rgb(layer_color.red, layer_color.green, layer_color.blue);
            if self.display_mode == DisplayMode::Ghosted {
                color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
            }
            let width = match self.display_mode {
                DisplayMode::Wireframe => 1.5,
                DisplayMode::Shaded => 2.25,
                DisplayMode::Ghosted => 1.25,
            };

            match object.geometry() {
                Geometry::Point(point) => {
                    if let Some(center) = self.project(*point, rect)
                        && rect.expand(6.0).contains(center)
                    {
                        painter.circle_filled(center, 3.5, color);
                        painter.circle_stroke(center, 5.0, Stroke::new(1.0, color));
                    }
                }
                Geometry::Line(line) => {
                    if let (Some(start), Some(end)) = (
                        self.project(line.start(), rect),
                        self.project(line.end(), rect),
                    ) {
                        painter.line_segment([start, end], Stroke::new(width, color));
                    }
                }
                Geometry::NurbsCurve(curve) => {
                    self.paint_nurbs_curve(painter, rect, curve, Stroke::new(width, color));
                }
                Geometry::Mesh(mesh) => {
                    self.paint_mesh(painter, rect, mesh, color, width);
                }
            }
        }
    }

    fn paint_nurbs_curve(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        curve: &NurbsCurve,
        stroke: Stroke,
    ) {
        const SAMPLES_PER_SPAN: usize = 16;

        let domain_end = *curve.domain().end();
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=SAMPLES_PER_SPAN {
                let fraction = sample as Real / SAMPLES_PER_SPAN as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                // At a fully multiple interior knot, the curve has distinct
                // left and right limits. Keep this span on its left side and
                // begin the next polyline separately on the right side.
                if sample == SAMPLES_PER_SPAN && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }

                let projected = curve
                    .evaluate(parameter)
                    .ok()
                    .and_then(|point| self.project(point, rect));
                if let (Some(start), Some(end)) = (previous, projected) {
                    painter.line_segment([start, end], stroke);
                }
                previous = projected;
            }
        }
    }

    fn paint_mesh(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        mesh: &TriangleMesh,
        color: Color32,
        width: f32,
    ) {
        let edge = Stroke::new(width, color);
        for triangle_index in 0..mesh.triangles().len() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            let projected = points.map(|point| self.project(point, rect));
            let [Some(first), Some(second), Some(third)] = projected else {
                continue;
            };
            let fill = match self.display_mode {
                DisplayMode::Wireframe => Color32::TRANSPARENT,
                DisplayMode::Shaded => mesh.face_normal(triangle_index).map_or_else(
                    |_| blend_toward_white(color, 0.35),
                    |normal| shaded_color(color, normal),
                ),
                DisplayMode::Ghosted => {
                    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 35)
                }
            };
            painter.add(egui::Shape::convex_polygon(
                vec![first, second, third],
                fill,
                edge,
            ));
        }
    }

    fn paint_drafting(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: DraftingInput,
        cursor: DraftingCursor,
    ) {
        const TRACK_COLOR: Color32 = Color32::from_rgb(15, 155, 190);
        const SNAP_COLOR: Color32 = Color32::from_rgb(210, 45, 145);

        if let Some(track) = cursor.track
            && let Some(anchor) = input.anchor.and_then(|point| self.project(point, rect))
        {
            let stroke = Stroke::new(1.0, TRACK_COLOR);
            if matches!(track.axis(), TrackAxis::Horizontal | TrackAxis::Both) {
                painter.extend(egui::Shape::dashed_line(
                    &[
                        Pos2::new(rect.left(), anchor.y),
                        Pos2::new(rect.right(), anchor.y),
                    ],
                    stroke,
                    6.0,
                    4.0,
                ));
            }
            if matches!(track.axis(), TrackAxis::Vertical | TrackAxis::Both) {
                painter.extend(egui::Shape::dashed_line(
                    &[
                        Pos2::new(anchor.x, rect.top()),
                        Pos2::new(anchor.x, rect.bottom()),
                    ],
                    stroke,
                    6.0,
                    4.0,
                ));
            }
        }

        if let Some(anchor) = input.anchor.and_then(|point| self.project(point, rect))
            && let Some(target) = self.project(cursor.point, rect)
        {
            painter.extend(egui::Shape::dashed_line(
                &[anchor, target],
                Stroke::new(1.25, Color32::from_gray(80)),
                7.0,
                4.0,
            ));
        }

        let Some(target) = self.project(cursor.point, rect) else {
            return;
        };
        let marker_color = if cursor.object_snap.is_some() {
            SNAP_COLOR
        } else if cursor.track.is_some() {
            TRACK_COLOR
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
            .or_else(|| cursor.track.map(|track| track.axis().label()));
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

fn blend_toward_white(color: Color32, amount: f32) -> Color32 {
    let blend = |component: u8| {
        (f32::from(component) + (255.0 - f32::from(component)) * amount).round() as u8
    };
    Color32::from_rgb(blend(color.r()), blend(color.g()), blend(color.b()))
}

fn shaded_color(color: Color32, normal: UnitVector3) -> Color32 {
    // A fixed camera-space key light keeps the placeholder top viewport
    // deterministic while making face orientation legible.
    let illumination = normal
        .x()
        .mul_add(-0.35, normal.y().mul_add(-0.45, normal.z() * 0.82))
        .clamp(0.0, 1.0) as f32;
    blend_toward_white(color, 0.35 + 0.55 * illumination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::Geometry;
    use viboceros_geometry::{LineSegment, Tolerance};

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn projection_rejects_values_outside_f32_range() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let point = Point3::try_new(f64::MAX, 0.0, 0.0).unwrap();
        assert_eq!(viewport.project(point, rect), None);
    }

    #[test]
    fn shaded_faces_respond_to_the_fixed_light_direction() {
        let up =
            UnitVector3::try_new(0.0, 0.0, 1.0, viboceros_geometry::Tolerance::DEFAULT).unwrap();
        let down =
            UnitVector3::try_new(0.0, 0.0, -1.0, viboceros_geometry::Tolerance::DEFAULT).unwrap();
        let lit = shaded_color(Color32::BLACK, up);
        let unlit = shaded_color(Color32::BLACK, down);
        assert!(lit.r() > unlit.r());
    }

    #[test]
    fn projection_round_trips_and_zoom_keeps_the_pointer_pinned() {
        let mut viewport = Viewport {
            pan: Vec2::new(17.0, -23.0),
            ..Viewport::default()
        };
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));
        let model = point(3.25, -2.75, 9.0);
        let screen = viewport.project(model, rect).unwrap();
        let round_trip = viewport.unproject(screen, rect, model.z()).unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(round_trip.x(), model.x()));
        assert!(Tolerance::DEFAULT.approx_eq(round_trip.y(), model.y()));
        assert_eq!(round_trip.z(), model.z());

        viewport.zoom_by(2.0, Some(screen), rect);
        let after_zoom = viewport.project(model, rect).unwrap();
        assert!((after_zoom - screen).length() <= f32::EPSILON);
    }

    #[test]
    fn drafting_cursor_prefers_object_snaps_to_tracking() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(0.0, 0.0, 3.0),
                    point(4.0, 0.0, 3.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let pointer = viewport.project(point(0.1, 0.05, 0.0), rect).unwrap();
        let cursor = viewport
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: true,
                    smart_track: true,
                    anchor: Some(point(0.0, 0.0, 8.0)),
                },
            )
            .unwrap();
        assert_eq!(cursor.point, point(0.0, 0.0, 3.0));
        assert!(cursor.object_snap.is_some());
        assert!(cursor.track.is_none());
    }

    #[test]
    fn drafting_cursor_tracks_from_the_previous_pick() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let document = Document::default();
        let anchor = point(0.0, 0.0, 5.0);
        let pointer = viewport.project(point(3.0, 0.1, 5.0), rect).unwrap();
        let cursor = viewport
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: true,
                    smart_track: true,
                    anchor: Some(anchor),
                },
            )
            .unwrap();
        assert_eq!(cursor.point, point(3.0, 0.0, 5.0));
        assert_eq!(cursor.track.unwrap().axis(), TrackAxis::Horizontal);
    }

    #[test]
    fn disabled_drafting_aids_leave_the_cursor_unsnapped() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 4.0)))
            .unwrap();
        let raw = point(0.1, 0.05, 0.0);
        let pointer = viewport.project(raw, rect).unwrap();
        let cursor = viewport
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: false,
                    smart_track: false,
                    anchor: None,
                },
            )
            .unwrap();
        assert!(Tolerance::DEFAULT.approx_eq(cursor.point.x(), raw.x()));
        assert!(Tolerance::DEFAULT.approx_eq(cursor.point.y(), raw.y()));
        assert_eq!(cursor.point.z(), 0.0);
        assert!(cursor.object_snap.is_none());
        assert!(cursor.track.is_none());
    }
}
