//! Sampled wire draft for Pipe's accepted and tentative cross sections.
use super::*;
use viboceros_command::pipe::{Input, Rail};

const MERIDIANS: [[f64; 2]; 4] = [[1., 0.], [0., 1.], [-1., 0.], [0., -1.]];

impl Viewport {
    pub(super) fn paint_pipe_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: &Input,
        rails: &[Rail],
        cursor: Option<Point3>,
        tolerance: Tolerance,
    ) {
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        let draw = |a, b| {
            if let Some(segment) = self.project_segment(a, b, rect) {
                painter.line_segment(segment, stroke);
            }
        };
        for rail in rails {
            let Some(draft) = input.draft(rail, cursor, tolerance) else {
                continue;
            };
            for cage in &draft.cages {
                for (a, b) in cage.segments() {
                    draw(a, b);
                }
            }
            for (index, &(frame, radii)) in draft.sections.iter().enumerate() {
                for (wall, radius) in radii.into_iter().enumerate().filter(|&(_, r)| r > 0.) {
                    if index % 8 == 0 || index + 1 == draft.sections.len() {
                        self.paint_pipe_rim(painter, rect, frame, radius);
                    }
                    if let Some(&(next, next_radii)) = draft.sections.get(index + 1)
                        && next_radii[wall] > 0.
                    {
                        for [x, y] in MERIDIANS {
                            let end = next_radii[wall];
                            if let (Ok(a), Ok(b)) = (
                                frame.point_at([x * radius, y * radius, 0.]),
                                next.point_at([x * end, y * end, 0.]),
                            ) {
                                draw(a, b);
                            }
                        }
                    }
                }
            }
            if let Some((frame, radii)) = draft.active {
                for radius in radii.into_iter().filter(|&r| r > 0.) {
                    self.paint_pipe_rim(painter, rect, frame, radius);
                }
            }
            if draft.round_caps {
                for (end, sign) in [(draft.sections.first(), -1.), (draft.sections.last(), 1.)] {
                    let Some(&(frame, [radius, _])) = end else {
                        continue;
                    };
                    for [x, y] in MERIDIANS {
                        let mut previous = frame.point_at([radius * x, radius * y, 0.]);
                        for i in 1..=8 {
                            let angle = std::f64::consts::FRAC_PI_2 * f64::from(i) / 8.;
                            let (sin, cos) = angle.sin_cos();
                            let point = frame.point_at([
                                radius * cos * x,
                                radius * cos * y,
                                sign * radius * sin,
                            ]);
                            if let (Ok(a), Ok(b)) = (previous, point.as_ref()) {
                                draw(a, *b);
                            }
                            previous = point;
                        }
                    }
                }
            }
        }
    }

    fn paint_pipe_rim(&self, painter: &egui::Painter, rect: Rect, frame: Frame3, radius: f64) {
        if let Ok(circle) = Circle3::try_from_frame(
            frame.origin(),
            radius,
            frame.x_axis(),
            frame.z_axis(),
            Tolerance::NUMERICAL_VALIDATION,
        ) {
            self.paint_curve_draft(painter, rect, &circle);
        }
    }
}
