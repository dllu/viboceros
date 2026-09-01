use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use viboceros_document::{Document, Geometry};
use viboceros_geometry::Point3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayMode {
    Wireframe,
    Shaded,
    Ghosted,
}

impl DisplayMode {
    fn label(self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::Shaded => "Shaded",
            Self::Ghosted => "Ghosted",
        }
    }
}

pub struct Viewport {
    pub display_mode: DisplayMode,
    pixels_per_unit: f32,
    pan: Vec2,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::Wireframe,
            pixels_per_unit: 40.0,
            pan: Vec2::ZERO,
        }
    }
}

impl Viewport {
    pub fn show(&mut self, ui: &mut egui::Ui, document: &Document) {
        let desired_size = ui.available_size().max(Vec2::splat(1.0));
        let (response, painter) = ui.allocate_painter(desired_size, Sense::drag());
        let rect = response.rect;

        if response.dragged() {
            self.pan += response.drag_delta();
        }
        if response.hovered() {
            let zoom = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom != 0.0 {
                self.pixels_per_unit =
                    (self.pixels_per_unit * (zoom * 0.002).exp()).clamp(2.0, 2_000.0);
            }
        }

        painter.rect_filled(rect, 0.0, self.background_color());
        self.paint_grid(&painter, rect);
        self.paint_objects(&painter, rect, document);
        painter.text(
            rect.left_top() + Vec2::new(10.0, 8.0),
            Align2::LEFT_TOP,
            format!(
                "Top · {} · {} object(s)",
                self.display_mode.label(),
                document.objects().len()
            ),
            FontId::proportional(13.0),
            Color32::from_gray(100),
        );
    }

    fn background_color(&self) -> Color32 {
        match self.display_mode {
            DisplayMode::Wireframe => Color32::from_rgb(250, 250, 250),
            DisplayMode::Shaded => Color32::from_rgb(226, 232, 240),
            DisplayMode::Ghosted => Color32::from_rgb(242, 246, 250),
        }
    }

    fn world_origin(&self, rect: Rect) -> Pos2 {
        rect.center() + self.pan
    }

    fn project(&self, point: Point3, rect: Rect) -> Option<Pos2> {
        let origin = self.world_origin(rect);
        let x = f64::from(origin.x) + point.x() * f64::from(self.pixels_per_unit);
        let y = f64::from(origin.y) - point.y() * f64::from(self.pixels_per_unit);
        if !x.is_finite()
            || !y.is_finite()
            || x.abs() > f64::from(f32::MAX)
            || y.abs() > f64::from(f32::MAX)
        {
            return None;
        }
        Some(Pos2::new(x as f32, y as f32))
    }

    fn paint_grid(&self, painter: &egui::Painter, rect: Rect) {
        let origin = self.world_origin(rect);
        let spacing = self.pixels_per_unit;
        if spacing >= 8.0 {
            let start_x = origin.x - ((origin.x - rect.left()) / spacing).ceil() * spacing;
            let mut x = start_x;
            while x <= rect.right() {
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(1.0, Color32::from_gray(218)),
                );
                x += spacing;
            }
            let start_y = origin.y - ((origin.y - rect.top()) / spacing).ceil() * spacing;
            let mut y = start_y;
            while y <= rect.bottom() {
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(1.0, Color32::from_gray(218)),
                );
                y += spacing;
            }
        }

        painter.line_segment(
            [
                Pos2::new(rect.left(), origin.y),
                Pos2::new(rect.right(), origin.y),
            ],
            Stroke::new(1.5, Color32::from_rgb(190, 65, 65)),
        );
        painter.line_segment(
            [
                Pos2::new(origin.x, rect.top()),
                Pos2::new(origin.x, rect.bottom()),
            ],
            Stroke::new(1.5, Color32::from_rgb(60, 145, 75)),
        );
    }

    fn paint_objects(&self, painter: &egui::Painter, rect: Rect, document: &Document) {
        for object in document.objects() {
            let attributes = object.attributes();
            let Some(layer) = document.layer(attributes.layer_id()) else {
                continue;
            };
            if !attributes.is_visible() || !layer.is_visible() {
                continue;
            }

            let layer_color = layer.color();
            let mut color = Color32::from_rgb(layer_color.red, layer_color.green, layer_color.blue);
            if self.display_mode == DisplayMode::Ghosted {
                color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110);
            }
            let width = match self.display_mode {
                DisplayMode::Wireframe => 1.5,
                DisplayMode::Shaded => 2.25,
                DisplayMode::Ghosted => 1.25,
            };

            match object.geometry() {
                Geometry::Point(point) => {
                    if let Some(center) = self.project(*point, rect)
                        && rect.expand(6.0).contains(center)
                    {
                        painter.circle_filled(center, 3.5, color);
                        painter.circle_stroke(center, 5.0, Stroke::new(1.0, color));
                    }
                }
                Geometry::Line(line) => {
                    if let (Some(start), Some(end)) = (
                        self.project(line.start(), rect),
                        self.project(line.end(), rect),
                    ) {
                        painter.line_segment([start, end], Stroke::new(width, color));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_rejects_values_outside_f32_range() {
        let viewport = Viewport::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let point = Point3::try_new(f64::MAX, 0.0, 0.0).unwrap();
        assert_eq!(viewport.project(point, rect), None);
    }
}
