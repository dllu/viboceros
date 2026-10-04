//! Visible grip drawing and point picking use the display frustum.
use super::*;
use viboceros_document::ControlPointId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlPointSelection {
    pub picks: Vec<ControlPointId>,
    pub mode: SelectionMode,
}

impl Viewport {
    pub(super) fn pick_control_point(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
    ) -> Option<ControlPointId> {
        document
            .control_points()
            .filter_map(|(id, point, _)| {
                let screen = self.project_selection_point(point, rect)?;
                let distance = screen.distance_sq(pointer);
                (rect.contains(screen) && distance <= PICK_CAPTURE_PIXELS * PICK_CAPTURE_PIXELS)
                    .then_some((id, distance, self.view_depth(point)))
            })
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then(a.2.total_cmp(&b.2))
                    .then(a.0.cmp(&b.0))
            })
            .map(|(id, _, _)| id)
    }

    pub(super) fn control_points_in_window(
        &self,
        rect: Rect,
        window: Rect,
        inverted: bool,
        document: &Document,
    ) -> Vec<ControlPointId> {
        document
            .control_points()
            .filter_map(|(id, point, _)| {
                let screen = self.project_selection_point(point, rect)?;
                (rect.contains(screen) && (window.contains(screen) != inverted)).then_some(id)
            })
            .collect()
    }

    pub(super) fn paint_control_points(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        preview: Option<super::object_preview::TransformedObjects<'_>>,
    ) {
        let moving_ids = preview
            .map(|p| {
                p.grips
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        for (id, point, selected) in document.control_points() {
            let moving = preview.filter(|_| moving_ids.contains(&id));
            let posed = moving.and_then(|preview| preview.transform.transform_point(point).ok());
            let point = if moving.is_some_and(|p| !p.copy) {
                posed.unwrap_or(point)
            } else {
                point
            };
            if let Some(screen) = self
                .project_selection_point(point, rect)
                .filter(|p| rect.contains(*p))
            {
                let color = if selected {
                    SELECTED_COLOR
                } else {
                    Color32::from_rgb(180, 75, 210)
                };
                painter.rect_filled(Rect::from_center_size(screen, Vec2::splat(6.)), 0., color);
            }
            if moving.is_some_and(|p| p.copy)
                && let Some(screen) = posed
                    .and_then(|p| self.project_selection_point(p, rect))
                    .filter(|p| rect.contains(*p))
            {
                painter.rect_filled(
                    Rect::from_center_size(screen, Vec2::splat(6.)),
                    0.,
                    SELECTED_COLOR,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grip_picking_filters_hidden_sources_and_obeys_window_inversion() {
        let mut document = Document::default();
        let points = [
            Point3::try_new(-2., 0., 0.).unwrap(),
            Point3::try_new(0., 2., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ];
        let source = document
            .add_geometry(Geometry::NurbsCurve(
                NurbsCurve::try_clamped_uniform(2, points.to_vec()).unwrap(),
            ))
            .unwrap();
        document.enable_control_points([source]).unwrap();
        let mut view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        view.refresh_clipping(&document, rect).unwrap();
        let screen = view.project(points[1], rect).unwrap();
        let window = Rect::from_center_size(screen, Vec2::splat(5.));
        let pick = ControlPointId {
            object: source,
            index: 1,
        };
        assert_eq!(view.pick_control_point(screen, rect, &document), Some(pick));
        assert_eq!(
            view.control_points_in_window(rect, window, false, &document),
            [pick]
        );
        assert_eq!(
            view.control_points_in_window(rect, window, true, &document)
                .len(),
            2
        );
        document.set_objects_visibility([source], false).unwrap();
        assert_eq!(view.pick_control_point(screen, rect, &document), None);
    }
}
