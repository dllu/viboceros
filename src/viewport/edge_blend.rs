//! Size handles for the selected original B-rep edges.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum HandleLocation {
    Start,
    End,
    Seam,
    Interior(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SizeHandleId {
    pub(crate) pick: ComponentPick,
    pub(crate) location: HandleLocation,
}

#[derive(Debug)]
pub(crate) struct SizeHandle {
    pub(crate) id: SizeHandleId,
    pub(crate) point: Point3,
    pub(crate) size: f64,
    pub(crate) active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum HandleGesture {
    Pick(SizeHandleId),
    Select(Vec<SizeHandleId>, SelectionMode),
    Place(SizeHandleId),
    Drag {
        id: SizeHandleId,
        value: Option<f64>,
        finished: bool,
    },
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum HandleMode<'a> {
    #[default]
    Edit,
    Copy,
    Remove,
    Place(&'a [(ComponentPick, &'a NurbsCurve)], f64),
}

pub(super) enum Drag {
    Size(SizeDrag),
    Window { start: Pos2, mode: SelectionMode },
}

pub(super) struct SizeDrag {
    id: SizeHandleId,
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
        mode: HandleMode<'_>,
    ) -> (Option<HandleGesture>, Option<SizeHandleId>) {
        if handles.is_empty() || response.ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.edge_blend_drag = None;
            return (None, None);
        }
        let clicked = response.clicked_by(PointerButton::Primary);
        let pointer = if clicked {
            response.interact_pointer_pos()
        } else {
            response.hover_pos()
        };
        let mut nearest = None::<(f32, SizeHandleId)>;
        let modifiers = response.ctx.input(|input| input.modifiers);
        let selection_mode = if modifiers.ctrl && !modifiers.shift {
            SelectionMode::Remove
        } else {
            selection_mode(modifiers)
        };
        let resize_handles = matches!(mode, HandleMode::Edit);
        let select_handles = matches!(mode, HandleMode::Edit | HandleMode::Remove);
        let pickable = |location| match mode {
            HandleMode::Place(..) => false,
            HandleMode::Remove => matches!(location, HandleLocation::Interior(_)),
            _ => true,
        };
        if !select_handles
            || (!resize_handles && matches!(self.edge_blend_drag, Some(Drag::Size(_))))
        {
            self.edge_blend_drag = None;
        }
        if select_handles
            && response.hovered()
            && response
                .ctx
                .input(|input| input.pointer.button_pressed(PointerButton::Primary))
        {
            self.edge_blend_drag = response.interact_pointer_pos().map(|start| Drag::Window {
                start,
                mode: selection_mode,
            });
        }
        let press = match self.edge_blend_drag {
            Some(Drag::Window {
                start,
                mode: SelectionMode::Replace,
            }) if resize_handles && response.drag_started_by(PointerButton::Primary) => Some(start),
            _ => None,
        };
        let mut pressed_handle = None::<(f32, &SizeHandle)>;
        for handle in handles {
            let Some(position) = self
                .project(handle.point, rect)
                .filter(|&pos| rect.contains(pos))
            else {
                continue;
            };
            let hit = pointer
                .filter(|_| pickable(handle.id.location))
                .map_or(f32::INFINITY, |pointer| pointer.distance(position));
            let hovered = hit <= PICK_CAPTURE_PIXELS;
            if hovered && nearest.is_none_or(|(distance, _)| hit < distance) {
                nearest = Some((hit, handle.id));
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
        if let HandleMode::Place(edges, size) = mode {
            let Some(pointer) = pointer else {
                return (None, None);
            };
            let mut nearest = None::<(f32, f64, SizeHandleId, Pos2)>;
            for &(pick, curve) in edges {
                let Some(t) = self.pick_edge_parameter(curve, pointer, rect, false) else {
                    continue;
                };
                let domain = curve.domain();
                let fraction = (t - domain.start()) / (domain.end() - domain.start());
                if !(0. < fraction && fraction < 1.) {
                    continue;
                }
                let Ok(point) = curve.evaluate(t) else {
                    continue;
                };
                let Some(position) = self.project(point, rect) else {
                    continue;
                };
                let distance = pointer.distance(position);
                let depth = self.view_depth(point);
                if distance <= PICK_CAPTURE_PIXELS
                    && nearest.is_none_or(|(previous, farther, _, _)| {
                        distance < previous || (distance == previous && depth < farther)
                    })
                {
                    nearest = Some((
                        distance,
                        depth,
                        SizeHandleId {
                            pick,
                            location: HandleLocation::Interior(fraction),
                        },
                        position,
                    ));
                }
            }
            if let Some((_, _, id, position)) = nearest {
                painter.circle_stroke(position, 6., Stroke::new(1.5, SELECTED_COLOR));
                painter.text(
                    position + Vec2::new(9., -6.),
                    Align2::LEFT_BOTTOM,
                    format!("{size:.4}"),
                    FontId::monospace(11.),
                    SELECTED_COLOR,
                );
                return (clicked.then_some(HandleGesture::Place(id)), Some(id));
            }
            return (None, None);
        }
        if let Some((press, (_, handle))) = press.zip(pressed_handle) {
            let scale = if self.kind == ViewKind::Perspective {
                self.perspective_focal_length_pixels(rect) / self.view_depth(handle.point)
            } else {
                self.model_to_view_scale(rect)
            };
            if scale.is_finite() && scale > 0. {
                self.edge_blend_drag = Some(Drag::Size(SizeDrag {
                    id: handle.id,
                    start_y: press.y,
                    start_size: handle.size,
                    pixels_per_unit: scale,
                }));
            }
        }
        let finished = response.drag_stopped_by(PointerButton::Primary);
        if finished || response.dragged_by(PointerButton::Primary) {
            let gesture = match &self.edge_blend_drag {
                Some(Drag::Size(drag)) => {
                    let value = response
                        .interact_pointer_pos()
                        .map(|pointer| {
                            drag.start_size
                                + (f64::from(drag.start_y) - f64::from(pointer.y))
                                    / drag.pixels_per_unit
                        })
                        .filter(|&size| size.is_finite() && size > 0.);
                    Some(HandleGesture::Drag {
                        id: drag.id,
                        value,
                        finished,
                    })
                }
                Some(Drag::Window { start, mode }) => {
                    response.interact_pointer_pos().and_then(|end| {
                        if !finished {
                            self.paint_selection_window(painter, *start, end, None);
                            return None;
                        }
                        let window = Rect::from_two_pos(*start, end).intersect(rect);
                        let ids = handles
                            .iter()
                            .filter(|handle| pickable(handle.id.location))
                            .filter(|handle| {
                                self.project(handle.point, rect)
                                    .is_some_and(|p| window.contains(p))
                            })
                            .map(|handle| handle.id)
                            .collect();
                        Some(HandleGesture::Select(ids, *mode))
                    })
                }
                None => None,
            };
            if finished {
                self.edge_blend_drag = None;
            }
            return (gesture, None);
        }
        if !clicked {
            return (None, None);
        }
        let started_mode = match self.edge_blend_drag.take() {
            Some(Drag::Window { mode, .. }) => mode,
            _ => selection_mode,
        };
        let gesture = match nearest {
            Some((_, id))
                if matches!(mode, HandleMode::Edit) && started_mode != SelectionMode::Replace =>
            {
                Some(HandleGesture::Select(vec![id], started_mode))
            }
            Some((_, id)) => Some(HandleGesture::Pick(id)),
            None if matches!(mode, HandleMode::Edit) => {
                Some(HandleGesture::Select(Vec::new(), started_mode))
            }
            None => None,
        };
        (gesture, None)
    }
}
