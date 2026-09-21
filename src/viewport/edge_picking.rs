//! Screen-space boundary components, separate from object selection and CPlane input.
use super::screen::point_segment_distance;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgePick {
    pub object: ObjectId,
    pub edge: usize,
}

impl Viewport {
    pub(super) fn pick_edges(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
    ) -> Vec<EdgePick> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for object in document.selectable_objects() {
            if !matches!(
                object.geometry(),
                Geometry::Brep(_) | Geometry::NurbsSurface(_)
            ) {
                continue;
            }
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            for (edge, segments) in display.edges().iter().enumerate() {
                let distance = segments
                    .iter()
                    .filter_map(|&[a, b]| self.project_segment(a, b, rect))
                    .map(|[a, b]| point_segment_distance(pointer, a, b))
                    .fold(f32::INFINITY, f32::min);
                if distance.is_finite() && distance <= PICK_CAPTURE_PIXELS {
                    hits.push((
                        distance,
                        EdgePick {
                            object: object.id(),
                            edge,
                        },
                    ));
                }
            }
        }
        // Keep all captured components; depth or table order must not silently
        // decide which overlapping edge is modified. Stable distance order makes
        // the explicit ambiguity prompt predictable for an unchanged document.
        hits.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then(a.1.object.cmp(&b.1.object))
                .then(a.1.edge.cmp(&b.1.edge))
        });
        hits.into_iter().map(|(_, pick)| pick).collect()
    }

    pub(super) fn paint_edge_highlights(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        picks: &[EdgePick],
    ) {
        for pick in picks {
            let Some(object) = document
                .object(pick.object)
                .filter(|_| document.is_object_selectable(pick.object))
            else {
                continue;
            };
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            let Some(segments) = display.edges().get(pick.edge) else {
                continue;
            };
            for &[a, b] in segments {
                if let Some([a, b]) = self.project_segment(a, b, rect) {
                    painter.line_segment([a, b], Stroke::new(3., Color32::from_rgb(245, 160, 20)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
