//! Draw mesh grid lines without building a temporary polygon mesh.
use super::*;
use viboceros_geometry::LineSegment;

impl Viewport {
    pub(super) fn paint_rectangle_mesh_grid(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        [a, b, c, d]: [Point3; 4],
        [x_count, y_count]: [usize; 2],
    ) {
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        for (sides, count) in [([(a, b), (d, c)], x_count), ([(a, d), (b, c)], y_count)] {
            let [Ok(a), Ok(b)] =
                sides.map(|(p, q)| LineSegment::try_new(p, q, Tolerance::NUMERICAL_VALIDATION))
            else {
                continue;
            };
            // Skip interior grid lines on dense meshes while retaining actual
            // grid positions and both boundaries.
            let step = count.div_ceil(64).max(1);
            for i in (0..count).step_by(step).chain([count]) {
                let f = i as Real / count as Real;
                if let (Ok(p), Ok(q)) = (a.point_at(f), b.point_at(f))
                    && let Some(segment) = self.project_segment(p, q, rect)
                {
                    painter.line_segment(segment, stroke);
                }
            }
        }
    }
}
