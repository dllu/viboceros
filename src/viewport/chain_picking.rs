//! Mixed standalone curve and surface-edge picks retain explicit ambiguity.
use super::*;
use viboceros_command::chain_selection::Target;
impl Viewport {
    pub(super) fn pick_chain_targets(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        polygon_edges: bool,
    ) -> Vec<Target> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for object in document.selectable_objects() {
            let geo = object.geometry();
            if geo.curve_ref().is_none()
                && !matches!(geo, Geometry::Brep(_) | Geometry::NurbsSurface(_))
                && !(polygon_edges && matches!(geo, Geometry::Mesh(_) | Geometry::SubD(_)))
            {
                continue;
            }
            let display = self.display_cache.borrow_mut().get(
                object,
                document.tolerance(),
                document.subd_control_net(),
            );
            if geo.curve_ref().is_some() {
                let hit = self.pick_display_geometry(pointer, rect, &display, document.tolerance());
                if hit.distance.is_finite() && hit.distance <= PICK_CAPTURE_PIXELS {
                    hits.push((hit.distance, Target::Curve(object.id())));
                }
            } else {
                for (edge, segments) in display.edges().iter().enumerate() {
                    let distance = segments
                        .iter()
                        .map(|&[a, b]| self.selection_line_distance(pointer, a, b, rect))
                        .fold(f32::INFINITY, f32::min);
                    if distance.is_finite() && distance <= PICK_CAPTURE_PIXELS {
                        hits.push((distance, Target::Edge(object.id(), edge)));
                    }
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let Some((best, _)) = hits.first() else {
            return Vec::new();
        };
        hits.iter()
            .filter(|(distance, _)| distance - best <= 0.75)
            .map(|(_, t)| *t)
            .collect()
    }
    pub(super) fn paint_chain_targets(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        doc: &Document,
        targets: &[Target],
    ) {
        let curves = targets
            .iter()
            .filter_map(|&t| {
                if let Target::Curve(object) = t {
                    Some(ParametricPick {
                        object,
                        member: None,
                        face: None,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let edges = targets
            .iter()
            .filter_map(|&t| {
                if let Target::Edge(object, edge) = t {
                    Some(EdgePick { object, edge })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        self.paint_parametric_highlights(painter, rect, doc, &curves);
        self.paint_edge_highlights(painter, rect, doc, &edges);
    }
}
