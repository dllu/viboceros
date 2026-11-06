//! Curve endpoint snaps and previews for the shared perpendicular getter.
use super::*;
use viboceros_command::line::perpendicular::Input;
impl Viewport {
    pub(super) fn perpendicular_curve_hit(
        &self,
        pointer: Pos2,
        rect: Rect,
        doc: &Document,
        getter: &Input,
    ) -> Option<(ObjectId, Option<usize>, Point3)> {
        if !getter.endpoint() || !getter.accepts_curve() {
            return None;
        }
        let target = self
            .pick_object_candidates_matching_preview_excluding(
                pointer,
                rect,
                doc,
                viboceros_command::ObjectSelectionFilter::Curves,
                None,
                None,
            )
            .into_iter()
            .next()
            .map(|id| (id, None))
            .or_else(|| {
                self.pick_edges(pointer, rect, doc)
                    .first()
                    .map(|e| (e.object, Some(e.edge)))
            })?;
        let point = self.curve_pick_point(
            doc.object(target.0)?.geometry(),
            target.1,
            pointer,
            doc.tolerance(),
        )?;
        Some((target.0, target.1, point))
    }
    pub(super) fn paint_perpendicular(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        doc: &Document,
        getter: &Input,
        cursor: Option<Point3>,
        hit: Option<(ObjectId, Option<usize>, Point3)>,
    ) {
        if let Some(anchor) = getter.anchor() {
            self.paint_draft_points(painter, rect, &[anchor]);
        }
        let definition = if let Some((id, edge, pick)) = hit {
            getter
                .clone()
                .pick_curve(doc, id, edge, pick)
                .ok()
                .flatten()
        } else {
            cursor.and_then(|p| getter.preview(p))
        };
        if let Some(d) = definition
            && let (Some(a), Some(b)) = (
                self.project_selection_point(d.line.start(), rect),
                self.project_selection_point(d.line.end(), rect),
            )
        {
            painter.line_segment([a, b], Stroke::new(2., Color32::from_rgb(20, 115, 190)));
        }
    }
}
