//! Screen-sized annotation labels share their layout with selection.
use super::*;
use egui::text::{LayoutJob, TextFormat};
use std::sync::Arc;
use viboceros_geometry::TextDot;

fn layout(ctx: &egui::Context, dot: &TextDot) -> Arc<egui::Galley> {
    let style = dot.style();
    let size = (style.height_points as f32).clamp(3.0, 256.0);
    let format = TextFormat {
        font_id: FontId::proportional(size),
        italics: style.italic,
        color: Color32::PLACEHOLDER,
        ..Default::default()
    };
    ctx.fonts_mut(|fonts| fonts.layout_job(LayoutJob::simple_format(dot.text().into(), format)))
}

fn badge_bounds(center: Pos2, size: Vec2) -> Rect {
    Rect::from_center_size(center, size + Vec2::new(12.0, 5.0))
}

impl Viewport {
    fn text_dot_center(&self, dot: &TextDot, rect: Rect) -> Option<Pos2> {
        self.project_selection_point(dot.point(), rect)
            .filter(|point| point.is_finite())
    }

    pub(super) fn text_dot_bounds(&self, dot: &TextDot, rect: Rect) -> Option<Rect> {
        let center = self.text_dot_center(dot, rect)?;
        let size = self
            .text_dot_fonts
            .as_ref()
            .map_or(Vec2::ZERO, |ctx| layout(ctx, dot).size());
        Some(badge_bounds(center, size))
    }

    pub(super) fn paint_scene_text_dots(&self, painter: &egui::Painter, rect: Rect) {
        if let Some(scene) = self.cached_scene.borrow().as_ref() {
            for (dot, color) in &scene.text_dots {
                if self.point_within_display_depth(dot.point()) {
                    self.paint_text_dot(painter, rect, dot, *color);
                }
            }
        }
    }

    pub(super) fn paint_text_dot(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        dot: &TextDot,
        color: Color32,
    ) {
        let Some(center) = self.project(dot.point(), rect).filter(|p| p.is_finite()) else {
            return;
        };
        let galley = layout(painter.ctx(), dot);
        let bounds = badge_bounds(center, galley.size());
        if !bounds.intersects(rect) {
            return;
        }
        let text_color = if dot.style().transparent {
            color
        } else {
            let [r, g, b, _] = color.to_srgba_unmultiplied();
            let luma = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114;
            painter.rect_filled(bounds, 5.0, Color32::from_rgb(r, g, b));
            if luma >= 150_000 {
                Color32::BLACK
            } else {
                Color32::WHITE
            }
        };
        painter.galley(center - galley.size() * 0.5, galley, text_color);
    }
}
