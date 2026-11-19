//! Project native ink bounds and keep glyph painting and picking on the same quad.
use super::*;
use egui::{Align, epaint::TextShape, text::LayoutJob};
use std::sync::Arc;
use viboceros_geometry::{TextAlignment, TextAnnotation};

impl Viewport {
    pub(super) fn project_model_text(
        &self,
        text: &TextAnnotation,
        rect: Rect,
    ) -> Option<[Pos2; 4]> {
        let corners = if text.style().text_in_view() {
            let (right, up, _) = self.screen_axes();
            let origin = text.plane().origin();
            let [[left, bottom], [right_edge, top]] = text.layout().ink_bounds;
            let point = |x, y| {
                let delta: NaVector3<Real> = right * x + up * y;
                origin
                    .translated(Vector3::try_new(delta.x, delta.y, delta.z).ok()?)
                    .ok()
            };
            [
                point(left, top)?,
                point(right_edge, top)?,
                point(right_edge, bottom)?,
                point(left, bottom)?,
            ]
        } else {
            text.quad()
        };
        let [Some(a), Some(b), Some(c), Some(d)] =
            corners.map(|point| self.project_selection_point(point, rect))
        else {
            return None;
        };
        let edge = b - a;
        let down = d - a;
        let area = edge.x * down.y - edge.y * down.x;
        (area.is_finite() && area.abs() > 1e-4).then_some([a, b, c, d])
    }

    pub(super) fn paint_scene_model_text(&self, painter: &egui::Painter, rect: Rect) {
        if let Some(scene) = self.cached_scene.borrow().as_ref() {
            for (text, color) in &scene.model_text {
                self.paint_model_text(painter, rect, text, *color);
            }
        }
    }

    pub(super) fn paint_model_text(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        text: &TextAnnotation,
        color: Color32,
    ) {
        let Some(mut quad) = self.project_model_text(text, rect) else {
            return;
        };
        if text.text().is_empty() || !Rect::from_points(&quad).intersects(rect) {
            return;
        }

        let style = text.style();
        if style.leader_appearance().draws_forward() && !style.text_in_view() {
            let [flip_x, flip_y] = self.model_text_flips(text);
            if flip_x {
                quad.swap(0, 1);
                quad.swap(2, 3);
            }
            if flip_y {
                quad.swap(0, 3);
                quad.swap(1, 2);
            }
        }

        let height = quad[0].distance(quad[3]);
        let size = (height / text.text().lines().count().max(1) as f32).clamp(8.0, 128.0);
        let layout = text.layout();
        let width = layout.wrapping_width.map_or(f32::INFINITY, |width| {
            let nominal = style.text_style_height() * layout.height_factor;
            (size * (width / nominal) as f32).max(1.0)
        });
        let mut job = LayoutJob::simple(
            text.text().into(),
            FontId::proportional(size),
            Color32::PLACEHOLDER,
            width,
        );
        job.halign = match style.text_alignment() {
            TextAlignment::Left => Align::LEFT,
            TextAlignment::Center => Align::Center,
            TextAlignment::Right => Align::RIGHT,
        };
        let galley = painter.ctx().fonts_mut(|fonts| fonts.layout_job(job));
        let Some(galley) = pose_galley(&galley, quad) else {
            return;
        };
        painter.add(TextShape::new(quad[0], Arc::new(galley), color));
    }

    // Match OpenNURBS CalcTextFlip for a text plane already placed in model space.
    fn model_text_flips(&self, text: &TextAnnotation) -> [bool; 2] {
        let (right, up, into_view) = self.screen_axes();
        let plane = text.plane();
        let x = NaVector3::from(plane.x_axis().as_vector().to_array());
        let y = NaVector3::from(plane.y_axis().as_vector().to_array());
        let (sin, cos) = text.layout().rotation.sin_cos();
        let baseline = x * cos + y * sin;
        let ascenders = y * cos - x * sin;
        let back = x.cross(&y).dot(&into_view) > 0.0;
        let x_right = baseline.dot(&right);
        let upright = if x_right.abs() > baseline.dot(&up).abs() {
            ascenders.dot(&up) > 0.0
        } else {
            let tolerance = if self.kind.is_parallel() {
                80.001_f64.to_radians().cos()
            } else {
                0.0
            };
            let y_right = ascenders.dot(&right);
            match (back, y_right < 0.0, y_right > 0.0) {
                (true, true, _) => x_right < tolerance,
                (true, false, _) => x_right <= -tolerance,
                (false, _, true) => x_right > tolerance,
                (false, _, false) => x_right >= -tolerance,
            }
        };
        [upright == back, !upright]
    }
}

fn pose_galley(source: &egui::Galley, quad: [Pos2; 4]) -> Option<egui::Galley> {
    let ink = source.mesh_bounds;
    if !ink.is_finite() || !ink.is_positive() {
        return None;
    }
    // Keep the UVs in atlas texels. egui normalizes them after all frame text has been laid out.
    let [a, b, c, d] = quad;
    let mut galley = source.clone();
    galley.mesh_bounds = Rect::NOTHING;
    for placed in &mut galley.rows {
        let offset = placed.pos.to_vec2();
        placed.pos = Pos2::ZERO;
        let row = Arc::make_mut(&mut placed.row);
        let mut bounds = Rect::NOTHING;
        for vertex in &mut row.visuals.mesh.vertices {
            let pos = vertex.pos + offset;
            let u = (pos.x - ink.min.x) / ink.width();
            let v = (pos.y - ink.min.y) / ink.height();
            let top = (b - a) * u;
            let bottom = d - a + (c - d) * u;
            vertex.pos = Pos2::ZERO + top + (bottom - top) * v;
            bounds.extend_with(vertex.pos);
        }
        row.visuals.mesh_bounds = bounds;
        galley.mesh_bounds = galley.mesh_bounds.union(bounds);
    }
    Some(galley)
}
