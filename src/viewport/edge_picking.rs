//! Screen-space boundary components, separate from object selection and CPlane input.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgePick {
    pub object: ObjectId,
    pub edge: usize,
}

impl Viewport {
    pub(super) fn paint_edge_analysis(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        view: &viboceros_command::edge_analysis::View,
    ) {
        let color = Color32::from_rgb(view.color[0], view.color[1], view.color[2]);
        let mut cache = self.display_cache.borrow_mut();
        let segments = cache.analysis_segments(&view.edges);
        for edge in view.displayed().filter(|e| {
            document.object(e.object).is_some_and(|o| {
                o.attributes().is_visible()
                    && document
                        .layer(o.attributes().layer_id())
                        .is_some_and(|l| l.is_visible())
            })
        }) {
            if let Some(lines) = segments.get(&(edge.object, edge.index)) {
                for &[a, b] in lines {
                    if let Some([a, b]) = self.project_selection_segment(a, b, rect) {
                        painter.line_segment([a, b], Stroke::new(3., color));
                    }
                }
            }
            for point in edge.endpoints {
                if let Some(p) = self.project_selection_point(point, rect) {
                    painter.circle_filled(p, 4., color);
                }
            }
        }
    }

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
                    .map(|&[a, b]| self.selection_line_distance(pointer, a, b, rect))
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
                if let Some([a, b]) = self.project_selection_segment(a, b, rect) {
                    painter.line_segment([a, b], Stroke::new(3., Color32::from_rgb(245, 160, 20)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
