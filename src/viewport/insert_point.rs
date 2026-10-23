//! Original control-edge capture with a constrained spatial point, independent of CPlane.
use super::*;
use viboceros_geometry::{LineSegment, PolygonEdgePoint};
#[derive(Clone, Copy)]
pub(super) struct InsertPointCursor {
    pub pick: PolygonEdgePoint,
    constrained: edge_point::EdgePointCursor,
}
impl Viewport {
    pub(super) fn insert_point_cursor(
        &self,
        edges: &[LineSegment],
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        osnap: impl Into<viboceros_drafting::ObjectSnapOptions>,
    ) -> Option<InsertPointCursor> {
        if !rect.contains(pointer) {
            return None;
        }
        let (_, edge) = edges
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                let distance =
                    self.selection_line_distance(pointer, line.start(), line.end(), rect);
                (distance.is_finite() && distance <= PICK_CAPTURE_PIXELS).then_some((distance, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))?;
        let curve = edges[edge].to_nurbs().ok()?;
        let constrained = self.edge_point_cursor(&curve, None, pointer, rect, document, osnap)?;
        let domain = curve.domain();
        Some(InsertPointCursor {
            pick: PolygonEdgePoint {
                edge,
                parameter: ((constrained.parameter - domain.start())
                    / (domain.end() - domain.start()))
                .clamp(0., 1.),
            },
            constrained,
        })
    }
    pub(super) fn paint_insert_edges(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        edges: &[LineSegment],
        cursor: Option<InsertPointCursor>,
    ) {
        for line in edges {
            if let Some(segment) = self.project_segment(line.start(), line.end(), rect) {
                painter.line_segment(segment, Stroke::new(1., Color32::from_gray(150)));
            }
        }
        if let Some(cursor) = cursor
            && let Ok(curve) = edges[cursor.pick.edge].to_nurbs()
        {
            self.paint_edge_point_cursor(painter, rect, &curve, cursor.constrained);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::preview_test_support::frame;
    use super::*;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn constrained_clicks_follow_spatial_edges_in_parallel_and_perspective_views() {
        for kind in [ViewKind::Top, ViewKind::Front, ViewKind::Perspective] {
            let line =
                LineSegment::try_new(p(-1., -0.4, 0.6), p(2., 0.8, 2.4), Tolerance::default())
                    .unwrap();
            let edges = [line];
            let mut doc = Document::default();
            doc.add_geometry(Geometry::Line(line)).unwrap();
            let context = egui::Context::default();
            let mut view = Viewport::new(kind);
            frame(
                &context,
                &mut view,
                &doc,
                ViewportInput {
                    insert_edges: Some(&edges),
                    ..Default::default()
                },
                vec![],
            );
            let target = line.point_at(0.37).unwrap();
            let pixel = view.project(target, view.last_rect.unwrap()).unwrap();
            let output = frame(
                &context,
                &mut view,
                &doc,
                ViewportInput {
                    insert_edges: Some(&edges),
                    ..Default::default()
                },
                vec![
                    egui::Event::PointerMoved(pixel),
                    egui::Event::PointerButton {
                        pos: pixel,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                    egui::Event::PointerButton {
                        pos: pixel,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: Default::default(),
                    },
                ],
            );
            let pick = output.insert_point.unwrap().0;
            assert!(
                line.point_at(pick.parameter)
                    .unwrap()
                    .distance_to(target)
                    .unwrap()
                    < 1e-5,
                "{kind:?}"
            );
            assert!(
                output.picked_point.is_none()
                    && output.selection_click.is_none()
                    && output.component_click.is_none()
            );
        }
    }
    #[test]
    fn off_edge_and_outside_clicks_do_not_return_a_plane_point() {
        let view = Viewport::new(ViewKind::Top);
        let edges =
            [LineSegment::try_new(p(0., 0., 2.), p(1., 0., 3.), Tolerance::default()).unwrap()];
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let document = Document::default();
        let modes = viboceros_drafting::ObjectSnapModes::NONE;
        assert!(
            view.insert_point_cursor(&edges, Pos2::new(-1., 300.), rect, &document, modes)
                .is_none()
        );
        assert!(
            view.insert_point_cursor(&edges, Pos2::new(10., 10.), rect, &document, modes)
                .is_none()
        );
    }
    #[test]
    fn feature_snaps_project_onto_the_chosen_edge_without_inventing_a_plane_point() {
        use viboceros_drafting::{ObjectSnapKind, ObjectSnapModes};
        let view = Viewport::new(ViewKind::Top);
        let edges =
            [LineSegment::try_new(p(-2., 0., 2.), p(2., 0., 4.), Tolerance::default()).unwrap()];
        let mut document = Document::default();
        let feature = p(0., 0.02, 3.5);
        document.add_geometry(Geometry::Point(feature)).unwrap();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let pointer = view.project(feature, rect).unwrap();
        let free = view
            .insert_point_cursor(&edges, pointer, rect, &document, ObjectSnapModes::NONE)
            .unwrap()
            .pick;
        let snapped = view
            .insert_point_cursor(
                &edges,
                pointer,
                rect,
                &document,
                ObjectSnapModes::only(ObjectSnapKind::Point),
            )
            .unwrap()
            .pick;
        let expected = edges[0]
            .closest_parameter(feature, Tolerance::MESH_VALIDATION)
            .unwrap();
        assert!((snapped.parameter - expected).abs() < 1e-7);
        assert!((free.parameter - snapped.parameter).abs() > 0.01);
        assert_ne!(edges[0].point_at(snapped.parameter).unwrap(), feature);
    }
}
