//! Surface-station labels share projection and hit testing with the viewport.
use super::*;
use viboceros_command::soft_crease::SoftCreaseLabel;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Overlay<'a> {
    pub labels: &'a [SoftCreaseLabel],
    pub text_size: f32,
    pub highlighted: Option<(usize, usize)>,
}
impl Viewport {
    pub(super) fn paint_soft_crease(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        pointer: Option<Pos2>,
        overlay: Overlay<'_>,
    ) -> Option<(usize, usize)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut picked = None;
        for label in overlay.labels {
            if !seen.insert((label.object, label.vertex)) {
                continue;
            }
            let Some(point) = self
                .project(label.point, rect)
                .filter(|p| rect.contains(*p))
            else {
                continue;
            };
            let highlighted = overlay.highlighted == Some((label.chain, label.vertex));
            let color = if highlighted {
                Color32::from_rgb(240, 105, 25)
            } else {
                Color32::from_rgb(40, 155, 100)
            };
            painter.circle_filled(point, if highlighted { 4. } else { 3. }, color);
            let text = painter.text(
                point + Vec2::new(5., -5.),
                Align2::LEFT_BOTTOM,
                format!("{:.2}", label.weight),
                FontId::proportional(overlay.text_size),
                color,
            );
            if picked.is_none()
                && pointer.is_some_and(|p| p.distance(point) <= 12. || text.expand(3.).contains(p))
            {
                picked = Some((label.chain, label.vertex));
            }
        }
        picked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        context: &egui::Context,
        view: &mut Viewport,
        doc: &Document,
        labels: &[SoftCreaseLabel],
        events: Vec<egui::Event>,
    ) -> ViewportOutput {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    output = view.show(
                        ui,
                        doc,
                        ViewportInput {
                            soft_crease: Some(Overlay {
                                labels,
                                text_size: 14.,
                                highlighted: None,
                            }),
                            ..Default::default()
                        },
                        &[],
                        0,
                        true,
                    );
                },
            )
            .drop_without_applying_deltas();
        output
    }
    #[test]
    fn soft_crease_labels_accept_projected_clicks_and_choose_the_first_shared_chain() {
        let mut doc = Document::default();
        let point = Point3::try_new(0., 0., 0.).unwrap();
        let object = doc.add_geometry(Geometry::Point(point)).unwrap();
        let context = egui::Context::default();
        let mut view = Viewport::new(ViewKind::Top);
        let label = SoftCreaseLabel {
            object,
            chain: 0,
            vertex: 1,
            point,
            weight: 50.,
        };
        let labels = [label, SoftCreaseLabel { chain: 1, ..label }];
        frame(&context, &mut view, &doc, &labels, vec![]);
        let pointer = view.project(point, view.last_rect.unwrap()).unwrap();
        let events = vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        let result = frame(&context, &mut view, &doc, &labels, events);
        assert_eq!(result.soft_crease_vertex, Some((0, 1)));
        assert_eq!(doc.selected_object_count(), 0);
    }
}
