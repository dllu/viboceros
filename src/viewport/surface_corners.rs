//! Surface-direction controls use pixel hit testing independent of model snaps.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceCornerAction {
    SwapUv,
    ReverseU,
    ReverseV,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceCornerControl {
    pub point: Point3,
    pub action: SurfaceCornerAction,
}
impl SurfaceCornerAction {
    fn label(self) -> &'static str {
        match self {
            Self::SwapUv => "U/V",
            Self::ReverseU => "U",
            Self::ReverseV => "V",
        }
    }
    fn color(self) -> Color32 {
        match self {
            Self::SwapUv => Color32::from_rgb(150, 75, 190),
            Self::ReverseU => Color32::from_rgb(205, 65, 55),
            Self::ReverseV => Color32::from_rgb(45, 115, 205),
        }
    }
}
impl Viewport {
    pub(super) fn pick_surface_corner(
        &self,
        controls: &[SurfaceCornerControl],
        pixel: Pos2,
        rect: Rect,
    ) -> Option<SurfaceCornerAction> {
        controls
            .iter()
            .filter_map(|c| {
                let p = self.project(c.point, rect)?;
                if !rect.contains(p) {
                    return None;
                }
                let distance = p.distance_sq(pixel);
                (distance <= 9. * 9.).then_some((distance, c.action))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|v| v.1)
    }
    pub(super) fn paint_surface_corners(
        &self,
        painter: &egui::Painter,
        controls: &[SurfaceCornerControl],
        pointer: Option<Pos2>,
        rect: Rect,
    ) {
        let hover = pointer.and_then(|p| self.pick_surface_corner(controls, p, rect));
        for c in controls {
            let Some(p) = self.project(c.point, rect).filter(|&p| rect.contains(p)) else {
                continue;
            };
            let color = c.action.color();
            painter.circle_filled(p, 4., color);
            painter.circle_stroke(
                p,
                if hover == Some(c.action) { 8. } else { 6. },
                Stroke::new(1.5, color),
            );
            painter.text(
                p + egui::vec2(9., -9.),
                egui::Align2::LEFT_BOTTOM,
                c.action.label(),
                egui::FontId::proportional(12.),
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corner_hit_testing_uses_pixels_in_all_display_modes_and_rejects_misses() {
        for kind in [ViewKind::Top, ViewKind::Perspective] {
            for mode in [
                DisplayMode::Wireframe,
                DisplayMode::Shaded,
                DisplayMode::Ghosted,
            ] {
                let mut viewport = Viewport::new(kind);
                viewport.display_mode = mode;
                let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 600.));
                let point = Point3::try_new(0., 0., 0.).unwrap();
                let controls = [SurfaceCornerControl {
                    point,
                    action: SurfaceCornerAction::SwapUv,
                }];
                let pixel = viewport.project(point, rect).unwrap();
                assert_eq!(
                    viewport.pick_surface_corner(&controls, pixel, rect),
                    Some(SurfaceCornerAction::SwapUv)
                );
                assert_eq!(
                    viewport.pick_surface_corner(&controls, pixel + egui::vec2(8., 0.), rect),
                    Some(SurfaceCornerAction::SwapUv)
                );
                assert_eq!(
                    viewport.pick_surface_corner(&controls, pixel + egui::vec2(10., 0.), rect),
                    None
                );
                let clipped = Rect::from_min_size(Pos2::new(900., 900.), egui::vec2(1., 1.));
                assert_eq!(
                    viewport.pick_surface_corner(&controls, pixel, clipped),
                    None
                );
            }
        }
    }
}
