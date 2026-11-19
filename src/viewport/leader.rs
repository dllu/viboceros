//! Leader arrowheads, landings, and model-sized text for the viewport.
use super::*;
use egui::epaint::TextShape;
use leader_arrow::ArrowPiece;
use std::sync::Arc;
use viboceros_geometry::Leader;

struct TextLayout {
    galley: Arc<egui::Galley>,
    origin: Pos2,
    angle: f32,
    bounds: Rect,
}

impl Viewport {
    pub(super) fn paint_scene_leaders(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
    ) {
        if let Some(scene) = self.cached_scene.borrow().as_ref() {
            for (leader, color) in &scene.leaders {
                self.paint_leader_annotation(painter, rect, document, leader, *color);
            }
        }
    }

    pub(super) fn paint_leader_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        leader: &Leader,
        color: Color32,
    ) {
        leader.visit_segments(|start, end| {
            if let Some([a, b]) = self.project_segment(start, end, rect) {
                painter.line_segment([a, b], Stroke::new(1.5, color));
            }
        });
        self.paint_leader_annotation(painter, rect, document, leader, color);
    }

    fn paint_leader_annotation(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        leader: &Leader,
        color: Color32,
    ) {
        let mut arrow = egui::Mesh::default();
        let mut append_triangle = |triangle: [Pos2; 3], color| {
            let first = arrow.vertices.len() as u32;
            for point in triangle {
                arrow.colored_vertex(point, color);
            }
            arrow.add_triangle(first, first + 1, first + 2);
        };
        for triangle in self.projected_leader_arrow(leader, rect) {
            append_triangle(triangle, color);
        }
        let mut shapes = Vec::new();
        self.visit_custom_leader_arrow(document, leader, color, |piece, color| match piece {
            ArrowPiece::Point(point) => {
                if let Some(point) = self.project_selection_point(point, rect) {
                    shapes.push(egui::Shape::circle_filled(point, 3., color));
                }
            }
            ArrowPiece::Segment([a, b]) => {
                if let Some(segment) = self.project_selection_segment(a, b, rect) {
                    shapes.push(egui::Shape::line_segment(segment, Stroke::new(1.5, color)));
                }
            }
            ArrowPiece::Triangle(face) => {
                for triangle in self.project_custom_arrow_face(face, rect) {
                    append_triangle(triangle, color);
                }
            }
        });
        if !arrow.indices.is_empty() {
            painter.add(egui::Shape::mesh(arrow));
        }
        painter.extend(shapes);
        if let Some(text) = self.leader_text_layout(painter.ctx(), leader, rect)
            && rect.intersects(text.bounds)
        {
            painter.add(TextShape::new(text.origin, text.galley, color).with_angle(text.angle));
        }
    }

    pub(super) fn projected_leader_arrow<'a>(
        &'a self,
        leader: &'a Leader,
        rect: Rect,
    ) -> impl Iterator<Item = [Pos2; 3]> + 'a {
        leader
            .arrow_triangles()
            .flat_map(move |triangle| self.clip_triangle(triangle).into_iter().flatten())
            .filter_map(move |[a, b, c]| {
                Some([
                    self.project(a, rect)?,
                    self.project(b, rect)?,
                    self.project(c, rect)?,
                ])
            })
    }

    fn leader_text_layout(
        &self,
        context: &egui::Context,
        leader: &Leader,
        rect: Rect,
    ) -> Option<TextLayout> {
        let (at, outward) = leader.text_attachment()?;
        let outward = outward.as_vector();
        let appearance = leader.style().leader_appearance();
        let up = leader.plane().z_axis().as_vector().cross(outward).ok()?;
        let height_point = at
            .translated(up.scaled(appearance.text_height).ok()?)
            .ok()?;
        let text_axis_point = at
            .translated(outward.scaled(appearance.text_height).ok()?)
            .ok()?;
        let anchor = self.project(at, rect)?;
        let size = self.project(height_point, rect)?.distance(anchor);
        let projected_axis = self.project(text_axis_point, rect)? - anchor;
        if leader.text().is_empty()
            || !size.is_finite()
            || size < 0.5
            || !projected_axis.is_finite()
            || projected_axis.length_sq() <= 1.0e-6
        {
            return None;
        }
        let direction = projected_axis.normalized();
        let right = if appearance.draws_forward() && direction.x < 0.0 {
            -direction
        } else {
            direction
        };
        let angle = right.y.atan2(right.x);
        let galley = context.fonts_mut(|fonts| {
            fonts.layout_no_wrap(
                leader.text().into(),
                FontId::proportional(size.clamp(3.0, 256.0)),
                Color32::PLACEHOLDER,
            )
        });
        let extent = galley.size();
        let down = egui::vec2(-right.y, right.x);
        let center = anchor + direction * (extent.x * 0.5);
        let origin = center - (right * extent.x + down * extent.y) * 0.5;
        let bounds = Rect::from_center_size(center, right.abs() * extent.x + down.abs() * extent.y);
        Some(TextLayout {
            galley,
            origin,
            angle,
            bounds,
        })
    }

    pub(super) fn leader_text_bounds(&self, leader: &Leader, rect: Rect) -> Option<Rect> {
        let tail = *leader.path().vertices().last()?;
        if !self.point_within_display_depth(tail) {
            return None;
        }
        self.leader_text_layout(self.annotation_fonts.as_ref()?, leader, rect)
            .map(|text| text.bounds)
    }
}
