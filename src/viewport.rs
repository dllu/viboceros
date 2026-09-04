use std::collections::HashMap;

use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Stroke, Vec2,
};
use nalgebra::{Matrix4 as NaMatrix4, Vector3 as NaVector3};
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, ObjectId, SelectionMode};
use viboceros_drafting::{
    ObjectSnap, OrthogonalTrack, TrackAxis, nearest_object_snap, nearest_object_snap_projected,
    orthogonal_track,
};
use viboceros_geometry::{
    Brep, Circle3, CircularArc3, Ellipse3, NurbsCurve, NurbsSurface, Point3, Polyline3, Real,
    Tolerance, TriangleMesh, Vector3,
};

use crate::viewport_gpu::{
    LineInstance as GpuLineInstance, PointInstance as GpuPointInstance,
    TriangleVertex as GpuTriangleVertex, ViewUniform as GpuViewUniform,
    ViewportScene as GpuViewportScene,
};

const OSNAP_CAPTURE_PIXELS: f32 = 12.0;
const TRACK_CAPTURE_PIXELS: f32 = 8.0;
const PICK_CAPTURE_PIXELS: f32 = 8.0;
const CURVE_SAMPLES_PER_SPAN: usize = 16;
const CIRCLE_SAMPLES: usize = 64;
const SURFACE_SAMPLES_PER_SPAN: usize = 8;
const SELECTED_COLOR: Color32 = Color32::from_rgb(255, 145, 0);
const LOCKED_COLOR: Color32 = Color32::from_gray(145);
const GRID_SPACING: Real = 1.0;
const DEFAULT_PERSPECTIVE_CAMERA_DISTANCE: Real = 50.0;
const MIN_PERSPECTIVE_CAMERA_DISTANCE: Real = 0.01;
const MAX_PERSPECTIVE_CAMERA_DISTANCE: Real = 1.0e9;
const PERSPECTIVE_VERTICAL_FOV_RADIANS: Real = 35.0 * std::f64::consts::PI / 180.0;
const SMOOTH_SHADING_COSINE: Real = std::f64::consts::FRAC_1_SQRT_2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Top,
    Perspective,
    Front,
    Right,
}

impl ViewKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Perspective => "Perspective",
            Self::Front => "Front",
            Self::Right => "Right",
        }
    }

    const fn is_parallel(self) -> bool {
        !matches!(self, Self::Perspective)
    }
}

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
    pub grid_snap: bool,
    pub anchor: Option<Point3>,
    pub reference: Option<Point3>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewportOutput {
    pub picked_point: Option<Point3>,
    pub selection_click: Option<SelectionClick>,
    pub selection_window: Option<SelectionWindow>,
    pub enter_pressed: bool,
    pub activated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionClick {
    pub object_id: Option<ObjectId>,
    pub mode: SelectionMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectionWindow {
    pub object_ids: Vec<ObjectId>,
    pub mode: SelectionMode,
    pub crossing: bool,
}

#[derive(Clone, Copy, Debug)]
struct DraftingCursor {
    pointer: Pos2,
    point: Point3,
    object_snap: Option<ObjectSnap>,
    track: Option<OrthogonalTrack>,
    grid_snapped: bool,
}

#[derive(Clone, Copy, Debug)]
struct SurfaceDisplayStyle {
    color: Color32,
    width: f32,
    wire_density: i32,
}

#[derive(Clone, Copy)]
struct DepthTriangle {
    depth: Real,
    vertices: [GpuTriangleVertex; 3],
}

struct GpuSceneBuilder {
    triangles: Vec<DepthTriangle>,
    lines: Vec<GpuLineInstance>,
    points: Vec<GpuPointInstance>,
    min_depth: Real,
    max_depth: Real,
}

impl GpuSceneBuilder {
    fn new() -> Self {
        Self {
            triangles: Vec::new(),
            lines: Vec::new(),
            points: Vec::new(),
            min_depth: Real::INFINITY,
            max_depth: Real::NEG_INFINITY,
        }
    }

    fn include_depth(&mut self, depth: Real) {
        if depth.is_finite() {
            self.min_depth = self.min_depth.min(depth);
            self.max_depth = self.max_depth.max(depth);
        }
    }

    fn depth_range(&self) -> Option<(Real, Real)> {
        (self.min_depth.is_finite() && self.max_depth.is_finite())
            .then_some((self.min_depth, self.max_depth))
    }

    fn finish(mut self, uniform: GpuViewUniform, transparent: bool) -> GpuViewportScene {
        if transparent {
            self.triangles
                .sort_by(|left, right| right.depth.total_cmp(&left.depth));
        }
        GpuViewportScene {
            uniform,
            triangles: self
                .triangles
                .into_iter()
                .flat_map(|triangle| triangle.vertices)
                .collect(),
            lines: self.lines,
            points: self.points,
            transparent,
        }
    }
}

#[derive(Default)]
struct ProjectedPrimitives {
    points: Vec<Pos2>,
    segments: Vec<[Pos2; 2]>,
    triangles: Vec<[Pos2; 3]>,
}

impl ProjectedPrimitives {
    fn add_point(&mut self, point: Option<Pos2>) {
        if let Some(point) = point {
            self.points.push(point);
        }
    }

    fn add_segment(&mut self, start: Option<Pos2>, end: Option<Pos2>) {
        if let (Some(start), Some(end)) = (start, end) {
            self.points.extend([start, end]);
            self.segments.push([start, end]);
        }
    }

    fn add_triangle(&mut self, vertices: [Option<Pos2>; 3]) {
        let [Some(first), Some(second), Some(third)] = vertices else {
            return;
        };
        self.points.extend([first, second, third]);
        self.segments
            .extend([[first, second], [second, third], [third, first]]);
        self.triangles.push([first, second, third]);
    }

    fn is_windowed_by(&self, selection: Rect) -> bool {
        !self.points.is_empty() && self.points.iter().all(|point| selection.contains(*point))
    }

    fn is_crossed_by(&self, selection: Rect) -> bool {
        self.points.iter().any(|point| selection.contains(*point))
            || self
                .segments
                .iter()
                .any(|[start, end]| segment_intersects_rect(*start, *end, selection))
            || self.triangles.iter().any(|triangle| {
                rect_corners(selection)
                    .iter()
                    .any(|corner| point_in_triangle(*corner, triangle[0], triangle[1], triangle[2]))
            })
    }
}

pub struct Viewport {
    pub kind: ViewKind,
    pub display_mode: DisplayMode,
    pixels_per_unit: f32,
    pan: Vec2,
    orbit_yaw: Real,
    orbit_pitch: Real,
    perspective_camera_distance: Real,
    selection_drag_start: Option<Pos2>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(ViewKind::Top)
    }
}

impl Viewport {
    pub fn new(kind: ViewKind) -> Self {
        Self {
            kind,
            display_mode: DisplayMode::Wireframe,
            pixels_per_unit: 40.0,
            pan: Vec2::ZERO,
            orbit_yaw: -std::f64::consts::FRAC_PI_4,
            orbit_pitch: std::f64::consts::FRAC_PI_6,
            perspective_camera_distance: DEFAULT_PERSPECTIVE_CAMERA_DISTANCE,
            selection_drag_start: None,
        }
    }

    pub(crate) fn apparent_intersection_normal(&self) -> Vector3 {
        let direction = match self.kind {
            ViewKind::Top => [0.0, 0.0, 1.0],
            ViewKind::Front => [0.0, 1.0, 0.0],
            ViewKind::Right => [1.0, 0.0, 0.0],
            ViewKind::Perspective => {
                let (_, _, forward) = self.perspective_basis();
                [forward.x, forward.y, forward.z]
            }
        };
        Vector3::try_from(direction).expect("viewport directions are finite")
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        drafting: DraftingInput,
        preview_polyline: &[Point3],
        viewport_index: usize,
        active: bool,
    ) -> ViewportOutput {
        let desired_size = ui.available_size().max(Vec2::splat(1.0));
        let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());
        let rect = response.rect;

        let modifiers = ui.input(|input| input.modifiers);
        if response.dragged_by(PointerButton::Middle) {
            self.apply_navigation_drag(PointerButton::Middle, modifiers, response.drag_delta());
        } else if response.dragged_by(PointerButton::Secondary) {
            self.apply_navigation_drag(PointerButton::Secondary, modifiers, response.drag_delta());
        }
        let mut zoomed = false;
        if response.hovered() {
            let zoom = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom != 0.0 {
                self.zoom_by((zoom * 0.002).exp(), response.hover_pos(), rect);
                zoomed = true;
            }
        }

        if drafting.active {
            self.selection_drag_start = None;
        } else if response.drag_started_by(PointerButton::Primary) {
            self.selection_drag_start = ui.input(|input| input.pointer.press_origin());
        }
        let selection_pointer = response.interact_pointer_pos();
        let selection_window = if !drafting.active
            && response.drag_stopped_by(PointerButton::Primary)
        {
            self.selection_drag_start.take().and_then(|start| {
                let end = selection_pointer?;
                let crossing = is_crossing_selection(start, end);
                let selection_rect = Rect::from_two_pos(start, end);
                Some(SelectionWindow {
                    object_ids: self.objects_in_selection(rect, selection_rect, crossing, document),
                    mode: selection_mode(modifiers),
                    crossing,
                })
            })
        } else {
            None
        };

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
            Some(SelectionClick {
                object_id: response
                    .interact_pointer_pos()
                    .and_then(|pointer| self.pick_object(pointer, rect, document)),
                mode: selection_mode(modifiers),
            })
        } else {
            None
        };

        painter.rect_filled(rect, 0.0, self.background_color());
        self.paint_grid(&painter, rect);
        self.paint_objects(&painter, rect, document, viewport_index);
        if let Some(cursor) = drafting_cursor {
            self.paint_drafting(&painter, rect, drafting, cursor, preview_polyline);
        }
        if let (Some(start), Some(end)) = (self.selection_drag_start, selection_pointer) {
            self.paint_selection_window(&painter, start, end);
        }
        painter.rect_stroke(
            rect.shrink(0.5),
            0.0,
            Stroke::new(
                if active { 2.0 } else { 1.0 },
                if active {
                    Color32::from_rgb(35, 115, 210)
                } else {
                    Color32::from_gray(155)
                },
            ),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.left_top() + Vec2::new(10.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "{} · {} · {} object(s) · {} selected",
                self.kind.label(),
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
            selection_window,
            enter_pressed: response.clicked_by(PointerButton::Secondary),
            activated: response.clicked_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Secondary)
                || response.clicked_by(PointerButton::Middle)
                || response.dragged_by(PointerButton::Primary)
                || response.dragged_by(PointerButton::Secondary)
                || response.dragged_by(PointerButton::Middle)
                || response.drag_stopped_by(PointerButton::Primary)
                || response.drag_stopped_by(PointerButton::Secondary)
                || response.drag_stopped_by(PointerButton::Middle)
                || zoomed,
        }
    }

    fn apply_navigation_drag(
        &mut self,
        button: PointerButton,
        modifiers: egui::Modifiers,
        delta: Vec2,
    ) {
        if button == PointerButton::Middle
            || (button == PointerButton::Secondary && (self.kind.is_parallel() || modifiers.shift))
        {
            self.pan += delta;
        } else if button == PointerButton::Secondary {
            self.orbit_yaw -= Real::from(delta.x) * 0.01;
            self.orbit_pitch = (self.orbit_pitch + Real::from(delta.y) * 0.01)
                .clamp(-1.553_343_034_274_953_2, 1.553_343_034_274_953_2);
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
        let (horizontal_pixels, vertical_pixels) = match self.kind {
            ViewKind::Top => (
                point.x() * f64::from(self.pixels_per_unit),
                point.y() * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Front => (
                point.x() * f64::from(self.pixels_per_unit),
                point.z() * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Right => (
                point.y() * f64::from(self.pixels_per_unit),
                point.z() * f64::from(self.pixels_per_unit),
            ),
            ViewKind::Perspective => {
                let (right, up, forward) = self.perspective_basis();
                let camera = -forward * self.perspective_camera_distance;
                let relative = NaVector3::new(point.x(), point.y(), point.z()) - camera;
                let depth = relative.dot(&forward);
                if !depth.is_finite() || depth <= 1.0e-6 {
                    return None;
                }
                let focal_length = self.perspective_focal_length_pixels(rect);
                (
                    relative.dot(&right) / depth * focal_length,
                    relative.dot(&up) / depth * focal_length,
                )
            }
        };
        let x = f64::from(origin.x) + horizontal_pixels;
        let y = f64::from(origin.y) - vertical_pixels;
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
        match self.kind {
            ViewKind::Top | ViewKind::Front | ViewKind::Right => {
                let scale = Real::from(self.pixels_per_unit);
                let horizontal = Real::from(position.x - origin.x) / scale;
                let vertical = Real::from(origin.y - position.y) / scale;
                match self.kind {
                    ViewKind::Top => Point3::try_new(horizontal, vertical, elevation).ok(),
                    ViewKind::Front => Point3::try_new(horizontal, elevation, vertical).ok(),
                    ViewKind::Right => Point3::try_new(elevation, horizontal, vertical).ok(),
                    ViewKind::Perspective => unreachable!(),
                }
            }
            ViewKind::Perspective => {
                let (right, up, forward) = self.perspective_basis();
                let camera = -forward * self.perspective_camera_distance;
                let focal_length = self.perspective_focal_length_pixels(rect);
                let horizontal = Real::from(position.x - origin.x) / focal_length;
                let vertical = Real::from(origin.y - position.y) / focal_length;
                let ray = forward + right * horizontal + up * vertical;
                if !ray.z.is_finite() || ray.z.abs() <= 1.0e-12 {
                    return None;
                }
                let parameter = (elevation - camera.z) / ray.z;
                if !parameter.is_finite() || parameter < 0.0 {
                    return None;
                }
                let point = camera + ray * parameter;
                Point3::try_new(point.x, point.y, point.z).ok()
            }
        }
    }

    fn perspective_basis(&self) -> (NaVector3<Real>, NaVector3<Real>, NaVector3<Real>) {
        let outward = NaVector3::new(
            self.orbit_pitch.cos() * self.orbit_yaw.cos(),
            self.orbit_pitch.cos() * self.orbit_yaw.sin(),
            self.orbit_pitch.sin(),
        );
        let forward = -outward;
        let world_up = NaVector3::new(0.0, 0.0, 1.0);
        let right = forward.cross(&world_up).normalize();
        let up = right.cross(&forward).normalize();
        (right, up, forward)
    }

    fn perspective_focal_length_pixels(&self, rect: Rect) -> Real {
        let viewport_height = Real::from(rect.height().max(1.0));
        viewport_height / (2.0 * (PERSPECTIVE_VERTICAL_FOV_RADIANS / 2.0).tan())
    }

    fn pixels_per_model_unit_at_origin(&self, rect: Rect) -> f32 {
        match self.kind {
            ViewKind::Perspective => {
                (self.perspective_focal_length_pixels(rect) / self.perspective_camera_distance)
                    as f32
            }
            ViewKind::Top | ViewKind::Front | ViewKind::Right => self.pixels_per_unit,
        }
    }

    fn zoom_by(&mut self, factor: f32, pointer: Option<Pos2>, rect: Rect) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        if self.kind == ViewKind::Perspective {
            let anchor = pointer.and_then(|pointer| self.unproject(pointer, rect, 0.0));
            let old_distance = self.perspective_camera_distance;
            let new_distance = (old_distance / Real::from(factor)).clamp(
                MIN_PERSPECTIVE_CAMERA_DISTANCE,
                MAX_PERSPECTIVE_CAMERA_DISTANCE,
            );
            if new_distance == old_distance {
                return;
            }
            self.perspective_camera_distance = new_distance;
            if let (Some(pointer), Some(anchor)) = (pointer, anchor)
                && let Some(projected) = self.project(anchor, rect)
            {
                self.pan += pointer - projected;
            }
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
        let elevation = input.anchor.map_or(0.0, |anchor| match self.kind {
            ViewKind::Top | ViewKind::Perspective => anchor.z(),
            ViewKind::Front => anchor.y(),
            ViewKind::Right => anchor.x(),
        });
        let raw_point = self.unproject(pointer, rect, elevation)?;
        let object_snap = input
            .osnap
            .then(|| {
                if self.kind == ViewKind::Top {
                    nearest_object_snap(
                        document,
                        raw_point,
                        Real::from(OSNAP_CAPTURE_PIXELS / self.pixels_per_unit),
                    )
                } else {
                    nearest_object_snap_projected(
                        document,
                        [Real::from(pointer.x), Real::from(pointer.y)],
                        Real::from(OSNAP_CAPTURE_PIXELS),
                        |point| {
                            self.project(point, rect)
                                .map(|projected| [Real::from(projected.x), Real::from(projected.y)])
                        },
                    )
                }
                .ok()
                .flatten()
            })
            .flatten();
        let track = if object_snap.is_none()
            && input.smart_track
            && matches!(self.kind, ViewKind::Top | ViewKind::Perspective)
        {
            input.anchor.and_then(|anchor| {
                orthogonal_track(
                    raw_point,
                    anchor,
                    Real::from(TRACK_CAPTURE_PIXELS / self.pixels_per_model_unit_at_origin(rect)),
                )
                .ok()
                .flatten()
            })
        } else {
            None
        };
        let grid_point = input.grid_snap.then(|| self.snap_to_grid(raw_point));
        let point = object_snap
            .map(ObjectSnap::point)
            .or_else(|| track.map(OrthogonalTrack::point))
            .or(grid_point)
            .unwrap_or(raw_point);
        Some(DraftingCursor {
            pointer,
            point,
            object_snap,
            track,
            grid_snapped: object_snap.is_none() && track.is_none() && grid_point.is_some(),
        })
    }

    fn snap_to_grid(&self, point: Point3) -> Point3 {
        let snap = |coordinate: Real| {
            let snapped = (coordinate / GRID_SPACING).round() * GRID_SPACING;
            if snapped == 0.0 { 0.0 } else { snapped }
        };
        let coordinates = match self.kind {
            ViewKind::Top | ViewKind::Perspective => [snap(point.x()), snap(point.y()), point.z()],
            ViewKind::Front => [snap(point.x()), point.y(), snap(point.z())],
            ViewKind::Right => [point.x(), snap(point.y()), snap(point.z())],
        };
        Point3::try_new(coordinates[0], coordinates[1], coordinates[2])
            .expect("snapping finite coordinates to a finite grid remains finite")
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
                    let distance = if self.kind == ViewKind::Top {
                        self.unproject(pointer, rect, 0.0)
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
                            })
                    } else {
                        cloud
                            .points()
                            .iter()
                            .filter_map(|point| self.project(*point, rect))
                            .map(|projected| (projected - pointer).length())
                            .fold(f32::INFINITY, f32::min)
                    };
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
                    self.nurbs_surface_pick_distance(
                        pointer,
                        rect,
                        surface,
                        object.attributes().wire_density(),
                        document.tolerance(),
                    ),
                ),
                Geometry::Brep(brep) => (
                    2,
                    self.brep_pick_distance(
                        pointer,
                        rect,
                        brep,
                        object.attributes().wire_density(),
                        document.tolerance(),
                    ),
                ),
                Geometry::Mesh(mesh) => (
                    2,
                    self.mesh_pick_distance(pointer, rect, mesh, document.tolerance()),
                ),
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

    fn objects_in_selection(
        &self,
        viewport_rect: Rect,
        selection: Rect,
        crossing: bool,
        document: &Document,
    ) -> Vec<ObjectId> {
        document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .filter_map(|object| {
                let primitives = self.projected_primitives(
                    object.geometry(),
                    object.attributes(),
                    viewport_rect,
                    document.tolerance(),
                );
                let selected = if crossing {
                    primitives.is_crossed_by(selection)
                } else {
                    primitives.is_windowed_by(selection)
                };
                selected.then_some(object.id())
            })
            .collect()
    }

    fn projected_primitives(
        &self,
        geometry: &Geometry,
        attributes: &ObjectAttributes,
        viewport_rect: Rect,
        tolerance: Tolerance,
    ) -> ProjectedPrimitives {
        let mut projected = ProjectedPrimitives::default();
        match geometry {
            Geometry::Point(point) => projected.add_point(self.project(*point, viewport_rect)),
            Geometry::PointCloud(cloud) => {
                for point in cloud.points() {
                    projected.add_point(self.project(*point, viewport_rect));
                }
            }
            Geometry::Line(line) => projected.add_segment(
                self.project(line.start(), viewport_rect),
                self.project(line.end(), viewport_rect),
            ),
            Geometry::Circle(circle) => self.add_projected_parametric_curve(
                &mut projected,
                viewport_rect,
                CIRCLE_SAMPLES,
                |parameter| circle.point_at_angle(std::f64::consts::TAU * parameter),
            ),
            Geometry::Arc(arc) => self.add_projected_parametric_curve(
                &mut projected,
                viewport_rect,
                circular_arc_samples(*arc),
                |parameter| arc.point_at(parameter),
            ),
            Geometry::Ellipse(ellipse) => self.add_projected_parametric_curve(
                &mut projected,
                viewport_rect,
                CIRCLE_SAMPLES,
                |parameter| ellipse.point_at_angle(std::f64::consts::TAU * parameter),
            ),
            Geometry::Polyline(polyline) => {
                for segment in polyline.segments() {
                    projected.add_segment(
                        self.project(segment.start(), viewport_rect),
                        self.project(segment.end(), viewport_rect),
                    );
                }
            }
            Geometry::NurbsCurve(curve) => {
                self.add_projected_nurbs_curve(&mut projected, viewport_rect, curve);
            }
            Geometry::NurbsSurface(surface) => {
                if self.display_mode != DisplayMode::Wireframe
                    && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
                {
                    self.add_projected_mesh(&mut projected, viewport_rect, &mesh, true, tolerance);
                }
                if let Ok(curves) = surface.wireframe_curves(attributes.wire_density()) {
                    for curve in &curves {
                        self.add_projected_nurbs_curve(&mut projected, viewport_rect, curve);
                    }
                }
            }
            Geometry::Brep(brep) => {
                if self.display_mode != DisplayMode::Wireframe
                    && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
                {
                    self.add_projected_mesh(&mut projected, viewport_rect, &mesh, true, tolerance);
                }
                if let Ok(curves) = brep.wireframe_curves(attributes.wire_density(), tolerance) {
                    for curve in &curves {
                        self.add_projected_nurbs_curve(&mut projected, viewport_rect, curve);
                    }
                }
            }
            Geometry::Mesh(mesh) => self.add_projected_mesh(
                &mut projected,
                viewport_rect,
                mesh,
                self.display_mode != DisplayMode::Wireframe,
                tolerance,
            ),
        }
        projected
    }

    fn add_projected_parametric_curve(
        &self,
        projected: &mut ProjectedPrimitives,
        rect: Rect,
        samples: usize,
        mut evaluate: impl FnMut(Real) -> Result<Point3, viboceros_geometry::GeometryError>,
    ) {
        let mut previous = None;
        for sample in 0..=samples {
            let point = evaluate(sample as Real / samples as Real)
                .ok()
                .and_then(|point| self.project(point, rect));
            projected.add_segment(previous, point);
            previous = point;
        }
    }

    fn add_projected_nurbs_curve(
        &self,
        projected: &mut ProjectedPrimitives,
        rect: Rect,
        curve: &NurbsCurve,
    ) {
        let domain_end = *curve.domain().end();
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=CURVE_SAMPLES_PER_SPAN {
                let fraction = sample as Real / CURVE_SAMPLES_PER_SPAN as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                if sample == CURVE_SAMPLES_PER_SPAN && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }
                let point = curve
                    .evaluate(parameter)
                    .ok()
                    .and_then(|point| self.project(point, rect));
                projected.add_segment(previous, point);
                previous = point;
            }
        }
    }

    fn add_projected_mesh(
        &self,
        projected: &mut ProjectedPrimitives,
        rect: Rect,
        mesh: &TriangleMesh,
        include_faces: bool,
        tolerance: Tolerance,
    ) {
        for point in mesh.vertices() {
            projected.add_point(self.project(*point, rect));
        }
        if let Ok(lines) = mesh.wireframe_lines(tolerance) {
            for line in lines {
                projected.add_segment(
                    self.project(line.start(), rect),
                    self.project(line.end(), rect),
                );
            }
        }
        if include_faces {
            for triangle in 0..mesh.triangles().len() {
                if let Some(points) = mesh.triangle_points(triangle) {
                    projected.add_triangle(points.map(|point| self.project(point, rect)));
                }
            }
        }
    }

    fn paint_selection_window(&self, painter: &egui::Painter, start: Pos2, end: Pos2) {
        let selection = Rect::from_two_pos(start, end);
        let crossing = is_crossing_selection(start, end);
        let color = if crossing {
            Color32::from_rgb(45, 145, 75)
        } else {
            Color32::from_rgb(45, 105, 215)
        };
        painter.rect_filled(
            selection,
            0.0,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 28),
        );
        if crossing {
            let corners = rect_corners(selection);
            painter.extend(egui::Shape::dashed_line(
                &[corners[0], corners[1], corners[2], corners[3], corners[0]],
                Stroke::new(1.25, color),
                5.0,
                3.0,
            ));
        } else {
            painter.rect_stroke(
                selection,
                0.0,
                Stroke::new(1.25, color),
                egui::StrokeKind::Inside,
            );
        }
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

    fn mesh_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        mesh: &TriangleMesh,
        tolerance: Tolerance,
    ) -> f32 {
        if self.display_mode == DisplayMode::Wireframe {
            return mesh
                .wireframe_lines(tolerance)
                .map(|lines| {
                    lines
                        .into_iter()
                        .filter_map(|line| {
                            self.project(line.start(), rect)
                                .zip(self.project(line.end(), rect))
                        })
                        .map(|(start, end)| point_segment_distance(pointer, start, end))
                        .fold(f32::INFINITY, f32::min)
                })
                .unwrap_or(f32::INFINITY);
        }
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
        wire_density: i32,
        tolerance: Tolerance,
    ) -> f32 {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick_distance(pointer, rect, &mesh, tolerance);
        }
        surface
            .wireframe_curves(wire_density)
            .map(|curves| {
                curves
                    .iter()
                    .map(|curve| self.nurbs_pick_distance(pointer, rect, curve))
                    .fold(f32::INFINITY, f32::min)
            })
            .unwrap_or(f32::INFINITY)
    }

    fn brep_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        brep: &Brep,
        wire_density: i32,
        tolerance: Tolerance,
    ) -> f32 {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            return self.mesh_pick_distance(pointer, rect, &mesh, tolerance);
        }
        brep.wireframe_curves(wire_density, tolerance)
            .map(|curves| {
                curves
                    .iter()
                    .map(|curve| self.nurbs_pick_distance(pointer, rect, curve))
                    .fold(f32::INFINITY, f32::min)
            })
            .unwrap_or(f32::INFINITY)
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
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

        let horizontal_color = match self.kind {
            ViewKind::Right => Color32::from_rgb(60, 145, 75),
            _ => Color32::from_rgb(190, 65, 65),
        };
        let vertical_color = match self.kind {
            ViewKind::Top | ViewKind::Perspective => Color32::from_rgb(60, 145, 75),
            ViewKind::Front | ViewKind::Right => Color32::from_rgb(65, 105, 195),
        };
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

    fn grid_point(&self, horizontal: Real, vertical: Real) -> Point3 {
        match self.kind {
            ViewKind::Top | ViewKind::Perspective => Point3::try_new(horizontal, vertical, 0.0),
            ViewKind::Front => Point3::try_new(horizontal, 0.0, vertical),
            ViewKind::Right => Point3::try_new(0.0, horizontal, vertical),
        }
        .expect("finite grid coordinates form a finite point")
    }

    fn paint_grid_line(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        start: Point3,
        end: Point3,
        stroke: Stroke,
    ) {
        if let (Some(start), Some(end)) = (self.project(start, rect), self.project(end, rect)) {
            painter.line_segment([start, end], stroke);
        }
    }

    fn paint_objects(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        viewport_index: usize,
    ) {
        let mut scene = GpuSceneBuilder::new();
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
                    self.add_gpu_point(&mut scene, rect, *point, 4.5, color);
                }
                Geometry::PointCloud(cloud) => {
                    let radius = if selected { 3.5 } else { 2.5 };
                    for point in cloud.points() {
                        self.add_gpu_point(&mut scene, rect, *point, radius, color);
                    }
                }
                Geometry::Line(line) => {
                    self.add_gpu_line(&mut scene, rect, line.start(), line.end(), width, color);
                }
                Geometry::Circle(circle) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        CIRCLE_SAMPLES,
                        width,
                        color,
                        |parameter| circle.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Arc(arc) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        circular_arc_samples(*arc),
                        width,
                        color,
                        |parameter| arc.point_at(parameter),
                    );
                }
                Geometry::Ellipse(ellipse) => {
                    self.add_gpu_parametric_curve(
                        &mut scene,
                        rect,
                        CIRCLE_SAMPLES,
                        width,
                        color,
                        |parameter| ellipse.point_at_angle(std::f64::consts::TAU * parameter),
                    );
                }
                Geometry::Polyline(polyline) => {
                    for segment in polyline.segments() {
                        self.add_gpu_line(
                            &mut scene,
                            rect,
                            segment.start(),
                            segment.end(),
                            width,
                            color,
                        );
                    }
                }
                Geometry::NurbsCurve(curve) => {
                    self.add_gpu_nurbs_curve(&mut scene, rect, curve, width, color);
                }
                Geometry::NurbsSurface(surface) => {
                    self.add_gpu_nurbs_surface(
                        &mut scene,
                        rect,
                        surface,
                        SurfaceDisplayStyle {
                            color,
                            width,
                            wire_density: attributes.wire_density(),
                        },
                        document.tolerance(),
                    );
                }
                Geometry::Brep(brep) => {
                    self.add_gpu_brep(
                        &mut scene,
                        rect,
                        brep,
                        SurfaceDisplayStyle {
                            color,
                            width,
                            wire_density: attributes.wire_density(),
                        },
                        document.tolerance(),
                    );
                }
                Geometry::Mesh(mesh) => {
                    self.add_gpu_mesh(&mut scene, rect, mesh, color, width, document.tolerance());
                }
            }
        }

        let uniform = self.gpu_view_uniform(rect, scene.depth_range());
        let transparent = self.display_mode == DisplayMode::Ghosted;
        crate::viewport_gpu::paint(
            painter,
            rect,
            viewport_index,
            scene.finish(uniform, transparent),
        );
    }

    fn add_gpu_point(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        point: Point3,
        radius: f32,
        color: Color32,
    ) {
        let Some(projected) = self.project(point, rect) else {
            return;
        };
        if !rect.expand(radius).contains(projected) {
            return;
        }
        let Some(position) = point_to_gpu(point) else {
            return;
        };
        scene.include_depth(self.view_depth(point));
        scene.points.push(GpuPointInstance {
            position_size: [position[0], position[1], position[2], radius],
            color: color_to_gpu(color),
        });
    }

    fn add_gpu_line(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        start: Point3,
        end: Point3,
        width: f32,
        color: Color32,
    ) {
        if self.project(start, rect).is_none() || self.project(end, rect).is_none() {
            return;
        }
        let (Some(start_position), Some(end_position)) = (point_to_gpu(start), point_to_gpu(end))
        else {
            return;
        };
        scene.include_depth(self.view_depth(start));
        scene.include_depth(self.view_depth(end));
        scene.lines.push(GpuLineInstance {
            start_width: [
                start_position[0],
                start_position[1],
                start_position[2],
                width,
            ],
            end_padding: [end_position[0], end_position[1], end_position[2], 0.0],
            color: color_to_gpu(color),
        });
    }

    fn add_gpu_nurbs_curve(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        curve: &NurbsCurve,
        width: f32,
        color: Color32,
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

                let evaluated = curve.evaluate(parameter).ok();
                if let (Some(start), Some(end)) = (previous, evaluated) {
                    self.add_gpu_line(scene, rect, start, end, width, color);
                }
                previous = evaluated;
            }
        }
    }

    fn add_gpu_parametric_curve(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        samples: usize,
        width: f32,
        color: Color32,
        mut evaluate: impl FnMut(Real) -> Result<Point3, viboceros_geometry::GeometryError>,
    ) {
        let mut previous = None;
        for sample in 0..=samples {
            let evaluated = evaluate(sample as Real / samples as Real).ok();
            if let (Some(start), Some(end)) = (previous, evaluated) {
                self.add_gpu_line(scene, rect, start, end, width, color);
            }
            previous = evaluated;
        }
    }

    fn add_gpu_nurbs_surface(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        surface: &NurbsSurface,
        style: SurfaceDisplayStyle,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = surface.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            self.add_gpu_mesh_faces(scene, rect, &mesh, style.color);
        }

        if let Ok(curves) = surface.wireframe_curves(style.wire_density) {
            for curve in &curves {
                self.add_gpu_nurbs_curve(scene, rect, curve, style.width, style.color);
            }
        }
    }

    fn add_gpu_brep(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        brep: &Brep,
        style: SurfaceDisplayStyle,
        tolerance: Tolerance,
    ) {
        if self.display_mode != DisplayMode::Wireframe
            && let Ok(mesh) = brep.tessellate(SURFACE_SAMPLES_PER_SPAN, tolerance)
        {
            self.add_gpu_mesh_faces(scene, rect, &mesh, style.color);
        }
        if let Ok(curves) = brep.wireframe_curves(style.wire_density, tolerance) {
            for curve in &curves {
                self.add_gpu_nurbs_curve(scene, rect, curve, style.width, style.color);
            }
        }
    }

    fn add_gpu_mesh(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        mesh: &TriangleMesh,
        color: Color32,
        width: f32,
        tolerance: Tolerance,
    ) {
        self.add_gpu_mesh_faces(scene, rect, mesh, color);
        if let Ok(lines) = mesh.wireframe_lines(tolerance) {
            for line in lines {
                self.add_gpu_line(scene, rect, line.start(), line.end(), width, color);
            }
        }
    }

    fn add_gpu_mesh_faces(
        &self,
        scene: &mut GpuSceneBuilder,
        rect: Rect,
        mesh: &TriangleMesh,
        color: Color32,
    ) {
        if self.display_mode == DisplayMode::Wireframe {
            return;
        }

        let corner_normals = smooth_corner_normals(mesh);
        let face_color = if self.display_mode == DisplayMode::Ghosted {
            color_with_alpha(color, 35)
        } else {
            color
        };
        let gpu_color = color_to_gpu(face_color);
        for (triangle_index, normals) in corner_normals.into_iter().enumerate() {
            let Some(points) = mesh.triangle_points(triangle_index) else {
                continue;
            };
            if points
                .iter()
                .any(|point| self.project(*point, rect).is_none())
            {
                continue;
            }
            let [Some(first), Some(second), Some(third)] = points.map(point_to_gpu) else {
                continue;
            };
            let normals = normals.map(vector_to_gpu);
            let depth = points
                .into_iter()
                .map(|point| self.view_depth(point))
                .sum::<Real>()
                / 3.0;
            for point in points {
                scene.include_depth(self.view_depth(point));
            }
            scene.triangles.push(DepthTriangle {
                depth,
                vertices: [
                    GpuTriangleVertex {
                        position: first,
                        normal: normals[0],
                        color: gpu_color,
                    },
                    GpuTriangleVertex {
                        position: second,
                        normal: normals[1],
                        color: gpu_color,
                    },
                    GpuTriangleVertex {
                        position: third,
                        normal: normals[2],
                        color: gpu_color,
                    },
                ],
            });
        }
    }

    fn gpu_view_uniform(&self, rect: Rect, depth_range: Option<(Real, Real)>) -> GpuViewUniform {
        let width = Real::from(rect.width().max(1.0));
        let height = Real::from(rect.height().max(1.0));
        let offset_x = 2.0 * Real::from(self.pan.x) / width;
        let offset_y = -2.0 * Real::from(self.pan.y) / height;
        let view_projection = match self.kind {
            ViewKind::Perspective => {
                let (right, up, forward) = self.perspective_basis();
                let camera = -forward * self.perspective_camera_distance;
                let view = NaMatrix4::new(
                    right.x,
                    right.y,
                    right.z,
                    -right.dot(&camera),
                    up.x,
                    up.y,
                    up.z,
                    -up.dot(&camera),
                    forward.x,
                    forward.y,
                    forward.z,
                    -forward.dot(&camera),
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                );
                let (minimum_depth, maximum_depth) = depth_range.unwrap_or((
                    self.perspective_camera_distance * 0.5,
                    self.perspective_camera_distance * 1.5,
                ));
                let near = (minimum_depth * 0.5)
                    .max(self.perspective_camera_distance * 1.0e-6)
                    .max(1.0e-6);
                let far = (maximum_depth * 1.5)
                    .max(self.perspective_camera_distance * 2.0)
                    .max(near + 1.0);
                let focal_length = self.perspective_focal_length_pixels(rect);
                let projection = NaMatrix4::new(
                    2.0 * focal_length / width,
                    0.0,
                    offset_x,
                    0.0,
                    0.0,
                    2.0 * focal_length / height,
                    offset_y,
                    0.0,
                    0.0,
                    0.0,
                    far / (far - near),
                    -far * near / (far - near),
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                );
                projection * view
            }
            ViewKind::Top | ViewKind::Front | ViewKind::Right => {
                let (right, up, forward) = match self.kind {
                    ViewKind::Top => (
                        NaVector3::new(1.0, 0.0, 0.0),
                        NaVector3::new(0.0, 1.0, 0.0),
                        NaVector3::new(0.0, 0.0, -1.0),
                    ),
                    ViewKind::Front => (
                        NaVector3::new(1.0, 0.0, 0.0),
                        NaVector3::new(0.0, 0.0, 1.0),
                        NaVector3::new(0.0, 1.0, 0.0),
                    ),
                    ViewKind::Right => (
                        NaVector3::new(0.0, 1.0, 0.0),
                        NaVector3::new(0.0, 0.0, 1.0),
                        NaVector3::new(-1.0, 0.0, 0.0),
                    ),
                    ViewKind::Perspective => unreachable!(),
                };
                let (minimum_depth, maximum_depth) = depth_range.unwrap_or((-1.0, 1.0));
                let span = (maximum_depth - minimum_depth).abs().max(1.0);
                let near = minimum_depth - span * 0.05 - 1.0e-3;
                let far = maximum_depth + span * 0.05 + 1.0e-3;
                let depth_span = far - near;
                let horizontal_scale = 2.0 * Real::from(self.pixels_per_unit) / width;
                let vertical_scale = 2.0 * Real::from(self.pixels_per_unit) / height;
                NaMatrix4::new(
                    horizontal_scale * right.x,
                    horizontal_scale * right.y,
                    horizontal_scale * right.z,
                    offset_x,
                    vertical_scale * up.x,
                    vertical_scale * up.y,
                    vertical_scale * up.z,
                    offset_y,
                    forward.x / depth_span,
                    forward.y / depth_span,
                    forward.z / depth_span,
                    -near / depth_span,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                )
            }
        };

        GpuViewUniform {
            view_projection: matrix_to_gpu(view_projection),
            viewport_size: [rect.width().max(1.0), rect.height().max(1.0)],
            padding: [0.0; 2],
        }
    }

    fn view_depth(&self, point: Point3) -> Real {
        match self.kind {
            ViewKind::Top => -point.z(),
            ViewKind::Front => point.y(),
            ViewKind::Right => -point.x(),
            ViewKind::Perspective => {
                let (_, _, forward) = self.perspective_basis();
                let camera = -forward * self.perspective_camera_distance;
                (NaVector3::new(point.x(), point.y(), point.z()) - camera).dot(&forward)
            }
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
        const GRID_COLOR: Color32 = Color32::from_rgb(80, 120, 45);

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

fn circular_arc_samples(arc: CircularArc3) -> usize {
    ((arc.sweep_radians() / std::f64::consts::TAU * CIRCLE_SAMPLES as Real).ceil() as usize).max(2)
}

fn selection_mode(modifiers: egui::Modifiers) -> SelectionMode {
    if modifiers.command && !modifiers.shift {
        SelectionMode::Remove
    } else if modifiers.shift {
        SelectionMode::Add
    } else {
        SelectionMode::Replace
    }
}

fn is_crossing_selection(start: Pos2, end: Pos2) -> bool {
    end.x < start.x
}

fn segment_intersects_rect(start: Pos2, end: Pos2, rect: Rect) -> bool {
    if rect.contains(start) || rect.contains(end) {
        return true;
    }
    let delta = end - start;
    let mut minimum = 0.0_f32;
    let mut maximum = 1.0_f32;
    for (direction, distance) in [
        (-delta.x, start.x - rect.left()),
        (delta.x, rect.right() - start.x),
        (-delta.y, start.y - rect.top()),
        (delta.y, rect.bottom() - start.y),
    ] {
        if direction == 0.0 {
            if distance < 0.0 {
                return false;
            }
            continue;
        }
        let parameter = distance / direction;
        if direction < 0.0 {
            minimum = minimum.max(parameter);
        } else {
            maximum = maximum.min(parameter);
        }
        if minimum > maximum {
            return false;
        }
    }
    true
}

fn rect_corners(rect: Rect) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
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

fn point_to_gpu(point: Point3) -> Option<[f32; 3]> {
    Some([
        real_to_gpu(point.x())?,
        real_to_gpu(point.y())?,
        real_to_gpu(point.z())?,
    ])
}

fn vector_to_gpu(vector: NaVector3<Real>) -> [f32; 3] {
    [vector.x as f32, vector.y as f32, vector.z as f32]
}

fn real_to_gpu(value: Real) -> Option<f32> {
    (value.is_finite() && value.abs() <= Real::from(f32::MAX)).then_some(value as f32)
}

fn color_to_gpu(color: Color32) -> [f32; 4] {
    color
        .to_srgba_unmultiplied()
        .map(|component| f32::from(component) / 255.0)
}

fn color_with_alpha(color: Color32, alpha: u8) -> Color32 {
    let [red, green, blue, _] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn matrix_to_gpu(matrix: NaMatrix4<Real>) -> [[f32; 4]; 4] {
    std::array::from_fn(|column| std::array::from_fn(|row| matrix[(row, column)] as f32))
}

fn resolved_display_color(attributes: &ObjectAttributes, layer_color: ColorRgb) -> Color32 {
    let color = attributes.display_color(layer_color);
    Color32::from_rgb(color.red, color.green, color.blue)
}

fn smooth_corner_normals(mesh: &TriangleMesh) -> Vec<[NaVector3<Real>; 3]> {
    let fallback = NaVector3::new(0.0, 0.0, 1.0);
    let face_normals = (0..mesh.triangles().len())
        .map(|index| {
            mesh.face_normal(index)
                .map(|normal| NaVector3::new(normal.x(), normal.y(), normal.z()))
                .unwrap_or(fallback)
        })
        .collect::<Vec<_>>();

    // Surface tessellation intentionally duplicates vertices at knot-span
    // boundaries. Group exact coincident samples so continuous spans shade as
    // one surface, then use the crease angle below to keep analytic caps and
    // other genuinely sharp joins hard.
    let mut incident_faces: HashMap<[u64; 3], Vec<usize>> = HashMap::new();
    for (face_index, triangle) in mesh.triangles().iter().enumerate() {
        for &vertex_index in triangle {
            let point = mesh.vertices()[vertex_index as usize];
            incident_faces
                .entry(point_position_key(point))
                .or_default()
                .push(face_index);
        }
    }

    mesh.triangles()
        .iter()
        .enumerate()
        .map(|(face_index, triangle)| {
            let reference = face_normals[face_index];
            triangle.map(|vertex_index| {
                let point = mesh.vertices()[vertex_index as usize];
                let mut sum = NaVector3::zeros();
                for &incident in &incident_faces[&point_position_key(point)] {
                    let candidate = face_normals[incident];
                    if reference.dot(&candidate) >= SMOOTH_SHADING_COSINE {
                        sum += candidate;
                    }
                }
                sum.try_normalize(Real::EPSILON).unwrap_or(reference)
            })
        })
        .collect()
}

fn point_position_key(point: Point3) -> [u64; 3] {
    [point.x(), point.y(), point.z()].map(|value| {
        if value == 0.0 {
            0.0_f64.to_bits()
        } else {
            value.to_bits()
        }
    })
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

    fn gpu_project(
        viewport: &Viewport,
        rect: Rect,
        point: Point3,
        depth_range: (Real, Real),
    ) -> (Pos2, f32) {
        let matrix = viewport
            .gpu_view_uniform(rect, Some(depth_range))
            .view_projection;
        let position = [point.x() as f32, point.y() as f32, point.z() as f32, 1.0];
        let clip: [f32; 4] = std::array::from_fn(|row| {
            (0..4)
                .map(|column| matrix[column][row] * position[column])
                .sum()
        });
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
        (
            Pos2::new(
                rect.center().x + ndc[0] * rect.width() * 0.5,
                rect.center().y - ndc[1] * rect.height() * 0.5,
            ),
            ndc[2],
        )
    }

    fn viewport_frame(
        context: &egui::Context,
        viewport: &mut Viewport,
        document: &Document,
        events: Vec<egui::Event>,
    ) -> ViewportOutput {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    events,
                    ..egui::RawInput::default()
                },
                |ui| {
                    output = viewport.show(ui, document, DraftingInput::default(), &[], 0, true);
                },
            )
            .drop_without_applying_deltas();
        output
    }

    fn drag_viewport(
        context: &egui::Context,
        viewport: &mut Viewport,
        document: &Document,
        button: PointerButton,
        start: Pos2,
        end: Pos2,
    ) -> ViewportOutput {
        let pointer_event = |position, pressed| egui::Event::PointerButton {
            pos: position,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        viewport_frame(context, viewport, document, Vec::new());
        viewport_frame(
            context,
            viewport,
            document,
            vec![egui::Event::PointerMoved(start), pointer_event(start, true)],
        );
        viewport_frame(
            context,
            viewport,
            document,
            vec![egui::Event::PointerMoved(end)],
        );
        viewport_frame(
            context,
            viewport,
            document,
            vec![egui::Event::PointerMoved(end), pointer_event(end, false)],
        )
    }

    #[test]
    fn projection_rejects_values_outside_f32_range() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let point = Point3::try_new(f64::MAX, 0.0, 0.0).unwrap();
        assert_eq!(viewport.project(point, rect), None);
    }

    #[test]
    fn standard_views_project_the_expected_world_axes() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let model = point(2.0, 3.0, 4.0);
        let projected = |kind| Viewport::new(kind).project(model, rect).unwrap();
        assert_eq!(projected(ViewKind::Top), Pos2::new(480.0, 180.0));
        assert_eq!(projected(ViewKind::Front), Pos2::new(480.0, 140.0));
        assert_eq!(projected(ViewKind::Right), Pos2::new(520.0, 140.0));
    }

    #[test]
    fn every_view_projects_and_unprojects_its_construction_plane() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));
        let model = point(2.25, -3.5, 1.75);
        for kind in [
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right,
        ] {
            let viewport = Viewport::new(kind);
            let screen = viewport.project(model, rect).unwrap();
            let fixed_coordinate = match kind {
                ViewKind::Top | ViewKind::Perspective => model.z(),
                ViewKind::Front => model.y(),
                ViewKind::Right => model.x(),
            };
            let round_trip = viewport.unproject(screen, rect, fixed_coordinate).unwrap();
            assert!((round_trip.x() - model.x()).abs() < 1.0e-5);
            assert!((round_trip.y() - model.y()).abs() < 1.0e-5);
            assert!((round_trip.z() - model.z()).abs() < 1.0e-5);
        }
    }

    #[test]
    fn perspective_foreshortens_geometry_with_depth() {
        let viewport = Viewport::new(ViewKind::Perspective);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let (right, _, forward) = viewport.perspective_basis();
        let make_point = |vector: NaVector3<Real>| point(vector.x, vector.y, vector.z);
        let near = make_point(right * 4.0 - forward * 5.0);
        let far = make_point(right * 4.0 + forward * 5.0);
        let center = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        let near_distance = (viewport.project(near, rect).unwrap() - center).length();
        let far_distance = (viewport.project(far, rect).unwrap() - center).length();
        assert!(near_distance > far_distance);
    }

    #[test]
    fn right_drag_matches_parallel_and_perspective_navigation() {
        let delta = Vec2::new(12.0, -7.0);
        let mut top = Viewport::new(ViewKind::Top);
        let top_angles = (top.orbit_yaw, top.orbit_pitch);
        top.apply_navigation_drag(PointerButton::Secondary, egui::Modifiers::NONE, delta);
        assert_eq!(top.pan, delta);
        assert_eq!((top.orbit_yaw, top.orbit_pitch), top_angles);

        let mut perspective = Viewport::new(ViewKind::Perspective);
        let perspective_angles = (perspective.orbit_yaw, perspective.orbit_pitch);
        perspective.apply_navigation_drag(PointerButton::Secondary, egui::Modifiers::NONE, delta);
        assert_eq!(perspective.pan, Vec2::ZERO);
        assert_ne!(
            (perspective.orbit_yaw, perspective.orbit_pitch),
            perspective_angles
        );

        let mut shifted = Viewport::new(ViewKind::Perspective);
        let shifted_angles = (shifted.orbit_yaw, shifted.orbit_pitch);
        shifted.apply_navigation_drag(
            PointerButton::Secondary,
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::NONE
            },
            delta,
        );
        assert_eq!(shifted.pan, delta);
        assert_eq!((shifted.orbit_yaw, shifted.orbit_pitch), shifted_angles);
    }

    #[test]
    fn right_click_emits_enter_and_activates_the_viewport() {
        let context = egui::Context::default();
        let mut viewport = Viewport::new(ViewKind::Top);
        let document = Document::default();
        let position = Pos2::new(200.0, 150.0);
        let pointer_event = |pressed| egui::Event::PointerButton {
            pos: position,
            button: PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        viewport_frame(&context, &mut viewport, &document, Vec::new());
        viewport_frame(
            &context,
            &mut viewport,
            &document,
            vec![egui::Event::PointerMoved(position), pointer_event(true)],
        );
        let output = viewport_frame(
            &context,
            &mut viewport,
            &document,
            vec![egui::Event::PointerMoved(position), pointer_event(false)],
        );
        assert!(output.enter_pressed, "{output:?}");
        assert!(output.activated, "{output:?}");
    }

    #[test]
    fn right_drag_navigates_without_emitting_enter() {
        let context = egui::Context::default();
        let mut viewport = Viewport::new(ViewKind::Top);
        let document = Document::default();
        let output = drag_viewport(
            &context,
            &mut viewport,
            &document,
            PointerButton::Secondary,
            Pos2::new(200.0, 150.0),
            Pos2::new(240.0, 180.0),
        );
        assert!(!output.enter_pressed);
        assert!(output.activated);
        assert_eq!(viewport.pan, Vec2::new(40.0, 30.0));
    }

    #[test]
    fn grid_snap_uses_each_views_construction_plane() {
        assert_eq!(
            Viewport::new(ViewKind::Top).snap_to_grid(point(1.49, -1.51, 7.25)),
            point(1.0, -2.0, 7.25)
        );
        assert_eq!(
            Viewport::new(ViewKind::Front).snap_to_grid(point(1.49, 7.25, -1.51)),
            point(1.0, 7.25, -2.0)
        );
        assert_eq!(
            Viewport::new(ViewKind::Right).snap_to_grid(point(7.25, 1.49, -1.51)),
            point(7.25, 1.0, -2.0)
        );
    }

    #[test]
    fn selection_direction_switches_between_window_and_crossing() {
        assert!(!is_crossing_selection(
            Pos2::new(10.0, 20.0),
            Pos2::new(30.0, 5.0)
        ));
        assert!(is_crossing_selection(
            Pos2::new(30.0, 20.0),
            Pos2::new(10.0, 35.0)
        ));

        let viewport = Viewport::new(ViewKind::Top);
        let viewport_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let selection_rect = Rect::from_min_max(Pos2::new(390.0, 280.0), Pos2::new(450.0, 320.0));
        let mut document = Document::default();
        let crossing_line = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(-2.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let enclosed_point = document
            .add_geometry(Geometry::Point(point(0.5, 0.0, 0.0)))
            .unwrap();

        assert_eq!(
            viewport.objects_in_selection(viewport_rect, selection_rect, false, &document),
            [enclosed_point]
        );
        assert_eq!(
            viewport.objects_in_selection(viewport_rect, selection_rect, true, &document),
            [crossing_line, enclosed_point]
        );
    }

    #[test]
    fn primary_drag_emits_directional_selection_results() {
        let context = egui::Context::default();
        let mut viewport = Viewport::new(ViewKind::Top);
        let mut document = Document::default();
        let crossing_line = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    point(-2.0, 0.0, 0.0),
                    point(2.0, 0.0, 0.0),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        let enclosed_point = document
            .add_geometry(Geometry::Point(point(0.5, 0.0, 0.0)))
            .unwrap();

        let window = drag_viewport(
            &context,
            &mut viewport,
            &document,
            PointerButton::Primary,
            Pos2::new(390.0, 280.0),
            Pos2::new(450.0, 320.0),
        )
        .selection_window
        .unwrap();
        assert!(!window.crossing);
        assert_eq!(window.object_ids, [enclosed_point]);

        let crossing = drag_viewport(
            &context,
            &mut viewport,
            &document,
            PointerButton::Primary,
            Pos2::new(450.0, 280.0),
            Pos2::new(390.0, 320.0),
        )
        .selection_window
        .unwrap();
        assert!(crossing.crossing);
        assert_eq!(crossing.object_ids, [crossing_line, enclosed_point]);
    }

    #[test]
    fn selection_modifiers_match_rhino_add_and_remove_rules() {
        assert_eq!(
            selection_mode(egui::Modifiers::NONE),
            SelectionMode::Replace
        );
        assert_eq!(
            selection_mode(egui::Modifiers {
                shift: true,
                ..egui::Modifiers::NONE
            }),
            SelectionMode::Add
        );
        assert_eq!(
            selection_mode(egui::Modifiers {
                command: true,
                ..egui::Modifiers::NONE
            }),
            SelectionMode::Remove
        );
    }

    #[test]
    fn smooth_shading_keeps_ninety_degree_mesh_edges_hard() {
        let mesh = TriangleMesh::try_new(
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(0.0, 1.0, 0.0),
                point(0.0, 0.0, 1.0),
            ],
            vec![[0, 1, 2], [0, 1, 3]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let normals = smooth_corner_normals(&mesh);
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].z, 1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[0][0].y, 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].y, -1.0));
        assert!(Tolerance::DEFAULT.approx_eq(normals[1][0].z, 0.0));
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
    fn perspective_zoom_dollies_the_camera_without_changing_the_lens() {
        let mut viewport = Viewport::new(ViewKind::Perspective);
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));
        let model = point(5.0, 0.0, 0.0);
        let origin = viewport.world_origin(rect);
        let before = viewport.project(model, rect).unwrap();
        let focal_length = viewport.perspective_focal_length_pixels(rect);
        let camera_distance = viewport.perspective_camera_distance;

        viewport.zoom_by(0.5, None, rect);

        let after = viewport.project(model, rect).unwrap();
        assert_eq!(viewport.perspective_focal_length_pixels(rect), focal_length);
        assert_eq!(viewport.perspective_camera_distance, camera_distance * 2.0);
        assert!((after - origin).length() < (before - origin).length());

        let on_construction_plane = point(3.25, -2.75, 0.0);
        let projected = viewport.project(on_construction_plane, rect).unwrap();
        let round_trip = viewport.unproject(projected, rect, 0.0).unwrap();
        assert!((round_trip.x() - on_construction_plane.x()).abs() < 1.0e-5);
        assert!((round_trip.y() - on_construction_plane.y()).abs() < 1.0e-5);
        assert!(round_trip.z().abs() < 1.0e-5);
    }

    #[test]
    fn gpu_projection_matches_interaction_projection_and_orders_depth() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));
        let model = point(3.25, -2.75, 1.5);
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let viewport = Viewport {
                kind,
                pan: Vec2::new(17.0, -23.0),
                ..Viewport::default()
            };
            let depth = viewport.view_depth(model);
            let depth_range = (depth - 10.0, depth + 10.0);
            let cpu = viewport.project(model, rect).unwrap();
            let (gpu, gpu_depth) = gpu_project(&viewport, rect, model, depth_range);
            assert!(
                (gpu - cpu).length() < 1.0e-3,
                "{kind:?}: {gpu:?} != {cpu:?}"
            );
            assert!((0.0..=1.0).contains(&gpu_depth), "{kind:?}: {gpu_depth}");

            let (near, far) = match kind {
                ViewKind::Top => (point(0.0, 0.0, 5.0), point(0.0, 0.0, -5.0)),
                ViewKind::Front => (point(0.0, -5.0, 0.0), point(0.0, 5.0, 0.0)),
                ViewKind::Right => (point(5.0, 0.0, 0.0), point(-5.0, 0.0, 0.0)),
                ViewKind::Perspective => {
                    let (_, _, forward) = viewport.perspective_basis();
                    let camera = -forward * viewport.perspective_camera_distance;
                    let near = camera + forward * 40.0;
                    let far = camera + forward * 60.0;
                    (point(near.x, near.y, near.z), point(far.x, far.y, far.z))
                }
            };
            let near_depth = viewport.view_depth(near);
            let far_depth = viewport.view_depth(far);
            let range = (near_depth.min(far_depth), near_depth.max(far_depth));
            let (_, near_gpu_depth) = gpu_project(&viewport, rect, near, range);
            let (_, far_gpu_depth) = gpu_project(&viewport, rect, far, range);
            assert!(
                near_gpu_depth < far_gpu_depth,
                "{kind:?}: near={near_gpu_depth}, far={far_gpu_depth}"
            );
        }
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
    fn front_view_point_cloud_picking_uses_the_xz_projection() {
        let viewport = Viewport::new(ViewKind::Front);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let target = document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![point(2.0, 100.0, 3.0)]).unwrap(),
            ))
            .unwrap();
        document
            .add_geometry(Geometry::PointCloud(
                PointCloud3::try_new(vec![point(2.0, 0.0, 8.0)]).unwrap(),
            ))
            .unwrap();
        let pointer = viewport.project(point(2.0, 0.0, 3.0), rect).unwrap();
        assert_eq!(viewport.pick_object(pointer, rect, &document), Some(target));
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
    fn wireframe_quad_picking_ignores_the_triangulation_diagonal() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let mesh_id = document
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new_faces(
                    vec![
                        point(-2.0, -2.0, 0.0),
                        point(2.0, -2.0, 0.0),
                        point(2.0, 2.0, 0.0),
                        point(-2.0, 2.0, 0.0),
                    ],
                    vec![viboceros_geometry::MeshFace::Quad([0, 1, 2, 3])],
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
        assert_eq!(shaded.pick_object(center, rect, &document), Some(mesh_id));
    }

    #[test]
    fn shaded_nurbs_surfaces_pick_from_their_display_tessellation() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let surface = NurbsSurface::try_bilinear([
            point(-2.0, -2.0, 0.0),
            point(2.0, -2.0, 0.0),
            point(2.0, 2.0, 1.0),
            point(-2.0, 2.0, 1.0),
        ])
        .unwrap();
        let surface_id = document
            .add_geometry(Geometry::NurbsSurface(surface.clone()))
            .unwrap();
        let viewport = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        let center = viewport.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            Viewport::default().pick_object(center, rect, &document),
            Some(surface_id)
        );
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

        let mut boundary_only = Document::default();
        boundary_only
            .add_geometry_with_attributes(
                Geometry::NurbsSurface(surface),
                ObjectAttributes::on_layer(boundary_only.current_layer_id())
                    .try_with_wire_density(-1)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            Viewport::default().pick_object(center, rect, &boundary_only),
            None
        );
    }

    #[test]
    fn shaded_breps_pick_faces_while_wireframe_uses_density_wires() {
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
            Some(brep_id)
        );
        let off_wire = Viewport::default()
            .project(point(1.0, 1.0, 0.0), rect)
            .unwrap();
        assert_eq!(
            Viewport::default().pick_object(off_wire, rect, &document),
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
    fn shaded_trimmed_brep_does_not_pick_through_an_inner_loop() {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let closed_rectangle = |min_x, min_y, max_x, max_y| {
            Polyline3::try_new(
                vec![
                    point(min_x, min_y, 0.0),
                    point(max_x, min_y, 0.0),
                    point(max_x, max_y, 0.0),
                    point(min_x, max_y, 0.0),
                    point(min_x, min_y, 0.0),
                ],
                Tolerance::DEFAULT,
            )
            .unwrap()
            .to_nurbs()
            .unwrap()
        };
        let outer = closed_rectangle(-3.0, -3.0, 3.0, 3.0);
        let hole = closed_rectangle(-1.0, -1.0, 1.0, 1.0);
        let brep_id = document
            .add_geometry(Geometry::Brep(
                Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
        let shaded = Viewport {
            display_mode: DisplayMode::Shaded,
            ..Viewport::default()
        };
        let material = shaded.project(point(2.0, 0.0, 0.0), rect).unwrap();
        let through_hole = shaded.project(point(0.0, 0.0, 0.0), rect).unwrap();

        assert_eq!(shaded.pick_object(material, rect, &document), Some(brep_id));
        assert_eq!(shaded.pick_object(through_hole, rect, &document), None);
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
                    grid_snap: false,
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
    fn front_view_osnap_uses_screen_projection() {
        let viewport = Viewport::new(ViewKind::Front);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let target_point = point(2.0, 100.0, 3.0);
        let target = document
            .add_geometry(Geometry::Point(target_point))
            .unwrap();
        document
            .add_geometry(Geometry::Point(point(2.0, 0.0, 8.0)))
            .unwrap();
        let pointer = viewport.project(target_point, rect).unwrap();
        let cursor = viewport
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: true,
                    smart_track: false,
                    grid_snap: false,
                    anchor: None,
                    reference: None,
                },
            )
            .unwrap();
        assert_eq!(cursor.point, target_point);
        assert_eq!(cursor.object_snap.unwrap().object_id(), target);
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
                    grid_snap: false,
                    anchor: Some(anchor),
                    reference: None,
                },
            )
            .unwrap();
        assert_eq!(cursor.point, point(3.0, 0.0, 5.0));
        assert_eq!(cursor.track.unwrap().axis(), TrackAxis::Horizontal);
    }

    #[test]
    fn drafting_cursor_uses_grid_snap_when_higher_priority_aids_do_not_capture() {
        let viewport = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let document = Document::default();
        let pointer = viewport.project(point(1.49, -2.49, 6.0), rect).unwrap();
        let cursor = viewport
            .drafting_cursor(
                pointer,
                rect,
                &document,
                DraftingInput {
                    active: true,
                    osnap: true,
                    smart_track: true,
                    grid_snap: true,
                    anchor: Some(point(8.0, 8.0, 6.0)),
                    reference: None,
                },
            )
            .unwrap();
        assert_eq!(cursor.point, point(1.0, -2.0, 6.0));
        assert!(cursor.grid_snapped);
        assert!(cursor.object_snap.is_none());
        assert!(cursor.track.is_none());
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
                    grid_snap: false,
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
