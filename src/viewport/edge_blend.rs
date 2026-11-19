//! Size handles for the selected original B-rep edges.
use super::*;

#[derive(Debug)]
pub(crate) struct SizeHandle {
    pub(crate) pick: ComponentPick,
    pub(crate) point: Point3,
    pub(crate) size: f64,
    pub(crate) active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HandleGesture {
    Pick(ComponentPick),
    Drag {
        pick: ComponentPick,
        value: Option<f64>,
        finished: bool,
    },
}

pub(super) struct Drag {
    pick: ComponentPick,
    start_y: f32,
    start_size: f64,
    pixels_per_unit: f64,
}

impl Viewport {
    pub(super) fn edge_blend_handle_input(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        rect: Rect,
        handles: &[SizeHandle],
        show: bool,
    ) -> Option<HandleGesture> {
        if handles.is_empty() || response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.edge_blend_drag = None;
            return None;
        }
        let clicked = response.clicked_by(PointerButton::Primary);
        let pointer = if clicked {
            response.interact_pointer_pos()
        } else {
            response.hover_pos()
        };
        let mut nearest = None::<(f32, ComponentPick)>;
        let press = response
            .drag_started_by(PointerButton::Primary)
            .then(|| response.ctx.input(|i| i.pointer.press_origin()))
            .flatten();
        let mut pressed_handle = None::<(f32, &SizeHandle)>;
        for handle in handles {
            let Some(position) = self
                .project(handle.point, rect)
                .filter(|&pos| rect.contains(pos))
            else {
                continue;
            };
            let hit = pointer.map_or(f32::INFINITY, |pointer| pointer.distance(position));
            let hovered = hit <= PICK_CAPTURE_PIXELS;
            if hovered && nearest.is_none_or(|(distance, _)| hit < distance) {
                nearest = Some((hit, handle.pick));
            }
            if let Some(press) = press {
                let distance = press.distance(position);
                if distance <= PICK_CAPTURE_PIXELS
                    && pressed_handle.is_none_or(|(prior, _)| distance < prior)
                {
                    pressed_handle = Some((distance, handle));
                }
            }
            if show || hovered || handle.active {
                let color = if hovered || handle.active {
                    SELECTED_COLOR
                } else {
                    Color32::from_rgb(20, 115, 190)
                };
                painter.circle_filled(position, 4., color);
                painter.text(
                    position + Vec2::new(7., -4.),
                    Align2::LEFT_BOTTOM,
                    format!("{:.4}", handle.size),
                    FontId::monospace(11.),
                    color,
                );
            }
        }
        if let Some((press, (_, handle))) = press.zip(pressed_handle) {
            let scale = if self.kind == ViewKind::Perspective {
                self.perspective_focal_length_pixels(rect) / self.view_depth(handle.point)
            } else {
                self.model_to_view_scale(rect)
            };
            if scale.is_finite() && scale > 0. {
                self.edge_blend_drag = Some(Drag {
                    pick: handle.pick,
                    start_y: press.y,
                    start_size: handle.size,
                    pixels_per_unit: scale,
                });
            }
        }
        let finished = response.drag_stopped_by(PointerButton::Primary);
        if (finished || response.dragged_by(PointerButton::Primary))
            && let Some(drag) = &self.edge_blend_drag
        {
            let pick = drag.pick;
            let value = response
                .interact_pointer_pos()
                .map(|pointer| {
                    drag.start_size
                        + (f64::from(drag.start_y) - f64::from(pointer.y)) / drag.pixels_per_unit
                })
                .filter(|&size| size.is_finite() && size > 0.);
            if finished {
                self.edge_blend_drag = None;
            }
            return Some(HandleGesture::Drag {
                pick,
                value,
                finished,
            });
        }
        nearest
            .filter(|_| clicked)
            .map(|(_, pick)| HandleGesture::Pick(pick))
    }
}
