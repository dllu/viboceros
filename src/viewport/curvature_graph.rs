//! Curvature comb overlay in every viewport display mode.
use super::*;
#[cfg(test)]
mod blend_tests;
#[cfg(test)]
mod tests;
impl Viewport {
    pub(super) fn paint_curvature_graph(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        view: &viboceros_command::curvature_graph::View,
    ) {
        for graph in view.graphs.iter() {
            if !document.object(graph.object).is_some_and(|o| {
                o.attributes().is_visible()
                    && document
                        .layer(o.attributes().layer_id())
                        .is_some_and(|l| l.is_visible())
            }) {
                continue;
            }
            let rgb = view.options.color(graph.kind);
            let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            self.paint_curvature_comb(painter, rect, &graph.comb, view.options.factor(), color);
        }
    }
    pub(super) fn paint_blend_curve_curvature(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view: &viboceros_command::blend_curve::CurvatureDisplay,
    ) {
        for graph in &view.data.sources {
            let rgb = view.options.color(graph.kind);
            self.paint_curvature_comb(
                painter,
                rect,
                &graph.comb,
                view.options.factor(),
                Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
            );
        }
        let rgb = view.options.color;
        self.paint_curvature_comb(
            painter,
            rect,
            &view.data.blend,
            view.options.factor(),
            Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
        );
    }
    fn paint_curvature_comb(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        comb: &viboceros_geometry::CurvatureComb,
        factor: Real,
        color: Color32,
    ) {
        let stroke = Stroke::new(1., color);
        for strip in &comb.strips {
            if comb.hairs[strip.clone()]
                .iter()
                .all(|h| h.curvature.to_array() == [0.; 3])
            {
                continue;
            }
            let mut previous = None;
            for hair in &comb.hairs[strip.clone()] {
                let Ok(tip) = hair.tip(factor) else {
                    previous = None;
                    continue;
                };
                if hair.display_hair
                    && let Some(segment) = self.project_selection_segment(hair.point, tip, rect)
                {
                    painter.line_segment(segment, stroke);
                }
                if let Some(p) = previous
                    && let Some(segment) = self.project_selection_segment(p, tip, rect)
                {
                    painter.line_segment(segment, stroke);
                }
                previous = Some(tip);
            }
        }
    }
}
