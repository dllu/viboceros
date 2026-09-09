//! Object filtering, click capture, and directional window selection.

use super::picking::PickHit;
use super::screen::{
    point_in_triangle, point_segment_distance, rect_corners, segment_intersects_rect,
};
use super::*;

#[derive(Default)]
pub(super) struct ProjectedPrimitives {
    points: Vec<Pos2>,
    pub(super) segments: Vec<[Pos2; 2]>,
    pub(super) triangles: Vec<[Pos2; 3]>,
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

    pub(super) fn is_crossed_by(&self, selection: Rect) -> bool {
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

impl Viewport {
    #[cfg(test)]
    pub(super) fn pick_object(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
    ) -> Option<ObjectId> {
        self.pick_object_matching(pointer, rect, document, ObjectSelectionFilter::Any)
    }

    pub(super) fn pick_object_matching(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        filter: ObjectSelectionFilter,
    ) -> Option<ObjectId> {
        let mut nearest: Option<(PickHit, ObjectId)> = None;
        for object in document.selectable_objects() {
            if !filter.accepts_object(object) {
                continue;
            }
            let hit = match object.geometry() {
                Geometry::Point(point) => {
                    let distance = self
                        .project(*point, rect)
                        .map_or(f32::INFINITY, |projected| {
                            point_segment_distance(pointer, projected, projected)
                        });
                    PickHit::screen(0, distance)
                }
                Geometry::PointCloud(cloud) => {
                    let distance = if let Some(projection) = self.point_cloud_projection() {
                        let origin = self.world_origin(rect);
                        let scale = Real::from(self.pixels_per_unit);
                        Point3::try_new(self.target.x, self.target.y, self.target.z)
                            .ok()
                            .and_then(|target| {
                                cloud
                                    .nearest_projected_relative(
                                        projection,
                                        target,
                                        [
                                            (Real::from(pointer.x) - Real::from(origin.x)) / scale,
                                            (Real::from(origin.y) - Real::from(pointer.y)) / scale,
                                        ],
                                        Real::from(PICK_CAPTURE_PIXELS) / scale,
                                    )
                                    .ok()
                                    .flatten()
                            })
                            .map_or(f32::INFINITY, |(_, _, distance)| {
                                (distance * Real::from(self.pixels_per_unit)) as f32
                            })
                    } else {
                        cloud
                            .points()
                            .iter()
                            .filter_map(|point| self.project(*point, rect))
                            .map(|projected| (projected - pointer).length())
                            .fold(f32::INFINITY, f32::min)
                    };
                    PickHit::screen(0, distance)
                }
                Geometry::Line(line) => {
                    let distance = self
                        .project_segment(line.start(), line.end(), rect)
                        .map_or(f32::INFINITY, |[start, end]| {
                            point_segment_distance(pointer, start, end)
                        });
                    PickHit::screen(1, distance)
                }
                Geometry::Circle(circle) => {
                    PickHit::screen(1, self.circle_pick_distance(pointer, rect, circle))
                }
                Geometry::Arc(arc) => {
                    PickHit::screen(1, self.arc_pick_distance(pointer, rect, arc))
                }
                Geometry::Ellipse(ellipse) => {
                    PickHit::screen(1, self.ellipse_pick_distance(pointer, rect, ellipse))
                }
                Geometry::Polyline(polyline) => {
                    PickHit::screen(1, self.polyline_pick_distance(pointer, rect, polyline))
                }
                Geometry::NurbsCurve(curve) => {
                    PickHit::screen(1, self.nurbs_pick_distance(pointer, rect, curve))
                }
                Geometry::PolyCurve(curve) => PickHit::screen(
                    1,
                    curve
                        .segments()
                        .iter()
                        .fold(f32::INFINITY, |distance, segment| {
                            distance.min(self.nurbs_pick_distance(pointer, rect, segment))
                        }),
                ),
                Geometry::NurbsSurface(surface) => self.nurbs_surface_pick(
                    pointer,
                    rect,
                    surface,
                    object.attributes().wire_density(),
                    document.tolerance(),
                ),
                Geometry::Brep(brep) => self.brep_pick(
                    pointer,
                    rect,
                    brep,
                    object.attributes().wire_density(),
                    document.tolerance(),
                ),
                Geometry::Mesh(mesh) => self.mesh_pick(pointer, rect, mesh, document.tolerance()),
            };
            if !hit.distance.is_finite() || hit.distance > PICK_CAPTURE_PIXELS {
                continue;
            }
            if nearest.is_none_or(|(best, _)| hit.is_better_than(best)) {
                nearest = Some((hit, object.id()));
            }
        }
        nearest.map(|(_, id)| id)
    }

    #[cfg(test)]
    pub(super) fn objects_in_selection(
        &self,
        viewport_rect: Rect,
        selection: Rect,
        crossing: bool,
        document: &Document,
    ) -> Vec<ObjectId> {
        self.objects_in_selection_matching(
            viewport_rect,
            selection,
            crossing,
            document,
            ObjectSelectionFilter::Any,
        )
    }

    pub(super) fn objects_in_selection_matching(
        &self,
        viewport_rect: Rect,
        selection: Rect,
        crossing: bool,
        document: &Document,
        filter: ObjectSelectionFilter,
    ) -> Vec<ObjectId> {
        document
            .selectable_objects()
            .filter(|object| filter.accepts_object(object))
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

    pub(super) fn projected_primitives(
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
            Geometry::Line(line) => {
                if let Some([start, end]) =
                    self.project_segment(line.start(), line.end(), viewport_rect)
                {
                    projected.add_segment(Some(start), Some(end));
                }
            }
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
                    if let Some([start, end]) =
                        self.project_segment(segment.start(), segment.end(), viewport_rect)
                    {
                        projected.add_segment(Some(start), Some(end));
                    }
                }
            }
            Geometry::NurbsCurve(curve) => {
                self.add_projected_nurbs_curve(&mut projected, viewport_rect, curve);
            }
            Geometry::PolyCurve(curve) => {
                for segment in curve.segments() {
                    self.add_projected_nurbs_curve(&mut projected, viewport_rect, segment);
                }
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
            let point = evaluate(sample as Real / samples as Real).ok();
            if let (Some(start), Some(end)) = (previous, point)
                && let Some([start, end]) = self.project_segment(start, end, rect)
            {
                projected.add_segment(Some(start), Some(end));
            }
            previous = point;
        }
    }

    pub(super) fn add_projected_nurbs_curve(
        &self,
        projected: &mut ProjectedPrimitives,
        rect: Rect,
        curve: &impl ViewportCurve,
    ) {
        let domain_end = *curve.domain().end();
        let samples = curve.samples_per_span();
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=samples {
                let fraction = sample as Real / samples as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                if sample == samples && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }
                let point = curve.evaluate(parameter).ok();
                if let (Some(start), Some(end)) = (previous, point)
                    && let Some([start, end]) = self.project_segment(start, end, rect)
                {
                    projected.add_segment(Some(start), Some(end));
                }
                previous = point;
            }
        }
    }

    pub(super) fn add_projected_mesh(
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
                if let Some([start, end]) = self.project_segment(line.start(), line.end(), rect) {
                    projected.add_segment(Some(start), Some(end));
                }
            }
        }
        if include_faces {
            for triangle in 0..mesh.triangles().len() {
                if let Some(points) = mesh.triangle_points(triangle) {
                    for points in self.clip_triangle(points).into_iter().flatten() {
                        projected.add_triangle(points.map(|point| self.project(point, rect)));
                    }
                }
            }
        }
    }

    pub(super) fn paint_selection_window(&self, painter: &egui::Painter, start: Pos2, end: Pos2) {
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

    pub(super) fn nurbs_pick_distance(
        &self,
        pointer: Pos2,
        rect: Rect,
        curve: &impl ViewportCurve,
    ) -> f32 {
        let domain_end = *curve.domain().end();
        let samples = curve.samples_per_span();
        let mut nearest = f32::INFINITY;
        for (span_start, span_end) in curve.spans() {
            let mut previous = None;
            for sample in 0..=samples {
                let fraction = sample as Real / samples as Real;
                let mut parameter = span_start.mul_add(1.0 - fraction, span_end * fraction);
                if sample == samples && span_end < domain_end {
                    parameter = span_end.next_down().max(span_start);
                }
                let projected = curve.evaluate(parameter).ok();
                if let (Some(start), Some(end)) = (previous, projected)
                    && let Some([start, end]) = self.project_segment(start, end, rect)
                {
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
            let projected = evaluate(sample as Real / samples as Real).ok();
            if let (Some(start), Some(end)) = (previous, projected)
                && let Some([start, end]) = self.project_segment(start, end, rect)
            {
                nearest = nearest.min(point_segment_distance(pointer, start, end));
            }
            previous = projected;
        }
        nearest
    }

    fn polyline_pick_distance(&self, pointer: Pos2, rect: Rect, polyline: &Polyline3) -> f32 {
        polyline
            .segments()
            .filter_map(|segment| self.project_segment(segment.start(), segment.end(), rect))
            .map(|[start, end]| point_segment_distance(pointer, start, end))
            .fold(f32::INFINITY, f32::min)
    }
}

pub(super) fn selection_mode(modifiers: egui::Modifiers) -> SelectionMode {
    if modifiers.command && !modifiers.shift {
        SelectionMode::Remove
    } else if modifiers.shift {
        SelectionMode::Add
    } else {
        SelectionMode::Replace
    }
}

pub(super) fn is_crossing_selection(start: Pos2, end: Pos2) -> bool {
    end.x < start.x
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::ColorRgb;

    #[test]
    #[ignore = "manual large-scene click-picking timing"]
    fn benchmark_large_scene_click_picking() {
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let mut document = Document::default();
        document.begin_transaction("fixture").unwrap();
        let mut first = None;
        for _ in 0..20_000 {
            let id = document
                .add_geometry(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()))
                .unwrap();
            first.get_or_insert(id);
        }
        document.commit_transaction().unwrap();
        let start = std::time::Instant::now();
        assert_eq!(view.pick_object(rect.center(), rect, &document), first);
        eprintln!("20k overlapping points, click pick: {:?}", start.elapsed());
    }

    #[test]
    fn grouped_filter_excludes_overlapping_ungrouped_hits_and_tracks_membership_edits() {
        for kind in [
            ViewKind::Top,
            ViewKind::Front,
            ViewKind::Right,
            ViewKind::Perspective,
        ] {
            let view = Viewport::new(kind);
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
            let mut document = Document::default();
            let point = Point3::try_new(0., 0., 0.).unwrap();
            let ungrouped = document.add_geometry(Geometry::Point(point)).unwrap();
            let grouped = document.add_geometry(Geometry::Point(point)).unwrap();
            let group = document.add_group(None, [grouped]).unwrap();
            let pointer = view.project(point, rect).unwrap();
            let selection = Rect::from_center_size(pointer, Vec2::splat(20.));
            let pick = |document: &Document| {
                view.pick_object_matching(pointer, rect, document, ObjectSelectionFilter::Grouped)
            };
            let before = format!("{document:?}");
            assert_eq!(view.pick_object(pointer, rect, &document), Some(ungrouped));
            assert_eq!(pick(&document), Some(grouped));
            for crossing in [false, true] {
                assert_eq!(
                    view.objects_in_selection_matching(
                        rect,
                        selection,
                        crossing,
                        &document,
                        ObjectSelectionFilter::Grouped
                    ),
                    [grouped]
                );
            }
            assert_eq!(format!("{document:?}"), before);
            document.set_objects_locked([grouped], true).unwrap();
            assert_eq!(pick(&document), None);
            document.set_objects_locked([grouped], false).unwrap();
            document.set_objects_visibility([grouped], false).unwrap();
            assert_eq!(pick(&document), None);
            document.undo().unwrap();
            assert_eq!(pick(&document), Some(grouped));
            document.remove_group(group).unwrap();
            assert_eq!(pick(&document), None);
            document.undo().unwrap();
            assert_eq!(pick(&document), Some(grouped));
            let layer = document.add_layer("Pick target", ColorRgb::BLACK).unwrap();
            document.set_objects_layer([grouped], layer).unwrap();
            for locked in [false, true] {
                if locked {
                    document.set_layer_locked(layer, true).unwrap();
                } else {
                    document.set_layer_visibility(layer, false).unwrap();
                }
                assert_eq!(pick(&document), None);
                assert_eq!(view.pick_object(pointer, rect, &document), Some(ungrouped));
                for crossing in [false, true] {
                    assert!(
                        view.objects_in_selection_matching(
                            rect,
                            selection,
                            crossing,
                            &document,
                            ObjectSelectionFilter::Grouped
                        )
                        .is_empty()
                    );
                }
                document.undo().unwrap();
                assert_eq!(pick(&document), Some(grouped));
            }
        }
    }

    #[test]
    fn long_visible_line_is_captured_at_its_screen_distance() {
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let id = document
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(
                    Point3::try_new(-1e30, 0.0, 0.0).unwrap(),
                    Point3::try_new(1e30, 0.0, 0.0).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            ))
            .unwrap();
        for x in [100.0, 400.0, 700.0] {
            for offset in [0.0, 3.0, 8.0, 9.0] {
                assert_eq!(
                    view.pick_object(Pos2::new(x, 300.0 + offset), rect, &document),
                    (offset <= PICK_CAPTURE_PIXELS).then_some(id)
                );
            }
        }
    }

    #[test]
    fn translated_point_cloud_capture_matches_screen_distance() {
        for kind in [ViewKind::Top, ViewKind::Front, ViewKind::Right] {
            let mut view = Viewport::new(kind);
            view.target = NaVector3::repeat(2.0_f64.powi(52));
            let point = Point3::try_new(view.target.x, view.target.y, view.target.z).unwrap();
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
            let mut document = Document::default();
            let id = document
                .add_geometry(Geometry::PointCloud(
                    viboceros_geometry::PointCloud3::try_new(vec![point]).unwrap(),
                ))
                .unwrap();
            let projected = view.project(point, rect).unwrap();
            for offset in [0.0, 7.0, 8.0, 9.0, 12.0] {
                assert_eq!(
                    view.pick_object(projected + Vec2::new(offset, 0.0), rect, &document),
                    (offset <= PICK_CAPTURE_PIXELS).then_some(id),
                    "offset={offset}",
                );
            }
        }
    }
}
