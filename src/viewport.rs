use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId, SelectionMode};
use viboceros_drafting::{
    ObjectSnap, OrthogonalTrack, TrackAxis, nearest_object_snap, orthogonal_track,
};
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, Ellipse3, NurbsCurve, NurbsSurface, Point3, Polyline3, Real,
    Tolerance, TriangleMesh, UnitVector3,
};

const OSNAP_CAPTURE_PIXELS: f32 = 12.0;
const TRACK_CAPTURE_PIXELS: f32 = 8.0;
const PICK_CAPTURE_PIXELS: f32 = 8.0;
const CURVE_SAMPLES_PER_SPAN: usize = 16;
const CIRCLE_SAMPLES: usize = 64;
const SURFACE_SAMPLES_PER_SPAN: usize = 8;
const SURFACE_ISOCURVES_PER_SPAN: usize = 2;
const SELECTED_COLOR: Color32 = Color32::from_rgb(255, 145, 0);
const LOCKED_COLOR: Color32 = Color32::from_gray(145);

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
    pub reference: Option<Point3>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportOutput {
    pub picked_point: Option<Point3>,
    pub selection_click: Option<SelectionClick>,
    pub cancelled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionClick {
    pub object_id: Option<ObjectId>,
    pub mode: SelectionMode,
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
        preview_polyline: &[Point3],
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
        let selection_click = if !drafting.active && response.clicked_by(PointerButton::Primary) {
            let mode = ui.input(|input| {
                if input.modifiers.command {
                    SelectionMode::Toggle
                } else if input.modifiers.shift {
                    SelectionMode::Add
                } else {
                    SelectionMode::Replace
                }
            });
            Some(SelectionClick {
                object_id: response
                    .interact_pointer_pos()
                    .and_then(|pointer| self.pick_object(pointer, rect, document)),
                mode,
            })
        } else {
            None
        };

        painter.rect_filled(rect, 0.0, self.background_color());
        self.paint_grid(&painter, rect);
        self.paint_objects(&painter, rect, document);
        if let Some(cursor) = drafting_cursor {
            self.paint_drafting(&painter, rect, drafting, cursor, preview_polyline);
        }
        painter.text(
            rect.left_top() + Vec2::new(10.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "Top · {} · {} object(s) · {} selected",
                self.display_mode.label(),
                document.objects().len(),
                document.selected_object_count(),
            ),
            FontId::proportional(13.0),
            Color32::from_gray(100),
        );

        ViewportOutput {
            picked_point: response
                .clicked_by(PointerButton::Primary)
                .then(|| drafting_cursor.map(|cursor| cursor.point))
                .flatten(),
            selection_click,
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

    fn pick_object(&self, pointer: Pos2, rect: Rect, document: &Document) -> Option<ObjectId> {
        let mut nearest: Option<(u8, f32, ObjectId)> = None;
        for object in document.objects() {
            if !document.is_object_selectable(object.id()) {
                continue;
            }
            let (priority, distance) = match object.geometry() {
                Geometry::Point(point) => {
                    let distance = self
                        .project(*point, rect)
                        .map_or(f32::INFINITY, |projected| {
                            point_segment_distance(pointer, projected, projected)
                        });
                    (0, distance)
                }
                Geometry::PointCloud(cloud) => {
                    let distance = self
                        .unproject(pointer, rect, 0.0)
                        .and_then(|query| {
                            cloud
                                .nearest_xy(
                                    query,
                                    Real::from(PICK_CAPTURE_PIXELS / self.pixels_per_unit),
                                )
                                .ok()
                                .flatten()
                        })
                        .map_or(f32::INFINITY, |(_, _, distance)| {
                            distance as f32 * self.pixels_per_unit
                        });
                    (0, distance)
                }
                Geometry::Line(line) => {
                    let distance = self
                        .project(line.start(), rect)
                        .zip(self.project(line.end(), rect))
                        .map_or(f32::INFINITY, |(start, end)| {
                            point_segment_distance(pointer, start, end)
                        });
                    (1, distance)
                }
                Geometry::Circle(circle) => (1, self.circle_pick_distance(pointer, rect, circle)),
                Geometry::Arc(arc) => (1, self.arc_pick_distance(pointer, rect, arc)),
                Geometry::Ellipse(ellipse) => {
                    (1, self.ellipse_pick_distance(pointer, rect, ellipse))
                }
                Geometry::Polyline(polyline) => {
                    (1, self.polyline_pick_distance(pointer, rect, polyline))
                }
                Geometry::NurbsCurve(curve) => (1, self.nurbs_pick_distance(pointer, rect, curve)),
                Geometry::NurbsSurface(surface) => (
                    2,
                    self.nurbs_surface_pick_distance(pointer, rect, surface, document.tolerance()),
                ),
                Geometry::Brep(brep) => (
                    2,
                    self.brep_pick_distance(pointer, rect, brep, document.tolerance()),
                ),
                Geometry::Mesh(mesh) => (2, self.mesh_pick_distance(pointer, rect, mesh)),
            };
            if distance > PICK_CAPTURE_PIXELS {
                continue;
            }
            if nearest.is_none_or(|(best_priority, best_distance, _)| {
                distance < best_distance || (distance == best_distance && priority < best_priority)
            }) {
                nearest = Some((priority, distance, object.id()));
            }
        }
        nearest.map(|(_, _, id)| id)
    }

    fn nurbs_pick_distance(&self, pointer: Pos2, rect: Rect, curve: &NurbsCurve) -> f32 {
        let domain_end = *curve.domain().end();
        let mut nearest = f32::INFINITY;
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=CURVE_SAMPLES_PER_SPAN {
                let fraction = sample as Real / CURVE_SAMPLES_PER_SPAN as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                if sample == CURVE_SAMPLES_PER_SPAN && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }
                let projected = curve
                    .evaluate(parameter)
                    .ok()
                    .and_then(|point| self.project(point, rect));
                if let (Some(start), Some(end)) = (previous, projected) {
                    nearest = nearest.min(point_segment_distance(pointer, start, end));
                }
                previous = projected;
            }
        }
        nearest
    }

    fn circle_pick_distance(&self, pointer: Pos2, rect: Rect, circle: &Circle3) -> f32 {
        self.parametric_pick_distance(pointer, rect, CIRCLE_SAMPLES, |parameter| {
            circle.point_at_angle(std::f64::consts::TAU * parameter)
        })
    }

    fn arc_pick_distance(&self, pointer: Pos2, rect: Rect, arc: &CircularArc3) -> f32 {
        let samples = circular_arc_samples(*arc);
        self.parametric_pick_distance(pointer, rect, samples, |parameter| arc.point_at(parameter))
    }

    fn ellipse_pick_distance(&self, pointer: Pos2, rect: Rect, ellipse: &Ellipse3) -> f32 {
        self.parametric_pick_distance(pointer, rect, CIRCLE_SAMPLES, |parameter| {
            ellipse.point_at_angle(std::f64::consts::TAU * parameter)
        })
    }

    fn parametric_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        samples: usize,
        mut evaluate: impl FnMut(Real) -> Result<Point3, viboceros_geometry::GeometryError>,
    ) -> f32 {
        let mut nearest = f32::INFINITY;
        let mut previous = None;
        for sample in 0..=samples {
            let projected = evaluate(sample as Real / samples as Real)
                .ok()
                .and_then(|point| self.project(point, rect));
            if let (Some(start), Some(end)) = (previous, projected) {
                nearest = nearest.min(point_segment_distance(pointer, start, end));
            }
            previous = projected;
        }
        nearest
    }

    fn polyline_pick_distance(&self, pointer: Pos2, rect: Rect, polyline: &Polyline3) -> f32 {
        polyline
            .segments()
            .filter_map(|segment| {
                self.project(segment.start(), rect)
                    .zip(self.project(segment.end(), rect))
            })
            .map(|(start, end)| point_segment_distance(pointer, start, end))
            .fold(f32::INFINITY, f32::min)
    }

    fn mesh_pick_distance(&self, pointer: Pos2, rect: Rect, mesh: &TriangleMesh) -> f32 {
        let mut nearest = f32::INFINITY;
        for triangle_index in 0..mesh.triangles().len() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            let [Some(first), Some(second), Some(third)] =
                points.map(|point| self.project(point, rect))
            else {
                continue;
            };
            if self.display_mode != DisplayMode::Wireframe
                && point_in_triangle(pointer, first, second, third)
            {
                return 0.0;
            }
            nearest = nearest
                .min(point_segment_distance(pointer, first, second))
                .min(point_segment_distance(pointer, second, third))
                .min(point_segment_distance(pointer, third, first));
        }
        nearest
    }

    fn nurbs_surface_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> f32 {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick_distance(pointer, rect, &mesh);
        }
        let mut nearest = f32::INFINITY;
        self.for_each_surface_grid_segment(rect, surface, |start, end| {
            nearest = nearest.min(point_segment_distance(pointer, start, end));
        });
        nearest
    }

    fn brep_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        brep: &Brep,
        tolerance: Tolerance,
    ) -> f32 {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick_distance(pointer, rect, &mesh);
        }
        brep.edges()
            .iter()
            .map(|edge| self.nurbs_pick_distance(pointer, rect, edge.curve()))
            .fold(f32::INFINITY, f32::min)
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

            let display_color = resolved_display_color(attributes, layer.color());
            let mut color = if attributes.is_locked() || layer.is_locked() {
                LOCKED_COLOR
            } else {
                display_color
            };
            if self.display_mode == DisplayMode::Ghosted {
                color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
            }
            let selected = document.is_selected(object.id());
            if selected {
                color = SELECTED_COLOR;
            }
            let mut width = match self.display_mode {
                DisplayMode::Wireframe => 1.5,
                DisplayMode::Shaded => 2.25,
                DisplayMode::Ghosted => 1.25,
            };
            if selected {
                width += 1.5;
            }

            match object.geometry() {
                Geometry::Point(point) => {
                    if let Some(center) = self.project(*point, rect)
                        && rect.expand(6.0).contains(center)
                    {
                        painter.circle_filled(center, 3.5, color);
                        painter.circle_stroke(center, 5.0, Stroke::new(1.0, color));
                    }
                }
                Geometry::PointCloud(cloud) => {
                    let radius = if selected { 3.5 } else { 2.5 };
                    for point in cloud.points() {
                        if let Some(center) = self.project(*point, rect)
                            && rect.expand(radius).contains(center)
                        {
                            painter.circle_filled(center, radius, color);
                        }
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
                Geometry::Circle(circle) => {
                    self.paint_parametric_curve(
                        painter,
                        rect,
                        CIRCLE_SAMPLES,
                        Stroke::new(width, color),
                        |parameter| circle.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Arc(arc) => {
                    self.paint_parametric_curve(
                        painter,
                        rect,
                        circular_arc_samples(*arc),
                        Stroke::new(width, color),
                        |parameter| arc.point_at(parameter),
                    );
                }
                Geometry::Ellipse(ellipse) => {
                    self.paint_parametric_curve(
                        painter,
                        rect,
                        CIRCLE_SAMPLES,
                        Stroke::new(width, color),
                        |parameter| ellipse.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Polyline(polyline) => {
                    for segment in polyline.segments() {
                        if let (Some(start), Some(end)) = (
                            self.project(segment.start(), rect),
                            self.project(segment.end(), rect),
                        ) {
                            painter.line_segment([start, end], Stroke::new(width, color));
                        }
                    }
                }
                Geometry::NurbsCurve(curve) => {
                    self.paint_nurbs_curve(painter, rect, curve, Stroke::new(width, color));
                }
                Geometry::NurbsSurface(surface) => {
                    self.paint_nurbs_surface(
                        painter,
                        rect,
                        surface,
                        color,
                        width,
                        document.tolerance(),
                    );
                }
                Geometry::Brep(brep) => {
                    self.paint_brep(painter, rect, brep, color, width, document.tolerance());
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
        let domain_end = *curve.domain().end();
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=CURVE_SAMPLES_PER_SPAN {
                let fraction = sample as Real / CURVE_SAMPLES_PER_SPAN as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                // At a fully multiple interior knot, the curve has distinct
                // left and right limits. Keep this span on its left side and
                // begin the next polyline separately on the right side.
                if sample == CURVE_SAMPLES_PER_SPAN && span_end < domain_end {
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

    fn paint_parametric_curve(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        samples: usize,
        stroke: Stroke,
        mut evaluate: impl FnMut(Real) -> Result<Point3, viboceros_geometry::GeometryError>,
    ) {
        let mut previous = None;
        for sample in 0..=samples {
            let projected = evaluate(sample as Real / samples as Real)
                .ok()
                .and_then(|point| self.project(point, rect));
            if let (Some(start), Some(end)) = (previous, projected) {
                painter.line_segment([start, end], stroke);
            }
            previous = projected;
        }
    }

    fn paint_nurbs_surface(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        surface: &NurbsSurface,
        color: Color32,
        width: f32,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            for triangle_index in 0..mesh.triangles().len() {
                let Some(points) = mesh.triangle_points(triangle_index) else {
                    continue;
                };
                let [Some(first), Some(second), Some(third)] =
                    points.map(|point| self.project(point, rect))
                else {
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
                    Stroke::NONE,
                ));
            }
        }

        let stroke = Stroke::new(width, color);
        self.for_each_surface_grid_segment(rect, surface, |start, end| {
            painter.line_segment([start, end], stroke);
        });
    }

    fn paint_brep(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        brep: &Brep,
        color: Color32,
        width: f32,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            for triangle_index in 0..mesh.triangles().len() {
                let Some(points) = mesh.triangle_points(triangle_index) else {
                    continue;
                };
                let [Some(first), Some(second), Some(third)] =
                    points.map(|point| self.project(point, rect))
                else {
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
                    Stroke::NONE,
                ));
            }
        }
        let stroke = Stroke::new(width, color);
        for edge in brep.edges() {
            self.paint_nurbs_curve(painter, rect, edge.curve(), stroke);
        }
    }

    fn for_each_surface_grid_segment(
        &self,
        rect: Rect,
        surface: &NurbsSurface,
        mut visit: impl FnMut(Pos2, Pos2),
    ) {
        let spans_u = surface.spans_u().collect::<Vec<_>>();
        let spans_v = surface.spans_v().collect::<Vec<_>>();
        let domain_u_end = *surface.domain_u().end();
        let domain_v_end = *surface.domain_v().end();

        for &(u_start, u_end) in &spans_u {
            for iso_sample in 0..=SURFACE_ISOCURVES_PER_SPAN {
                let u = sampled_span_parameter(
                    u_start,
                    u_end,
                    iso_sample,
                    SURFACE_ISOCURVES_PER_SPAN,
                    domain_u_end,
                );
                for &(v_start, v_end) in &spans_v {
                    let mut previous = None;
                    for sample in 0..=CURVE_SAMPLES_PER_SPAN {
                        let v = sampled_span_parameter(
                            v_start,
                            v_end,
                            sample,
                            CURVE_SAMPLES_PER_SPAN,
                            domain_v_end,
                        );
                        let projected = surface
                            .evaluate(u, v)
                            .ok()
                            .and_then(|point| self.project(point, rect));
                        if let (Some(start), Some(end)) = (previous, projected) {
                            visit(start, end);
                        }
                        previous = projected;
                    }
                }
            }
        }

        for &(v_start, v_end) in &spans_v {
            for iso_sample in 0..=SURFACE_ISOCURVES_PER_SPAN {
                let v = sampled_span_parameter(
                    v_start,
                    v_end,
                    iso_sample,
                    SURFACE_ISOCURVES_PER_SPAN,
                    domain_v_end,
                );
                for &(u_start, u_end) in &spans_u {
                    let mut previous = None;
                    for sample in 0..=CURVE_SAMPLES_PER_SPAN {
                        let u = sampled_span_parameter(
                            u_start,
                            u_end,
                            sample,
                            CURVE_SAMPLES_PER_SPAN,
                            domain_u_end,
                        );
                        let projected = surface
                            .evaluate(u, v)
                            .ok()
                            .and_then(|point| self.project(point, rect));
                        if let (Some(start), Some(end)) = (previous, projected) {
                            visit(start, end);
                        }
                        previous = projected;
                    }
                }
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
        preview_polyline: &[Point3],
    ) {
        const TRACK_COLOR: Color32 = Color32::from_rgb(15, 155, 190);
        const SNAP_COLOR: Color32 = Color32::from_rgb(210, 45, 145);

        for vertices in preview_polyline.windows(2) {
            if let (Some(start), Some(end)) = (
                self.project(vertices[0], rect),
                self.project(vertices[1], rect),
            ) {
                painter.line_segment([start, end], Stroke::new(1.5, Color32::from_gray(80)));
            }
        }

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

        if let (Some(anchor), Some(reference)) = (
            input.anchor.and_then(|point| self.project(point, rect)),
            input.reference.and_then(|point| self.project(point, rect)),
        ) {
            const REFERENCE_COLOR: Color32 = Color32::from_rgb(125, 80, 180);
            painter.extend(egui::Shape::dashed_line(
                &[anchor, reference],
                Stroke::new(1.5, REFERENCE_COLOR),
                5.0,
                3.0,
            ));
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

fn sampled_span_parameter(
    start: Real,
    end: Real,
    sample: usize,
    sample_count: usize,
    domain_end: Real,
) -> Real {
    let fraction = sample as Real / sample_count as Real;
    let parameter = start.mul_add(1.0 - fraction, end * fraction);
    if sample == sample_count && end < domain_end {
        parameter.next_down().max(start)
    } else {
        parameter
    }
}

fn circular_arc_samples(arc: CircularArc3) -> usize {
    ((arc.sweep_radians() / std::f64::consts::TAU * CIRCLE_SAMPLES as Real).ceil() as usize).max(2)
}

fn point_segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let start_x = f64::from(start.x);
    let start_y = f64::from(start.y);
    let delta_x = f64::from(end.x) - start_x;
    let delta_y = f64::from(end.y) - start_y;
    let length_squared = delta_x.mul_add(delta_x, delta_y * delta_y);
    let parameter = if length_squared > 0.0 && length_squared.is_finite() {
        ((f64::from(point.x) - start_x).mul_add(delta_x, (f64::from(point.y) - start_y) * delta_y)
            / length_squared)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest_x = delta_x.mul_add(parameter, start_x);
    let closest_y = delta_y.mul_add(parameter, start_y);
    let distance = (f64::from(point.x) - closest_x).hypot(f64::from(point.y) - closest_y);
    if distance.is_finite() && distance <= f64::from(f32::MAX) {
        distance as f32
    } else {
        f32::INFINITY
    }
}

fn point_in_triangle(point: Pos2, first: Pos2, second: Pos2, third: Pos2) -> bool {
    let signed_area = |start: Pos2, end: Pos2, target: Pos2| {
        (f64::from(end.x) - f64::from(start.x)).mul_add(
            f64::from(target.y) - f64::from(start.y),
            -(f64::from(end.y) - f64::from(start.y)) * (f64::from(target.x) - f64::from(start.x)),
        )
    };
    let area = signed_area(first, second, third);
    if !area.is_finite() || area.abs() <= f64::EPSILON {
        return false;
    }
    let tolerance = area.abs().max(1.0) * 1.0e-12;
    let signs = [
        signed_area(first, second, point),
        signed_area(second, third, point),
        signed_area(third, first, point),
    ];
    signs.iter().all(|value| *value >= -tolerance) || signs.iter().all(|value| *value <= tolerance)
}

fn blend_toward_white(color: Color32, amount: f32) -> Color32 {
    let blend = |component: u8| {
        (f32::from(component) + (255.0 - f32::from(component)) * amount).round() as u8
    };
    Color32::from_rgb(blend(color.r()), blend(color.g()), blend(color.b()))
}

fn resolved_display_color(attributes: &ObjectAttributes, layer_color: ColorRgb) -> Color32 {
    let color = attributes.display_color(layer_color);
    Color32::from_rgb(color.red, color.green, color.blue)
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
    use viboceros_document::{ColorRgb, Geometry};
    use viboceros_geometry::{
        Circle3, CircularArc3, Ellipse3, Frame3, LineSegment, NurbsCurve, NurbsSurface,
        PointCloud3, Polyline3, Tolerance, TriangleMesh, UnitVector3, Vector3,
    };

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
    fn object_color_overrides_layer_color_only_for_object_source() {
        let object = ColorRgb::new(12, 34, 56);
        let layer = ColorRgb::new(78, 90, 123);
        let document = Document::default();
        let base =
            ObjectAttributes::on_layer(document.current_layer_id()).with_object_color(object);
        assert_eq!(
            resolved_display_color(&base, layer),
            Color32::from_rgb(12, 34, 56)
        );
        for source in [
            viboceros_document::ObjectColorSource::Layer,
            viboceros_document::ObjectColorSource::Material,
            viboceros_document::ObjectColorSource::Parent,
        ] {
            assert_eq!(
                resolved_display_color(&base.clone().with_color_source(source), layer),
                Color32::from_rgb(78, 90, 123)
            );
        }
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
    fn picking_supports_points_point_clouds_lines_and_nurbs_curves() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let point_id = document
            .add_geometry(Geometry::Point(point(-4.0, 0.0, 0.0)))
            .unwrap();
        let line_id = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(-1.0, 0.0, 0.0),
                    point(1.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let curve_id = document
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(
                    1,
                    vec![point(3.0, 0.0, 0.0), point(5.0, 1.0, 0.0)],
                )
                .unwrap(),
            ))
            .unwrap();
        let cloud_id = document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![point(-5.0, 4.0, 8.0), point(2.0, 4.0, -3.0)]).unwrap(),
            ))
            .unwrap();
        document
            .add_geometry(Geometry::Point(point(0.0, 0.15, 0.0)))
            .unwrap();

        let near_point = viewport.project(point(-3.9, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(near_point, rect, &document),
            Some(point_id)
        );
        let near_line = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(near_line, rect, &document),
            Some(line_id)
        );
        let on_curve = viewport.project(point(4.0, 0.5, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(on_curve, rect, &document),
            Some(curve_id)
        );
        let near_cloud_point = viewport.project(point(2.1, 4.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(near_cloud_point, rect, &document),
            Some(cloud_id)
        );
    }

    #[test]
    fn picking_supports_analytic_circles_arcs_and_ellipses() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let normal = UnitVector3::try_new(0.0, 0.0, 1.0, Tolerance::DEFAULT).unwrap();
        let circle_id = document
            .add_geometry(Geometry::Circle(
                Circle3::try_new(point(-3.0, 0.0, 0.0), 1.0, normal, Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let arc_id = document
            .add_geometry(Geometry::Arc(
                CircularArc3::try_from_three_points(
                    point(2.0, 0.0, 0.0),
                    point(3.0, 1.0, 0.0),
                    point(4.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let ellipse_id = document
            .add_geometry(Geometry::Ellipse(
                Ellipse3::try_from_three_points(
                    point(0.0, -3.0, 0.0),
                    point(2.0, -3.0, 0.0),
                    point(0.0, -2.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();

        let on_circle = viewport.project(point(-2.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(on_circle, rect, &document),
            Some(circle_id)
        );
        let on_arc = viewport.project(point(3.0, 1.0, 0.0), rect).unwrap();
        assert_eq!(viewport.pick_object(on_arc, rect, &document), Some(arc_id));
        let on_ellipse = viewport.project(point(2.0, -3.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(on_ellipse, rect, &document),
            Some(ellipse_id)
        );
        let inside_circle = viewport.project(point(-3.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(viewport.pick_object(inside_circle, rect, &document), None);
    }

    #[test]
    fn picking_supports_polyline_segments_without_filling_closed_regions() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Polyline(
                Polyline3::try_new(
                    vec![
                        point(-2.0, -2.0, 0.0),
                        point(2.0, -2.0, 0.0),
                        point(2.0, 2.0, 0.0),
                        point(-2.0, 2.0, 0.0),
                        point(-2.0, -2.0, 0.0),
                    ],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let viewport = Viewport::default();
        let edge = viewport.project(point(2.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(viewport.pick_object(edge, rect, &document), Some(id));
        let center = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(viewport.pick_object(center, rect, &document), None);
    }

    #[test]
    fn shaded_meshes_pick_by_face_while_wireframe_picks_edges() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let mesh_id = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![
                        point(-2.0, -2.0, 0.0),
                        point(2.0, -2.0, 0.0),
                        point(0.0, 2.0, 0.0),
                    ],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let center = Viewport::default()
            .project(point(0.0, 0.0, 0.0), rect)
            .unwrap();

        let wireframe = Viewport::default();
        assert_eq!(wireframe.pick_object(center, rect, &document), None);
        let shaded = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        assert_eq!(shaded.pick_object(center, rect, &document), Some(mesh_id));
    }

    #[test]
    fn shaded_nurbs_surfaces_pick_from_their_display_tessellation() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let surface_id = document
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    point(-2.0, -2.0, 0.0),
                    point(2.0, -2.0, 0.0),
                    point(2.0, 2.0, 1.0),
                    point(-2.0, 2.0, 1.0),
                ])
                .unwrap(),
            ))
            .unwrap();
        let viewport = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        let center = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(center, rect, &document),
            Some(surface_id)
        );
        let off_isocurve = viewport.project(point(1.0, 1.0, 0.0), rect).unwrap();
        assert_eq!(
            Viewport::default().pick_object(off_isocurve, rect, &document),
            None
        );
        assert_eq!(
            viewport.pick_object(off_isocurve, rect, &document),
            Some(surface_id)
        );
    }

    #[test]
    fn shaded_breps_pick_faces_while_wireframe_uses_exact_edges() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            point(0.0, 0.0, 0.0),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let brep_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_box(
                    frame,
                    [[-2.0, 2.0], [-2.0, 2.0], [0.0, 3.0]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let center = Viewport::default()
            .project(point(0.0, 0.0, 0.0), rect)
            .unwrap();

        assert_eq!(
            Viewport::default().pick_object(center, rect, &document),
            None
        );
        let shaded = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        assert_eq!(shaded.pick_object(center, rect, &document), Some(brep_id));
    }

    #[test]
    fn shaded_trimmed_brep_respects_a_concave_cap_boundary() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let profile = Polyline3::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 1.0, 0.0),
                point(1.0, 1.0, 0.0),
                point(1.0, 3.0, 0.0),
                point(0.0, 3.0, 0.0),
                point(0.0, 0.0, 0.0),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let brep_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_extruded_curve(
                    &profile,
                    Vector3::try_new(0.0, 0.0, 0.0).unwrap(),
                    Vector3::try_new(0.0, 0.0, 4.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let shaded = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        let inside = shaded.project(point(0.5, 2.0, 0.0), rect).unwrap();
        let outside_notch = shaded.project(point(2.0, 2.0, 0.0), rect).unwrap();
        assert_eq!(shaded.pick_object(inside, rect, &document), Some(brep_id));
        assert_eq!(shaded.pick_object(outside_notch, rect, &document), None);
    }

    #[test]
    fn picking_ignores_locked_or_hidden_objects_and_prefers_point_features() {
        let viewport = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let mesh_id = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(
                    vec![
                        point(-2.0, -2.0, 0.0),
                        point(2.0, -2.0, 0.0),
                        point(0.0, 2.0, 0.0),
                    ],
                    vec![[0, 1, 2]],
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let point_id = document
            .add_geometry(Geometry::Point(point(0.0, 0.0, 0.0)))
            .unwrap();
        let locked_layer = document
            .add_layer("Locked", ColorRgb::new(1, 2, 3))
            .unwrap();
        document.set_current_layer(locked_layer).unwrap();
        document
            .add_geometry(Geometry::Point(point(4.0, 0.0, 0.0)))
            .unwrap();
        let default = document.layer_by_name("Default").unwrap().id();
        document.set_current_layer(default).unwrap();
        document.set_layer_locked(locked_layer, true).unwrap();

        let center = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            viewport.pick_object(center, rect, &document),
            Some(point_id)
        );
        document.set_objects_locked([point_id], true).unwrap();
        assert_eq!(viewport.pick_object(center, rect, &document), Some(mesh_id));
        document.set_objects_locked([point_id], false).unwrap();
        document.set_objects_visibility([point_id], false).unwrap();
        assert_eq!(viewport.pick_object(center, rect, &document), Some(mesh_id));
        let locked = viewport.project(point(4.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(viewport.pick_object(locked, rect, &document), None);
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
                    reference: None,
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
                    reference: None,
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
                    reference: None,
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
