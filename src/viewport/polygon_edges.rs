//! Mesh/SubD edges and SubD vertices share clipping, capture and cached samples.
use super::*;
use viboceros_command::ComponentSelectionKind;
use viboceros_document::Object;

impl super::display_cache::DisplayGeometry {
    pub(super) fn polygon_vertex_points<'a>(
        &'a self,
        document: &'a Document,
        object: &'a Object,
    ) -> &'a [Point3] {
        match &*self.geometry {
            Geometry::Mesh(mesh) => self
                .topology_vertices
                .get_or_init(|| mesh.topology_vertex_points()),
            Geometry::SubD(subd) => document
                .control_point_locations(object.id())
                .unwrap_or_else(|| subd.surface_points()),
            _ => &[],
        }
    }
}
fn vertex_kind(object: &Object) -> Option<ComponentSelectionKind> {
    match object.geometry() {
        Geometry::Mesh(_) => Some(ComponentSelectionKind::MeshVertex),
        Geometry::SubD(_) => Some(ComponentSelectionKind::SubDVertex),
        _ => None,
    }
}
fn kind(object: &Object) -> Option<ComponentSelectionKind> {
    match object.geometry() {
        Geometry::Mesh(_) => Some(ComponentSelectionKind::MeshEdge),
        Geometry::SubD(_) => Some(ComponentSelectionKind::SubDEdge),
        Geometry::Brep(_) | Geometry::NurbsSurface(_) => Some(ComponentSelectionKind::BrepEdge),
        _ => None,
    }
}
impl Viewport {
    fn visit_component_edges(
        &self,
        document: &Document,
        object: &Object,
        mut visit: impl FnMut(usize, &[[Point3; 2]]),
    ) {
        if let Geometry::SubD(subd) = object.geometry()
            && let Some(points) = document.control_point_locations(object.id())
        {
            for (index, &[a, b]) in subd.edges().iter().enumerate() {
                if let (Some(&a), Some(&b)) = (points.get(a as usize), points.get(b as usize)) {
                    visit(index, &[[a, b]]);
                }
            }
            return;
        }
        let display = self
            .display_cache
            .borrow_mut()
            .get(object, document.tolerance());
        for (index, segments) in display.edges().iter().enumerate() {
            visit(index, segments);
        }
    }
    pub(super) fn pick_component_edges(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        breps: bool,
    ) -> Vec<ComponentPick> {
        let mut hits = Vec::new();
        for object in document.selectable_objects() {
            let Some(kind) =
                kind(object).filter(|&k| breps || k != ComponentSelectionKind::BrepEdge)
            else {
                continue;
            };
            self.visit_component_edges(document, object, |index, segments| {
                let distance = segments
                    .iter()
                    .map(|&[a, b]| self.selection_line_distance(pointer, a, b, rect))
                    .fold(f32::INFINITY, f32::min);
                if distance.is_finite() && distance <= PICK_CAPTURE_PIXELS {
                    hits.push((
                        distance,
                        ComponentPick {
                            object: object.id(),
                            kind,
                            index,
                        },
                    ));
                }
            });
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        hits.into_iter().map(|(_, p)| p).collect()
    }
    pub(super) fn pick_polygon_vertices(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
    ) -> Vec<ComponentPick> {
        let mut hits = Vec::new();
        for object in document.selectable_objects() {
            let Some(kind) = vertex_kind(object) else {
                continue;
            };
            // Mesh grips use raw vertex ordinals, while component vertices use
            // welded topology ordinals. The existing grip picker owns that mode.
            if kind == ComponentSelectionKind::MeshVertex
                && document.grip_mode(object.id()).is_some()
            {
                continue;
            }
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            let hidden = document
                .hidden_control_point_indices(object.id())
                .into_iter()
                .flatten()
                .collect::<std::collections::BTreeSet<_>>();
            for (index, &point) in display
                .polygon_vertex_points(document, object)
                .iter()
                .enumerate()
            {
                if hidden.contains(&index) {
                    continue;
                }
                if let Some(screen) = self.project_selection_point(point, rect) {
                    let distance = screen.distance(pointer);
                    if distance.is_finite() && distance <= PICK_CAPTURE_PIXELS {
                        hits.push((
                            distance,
                            ComponentPick {
                                object: object.id(),
                                kind,
                                index,
                            },
                        ));
                    }
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        hits.into_iter().map(|(_, p)| p).collect()
    }
    pub(super) fn polygon_edges_in_rectangle(
        &self,
        viewport: Rect,
        selection: Rect,
        document: &Document,
        object: &Object,
        crossing: bool,
        inverted: bool,
    ) -> Vec<ComponentPick> {
        let mut result = Vec::new();
        let selected = |p: &ProjectedPrimitives| match (crossing, inverted) {
            (false, false) => p.is_windowed_by(selection),
            (true, false) => p.is_crossed_by(selection),
            (false, true) => !p.is_crossed_by(selection),
            (true, true) => !p.is_windowed_by(selection),
        };
        let hidden = document
            .hidden_control_point_indices(object.id())
            .into_iter()
            .flatten()
            .collect::<std::collections::BTreeSet<_>>();
        let display = self
            .display_cache
            .borrow_mut()
            .get(object, document.tolerance());
        let points = if matches!(object.geometry(), Geometry::Mesh(_))
            && document.grip_mode(object.id()).is_some()
        {
            &[][..]
        } else {
            display.polygon_vertex_points(document, object)
        };
        for (index, &point) in points.iter().enumerate() {
            if hidden.contains(&index) {
                continue;
            }
            let mut p = ProjectedPrimitives::default();
            self.add_selection_point(&mut p, point, viewport);
            if selected(&p) {
                result.push(ComponentPick {
                    object: object.id(),
                    kind: vertex_kind(object).unwrap(),
                    index,
                });
            }
        }
        let Some(kind) = kind(object) else {
            return result;
        };
        self.visit_component_edges(document, object, |index, segments| {
            let mut p = ProjectedPrimitives::default();
            for &[a, b] in segments {
                self.add_selection_segment(&mut p, a, b, viewport);
            }
            if selected(&p) {
                result.push(ComponentPick {
                    object: object.id(),
                    kind,
                    index,
                });
            }
        });
        result
    }
    pub(super) fn paint_polygon_edge(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        object: &Object,
        index: usize,
    ) {
        // Use the same curve/cage representation as the picker.
        if let Geometry::SubD(subd) = object.geometry()
            && let Some(points) = document.control_point_locations(object.id())
        {
            if let Some(&[a, b]) = subd.edges().get(index)
                && let (Some(&a), Some(&b)) = (points.get(a as usize), points.get(b as usize))
                && let Some(segment) = self.project_selection_segment(a, b, rect)
            {
                painter.line_segment(segment, Stroke::new(3., Color32::from_rgb(245, 160, 20)));
            }
            return;
        }
        let display = self
            .display_cache
            .borrow_mut()
            .get(object, document.tolerance());
        if let Some(segments) = display.edges().get(index) {
            for &[a, b] in segments {
                if let Some(segment) = self.project_selection_segment(a, b, rect) {
                    painter.line_segment(segment, Stroke::new(3., Color32::from_rgb(245, 160, 20)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
