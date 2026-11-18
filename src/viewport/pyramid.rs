//! Drafts for the polygon rim, apex and top ring.
use super::*;
use viboceros_command::pyramid::{End, Input};

impl Viewport {
    pub(super) fn paint_pyramid_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input,
        point: Option<Point3>,
    ) {
        let Some((base, end)) = input.preview(point) else {
            return;
        };
        let limit = MAX_DRAFT_MESH_FACES / 2;
        let Ok(rim) = base.preview_rim(limit) else {
            return;
        };
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        let draw = |start, end| {
            if let Some(segment) = self.project_segment(start, end, rect) {
                painter.line_segment(segment, stroke);
            }
        };
        for (&start, &next) in rim.iter().zip(rim.iter().cycle().skip(1)) {
            draw(start, next);
        }
        match end {
            Some(End::Apex(apex)) => {
                for point in rim {
                    draw(point, apex);
                }
            }
            Some(End::Top(top)) => {
                let Ok(top) = top.preview_rim(limit) else {
                    return;
                };
                for ((base, &point), &next) in
                    rim.into_iter().zip(&top).zip(top.iter().cycle().skip(1))
                {
                    draw(point, next);
                    draw(base, point);
                }
            }
            None => {}
        }
    }
}
