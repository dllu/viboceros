//! Screen-space rail constraints for Sweep2's slash point getter.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SweepSlashInput<'a> {
    pub(crate) rails: [&'a NurbsCurve; 2],
    pub(crate) first: Option<(usize, Point3)>,
    pub(crate) accepted: &'a [[Point3; 2]],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SweepSlashPoint {
    pub(crate) rail: usize,
    pub(crate) point: Point3,
    pub(crate) accept: bool,
}

impl Viewport {
    pub(super) fn paint_sweep_slash(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        response: &egui::Response,
        input: SweepSlashInput<'_>,
        document: &Document,
        drafting: DraftingInput,
    ) -> Option<SweepSlashPoint> {
        let stroke = Stroke::new(1.25, SELECTED_COLOR);
        let draw_point = |p: Point3| {
            if let Some(pixel) = self.project(p, rect) {
                painter.circle_stroke(pixel, 4., stroke);
            }
        };
        for pair in input.accepted {
            for &point in pair {
                draw_point(point);
            }
        }
        if let Some((_, point)) = input.first {
            draw_point(point);
        }
        let pointer = response.hover_pos()?;
        let (rail, curve, cursor, point, _) = input
            .rails
            .into_iter()
            .enumerate()
            .filter(|(rail, _)| input.first.is_none_or(|(first, _)| first != *rail))
            .filter_map(|(rail, curve)| {
                let cursor = self.edge_point_cursor(
                    curve,
                    None,
                    pointer,
                    rect,
                    document,
                    drafting.snap_options(),
                )?;
                let point = curve.evaluate(cursor.parameter).ok()?;
                let distance = self.project(point, rect)?.distance(pointer);
                Some((rail, curve, cursor, point, distance))
            })
            .min_by(|a, b| a.4.total_cmp(&b.4))?;
        if let Some((_, first)) = input.first
            && let (Some(a), Some(b)) = (self.project(first, rect), self.project(point, rect))
            && let Some(segment) = clip_drafting_line(a, b, rect, false)
        {
            painter.line_segment(segment, stroke);
        }
        self.paint_edge_point_cursor(painter, rect, curve, cursor);
        Some(SweepSlashPoint {
            rail,
            point,
            accept: response.clicked_by(PointerButton::Primary),
        })
    }
}
