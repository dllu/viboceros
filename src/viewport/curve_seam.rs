//! Closed-curve seam grips and their constrained screen-space drag.
use super::*;
use viboceros_geometry::{CurveRef, ParameterSide, wrap_closed_curve_parameter};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Marker {
    pub(crate) index: usize,
    pub(crate) point: Point3,
    pub(crate) tangent: Option<Vector3>,
}

impl Marker {
    pub(crate) fn at(
        index: usize,
        curve: CurveRef<'_>,
        parameter: Real,
        reversed: bool,
    ) -> Option<Self> {
        let domain = curve.domain();
        let parameter = if parameter == *domain.end() {
            *domain.start()
        } else {
            parameter
        };
        let side = if reversed {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        };
        let tangent_parameter = if reversed && parameter == *domain.start() {
            *domain.end()
        } else {
            parameter
        };
        let tangent = curve
            .evaluate_with_tangent_on_side(tangent_parameter, side)
            .ok()
            .map(|sample| sample.tangent().as_vector())
            .and_then(|tangent| {
                if reversed {
                    tangent.scaled(-1.).ok()
                } else {
                    Some(tangent)
                }
            });
        Some(Self {
            index,
            point: curve.evaluate(parameter).ok()?,
            tangent,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Drag<'a> {
    pub(crate) index: usize,
    pub(crate) origin: Pos2,
    pub(crate) origin_parameter: Real,
    pub(crate) curve: CurveRef<'a>,
    pub(crate) display: &'a NurbsCurve,
    pub(crate) knots: &'a [(Real, Point3)],
    pub(crate) reversed: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Overlay<'a> {
    pub(crate) markers: &'a [Marker],
    pub(crate) drag: Option<Drag<'a>>,
    pub(crate) active: Option<usize>,
    pub(crate) flipping: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Event {
    Grab(usize, Pos2),
    Move { parameter: Option<Real>, done: bool },
    Flip(usize),
}

impl Viewport {
    pub(super) fn curve_seam_event(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        response: &egui::Response,
        overlay: Overlay<'_>,
    ) -> Option<Event> {
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let parameter = overlay.drag.zip(pointer).and_then(|(drag, pointer)| {
            if !rect.contains(pointer) {
                return None;
            }
            if pointer.distance(drag.origin) < 2. {
                return Some(drag.origin_parameter);
            }
            if let Some((_, parameter)) = drag
                .knots
                .iter()
                .filter_map(|&(parameter, knot)| {
                    Some((self.project(knot, rect)?.distance(pointer), parameter))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
            {
                return Some(parameter);
            }
            let projected = self.pick_edge_parameter(drag.display, pointer, rect, false)?;
            let parameter = drag.curve.parameter_from_nurbs(projected).ok()?;
            let domain = drag.curve.domain();
            Some(if parameter == *domain.end() {
                *domain.start()
            } else {
                parameter
            })
        });
        let marker_position = |marker: &Marker| self.project(marker.point, rect);
        let hit = pointer.and_then(|pointer| {
            overlay
                .markers
                .iter()
                .filter_map(|marker| {
                    let distance = marker_position(marker)?.distance(pointer);
                    (distance <= PICK_CAPTURE_PIXELS).then_some((distance, marker.index))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, index)| index)
        });
        let dynamic_marker = overlay.drag.zip(parameter).and_then(|(drag, t)| {
            let t = wrap_closed_curve_parameter(t, &drag.curve.domain()).ok()?;
            Marker::at(drag.index, drag.curve, t, drag.reversed)
        });
        let mut previous = None;
        let guide = Color32::from_rgb(184, 91, 194);
        for stored in overlay.markers {
            let marker = dynamic_marker
                .filter(|marker| marker.index == stored.index)
                .unwrap_or(*stored);
            let Some(pixel) = marker_position(&marker) else {
                previous = None;
                continue;
            };
            if let Some(last) = previous {
                painter.line_segment([last, pixel], Stroke::new(1., guide));
            }
            previous = Some(pixel);
            let color = if overlay.active.or(hit) == Some(marker.index) {
                SELECTED_COLOR
            } else {
                guide
            };
            let length = 22. / self.model_to_view_scale(rect).max(1e-12);
            if let Some(end) = marker
                .tangent
                .and_then(|tangent| tangent.scaled(length).ok())
                .and_then(|offset| marker.point.translated(offset).ok())
                .and_then(|point| self.project(point, rect))
            {
                let direction = end - pixel;
                if direction.length() >= 2. {
                    painter.arrow(pixel, direction, Stroke::new(1.5, color));
                }
            }
            painter.circle_filled(pixel, 4., color);
            painter.circle_stroke(pixel, 6., Stroke::new(1.25, color));
        }

        if overlay.drag.is_some() {
            if response.hovered() {
                response.ctx.set_cursor_icon(CursorIcon::Grabbing);
            }
            let done = response.ctx.input(|input| {
                input.pointer.button_released(PointerButton::Primary)
                    || !input.pointer.button_down(PointerButton::Primary)
            });
            return (done || parameter.is_some()).then_some(Event::Move { parameter, done });
        }
        let index = hit?;
        if response.hovered() {
            response.ctx.set_cursor_icon(if overlay.flipping {
                CursorIcon::PointingHand
            } else {
                CursorIcon::Grab
            });
        }
        if overlay.flipping {
            return response
                .clicked_by(PointerButton::Primary)
                .then_some(Event::Flip(index));
        }
        (overlay.active.is_none()
            && response.is_pointer_button_down_on()
            && response
                .ctx
                .input(|input| input.pointer.button_pressed(PointerButton::Primary)))
        .then(|| Event::Grab(index, pointer.expect("seam hit has a pointer")))
    }
}
