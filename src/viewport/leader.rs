//! Leader arrowheads, landings, and model-sized text for the viewport.
use super::*;
use egui::epaint::TextShape;
use std::sync::Arc;
use viboceros_geometry::Leader;

struct TextLayout {
    galley: Arc<egui::Galley>,
    origin: Pos2,
    angle: f32,
    bounds: Rect,
}

impl Viewport {
    pub(super) fn paint_scene_leaders(&self, painter: &egui::Painter, rect: Rect) {
        if let Some(scene) = self.cached_scene.borrow().as_ref() {
            for (leader, color) in &scene.leaders {
                self.paint_leader_annotation(painter, rect, leader, *color);
            }
        }
    }

    pub(super) fn paint_leader_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        leader: &Leader,
        color: Color32,
    ) {
        for segment in leader.path().segments() {
            if let (Some(a), Some(b)) = (
                self.project(segment.start(), rect),
                self.project(segment.end(), rect),
            ) {
                painter.line_segment([a, b], Stroke::new(1.5, color));
            }
        }
        self.paint_leader_annotation(painter, rect, leader, color);
    }

    fn paint_leader_annotation(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        leader: &Leader,
        color: Color32,
    ) {
        if let Some(points) = self.leader_arrow(leader, rect) {
            painter.add(egui::Shape::convex_polygon(
                points.to_vec(),
                color,
                Stroke::NONE,
            ));
        }
        let landing_length = leader.style().leader_appearance().landing_length;
        if landing_length > 0.0
            && let Some((tail, outward)) = self.leader_tail(leader)
            && let Ok(exit) = leader
                .plane()
                .x_axis()
                .as_vector()
                .scaled(outward * landing_length)
            && let Ok(end) = tail.translated(exit)
            && let (Some(a), Some(b)) = (self.project(tail, rect), self.project(end, rect))
        {
            painter.line_segment([a, b], Stroke::new(1.5, color));
        }
        if let Some(text) = self.leader_text_layout(painter.ctx(), leader, rect)
            && rect.intersects(text.bounds)
        {
            painter.add(TextShape::new(text.origin, text.galley, color).with_angle(text.angle));
        }
    }

    fn leader_tail(&self, leader: &Leader) -> Option<(Point3, Real)> {
        let vertices = leader.path().vertices();
        let [.., preceding, tail] = vertices else {
            return None;
        };
        let axis = leader.plane().x_axis().as_vector();
        let sign = if axis.dot(preceding.vector_to(*tail).ok()?).ok()? < 0.0 {
            -1.0
        } else {
            1.0
        };
        Some((*tail, sign))
    }

    fn leader_arrow(&self, leader: &Leader, rect: Rect) -> Option<[Pos2; 3]> {
        let [start, next, ..] = leader.path().vertices() else {
            return None;
        };
        let direction = start.direction_to(*next).ok()?.as_vector();
        let size = leader.style().leader_appearance().arrow_size;
        if size <= 0.0 {
            return None;
        }
        let base = start.translated(direction.scaled(size).ok()?).ok()?;
        let side = leader.plane().z_axis().as_vector().cross(direction).ok()?;
        let left = base.translated(side.scaled(size * 0.18).ok()?).ok()?;
        let right = base.translated(side.scaled(-size * 0.18).ok()?).ok()?;
        Some([
            self.project(*start, rect)?,
            self.project(left, rect)?,
            self.project(right, rect)?,
        ])
    }

    fn leader_text_layout(
        &self,
        context: &egui::Context,
        leader: &Leader,
        rect: Rect,
    ) -> Option<TextLayout> {
        let (tail, outward) = self.leader_tail(leader)?;
        let axes = leader.plane();
        let appearance = leader.style().leader_appearance();
        let x = axes.x_axis().as_vector();
        let offset = appearance.landing_length + appearance.text_gap;
        let at = tail.translated(x.scaled(outward * offset).ok()?).ok()?;
        let height_point = at
            .translated(
                axes.y_axis()
                    .as_vector()
                    .scaled(appearance.text_height)
                    .ok()?,
            )
            .ok()?;
        let horizontal = at.translated(x.scaled(appearance.text_height).ok()?).ok()?;
        let anchor = self.project(at, rect)?;
        let size = self.project(height_point, rect)?.distance(anchor);
        let projected_axis = self.project(horizontal, rect)? - anchor;
        if leader.text().is_empty()
            || !size.is_finite()
            || size < 0.5
            || !projected_axis.is_finite()
            || projected_axis.length_sq() <= 1.0e-6
        {
            return None;
        }
        let reading_sign = if projected_axis.x < 0.0 { -1.0 } else { 1.0 };
        let right = projected_axis.normalized() * reading_sign;
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
        let center = anchor + right * (reading_sign * outward as f32 * extent.x * 0.5);
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
