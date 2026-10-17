//! Read-only circle contact and final arc-choice previews.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct TangentPreview<'a> {
    pub contacts: &'a viboceros_command::circle_tangent_input::Input,
    pub output: Option<viboceros_command::circle_deform_input::Output>,
    pub arc: Option<&'a viboceros_command::arc_tangent_input::Support>,
}
impl Viewport {
    pub(super) fn paint_tangent_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        input: TangentPreview<'_>,
        pointer: Option<Pos2>,
        cursor: Option<Point3>,
    ) {
        let getter = input.contacts;
        let curve = if let Some(support) = input.arc {
            self.paint_draft_points(painter, rect, &support.contact_points);
            cursor
                .and_then(|p| support.choose(p, document.tolerance()).ok())
                .and_then(|a| a.to_nurbs().ok())
        } else {
            if let Some(anchor) = getter.anchor() {
                self.paint_draft_points(painter, rect, &[anchor]);
            }
            let circle = if getter.wants_point() {
                cursor.and_then(|p| getter.preview_point(p))
            } else if getter.wants_curve() {
                pointer
                    .and_then(|pointer| {
                        let reference = self
                            .pick_edges(pointer, rect, document)
                            .first()
                            .map(|p| (p.object, Some(p.edge)))
                            .or_else(|| {
                                self.pick_object_candidates_matching_preview_excluding(
                                    pointer,
                                    rect,
                                    document,
                                    ObjectSelectionFilter::Curves,
                                    None,
                                    None,
                                )
                                .first()
                                .map(|id| (*id, None))
                            })?;
                        let geometry = document.object(reference.0)?.geometry();
                        let point = self.curve_pick_point(
                            geometry,
                            reference.1,
                            pointer,
                            document.tolerance(),
                        )?;
                        getter.preview_curve(document, reference.0, reference.1, point)
                    })
                    .or_else(|| getter.preview_current())
            } else {
                getter.preview_current()
            };
            circle.and_then(|c| {
                input
                    .output
                    .map_or_else(|| c.to_nurbs(), |o| o.curve(c))
                    .ok()
            })
        };
        if let Some(curve) = curve {
            let mut projected = ProjectedPrimitives::default();
            self.add_projected_nurbs_curve(&mut projected, rect, &curve);
            for segment in projected.segments {
                painter.line_segment(segment, Stroke::new(2., Color32::from_rgb(20, 115, 190)));
            }
        }
    }
}
