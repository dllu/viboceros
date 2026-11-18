//! Locate the curve under the pointer and retain the parameter for the click.
use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) enum RadiusInput<'a> {
    Curves {
        restrict_to_selection: bool,
    },
    Fixed {
        curve: viboceros_geometry::CurveRef<'a>,
        display: &'a NurbsCurve,
        object: ObjectId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RadiusHit {
    pub(crate) object: ObjectId,
    pub(crate) parameter: f64,
    pub(crate) point: Point3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RadiusClick {
    pub(crate) viewport: usize,
    pub(crate) pixel: Pos2,
    pub(crate) hit: Option<RadiusHit>,
}

impl Viewport {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn radius_getter(
        &self,
        input: Option<RadiusInput<'_>>,
        response: &egui::Response,
        painter: &egui::Painter,
        rect: Rect,
        viewport: usize,
        document: &Document,
        drafting: DraftingInput,
    ) -> (Option<(Pos2, RadiusHit)>, Option<RadiusClick>) {
        let Some(input) = input else {
            return (None, None);
        };
        if let RadiusInput::Fixed { display, .. } = input {
            self.paint_curve_draft(painter, rect, display);
        }
        let clicked = response.clicked_by(PointerButton::Primary);
        let pixel = if clicked {
            response.interact_pointer_pos()
        } else {
            response.hover_pos()
        };
        let Some(pixel) = pixel else {
            return (None, None);
        };
        let hit = self.radius_hit(input, pixel, rect, document, drafting.snap_options());
        if let Some(projected) = hit.and_then(|hit| self.project(hit.point, rect)) {
            painter.circle_filled(projected, 4.0, SELECTED_COLOR);
        }
        (
            hit.map(|hit| (pixel, hit)),
            clicked.then_some(RadiusClick {
                viewport,
                pixel,
                hit,
            }),
        )
    }

    pub(super) fn radius_hit(
        &self,
        input: RadiusInput<'_>,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        mut osnap: viboceros_drafting::ObjectSnapOptions,
    ) -> Option<RadiusHit> {
        osnap.modes = osnap
            .modes
            .with(viboceros_drafting::ObjectSnapKind::Center, false);
        let snap = (osnap.modes != viboceros_drafting::ObjectSnapModes::NONE)
            .then(|| self.object_snap(pointer, rect, document, osnap))
            .flatten();
        let hit = |object, curve: viboceros_geometry::CurveRef<'_>, display: &NurbsCurve| {
            let parameter = if let Some(snap) = snap.filter(|snap| snap.object_id() == object) {
                curve
                    .closest_parameter(snap.point(), document.tolerance())
                    .ok()?
            } else {
                let projected = self.pick_edge_parameter(display, pointer, rect, false)?;
                curve.parameter_from_nurbs(projected).ok()?
            };
            Some(RadiusHit {
                object,
                parameter,
                point: curve.evaluate(parameter).ok()?,
            })
        };
        let restrict_to_selection = match input {
            RadiusInput::Curves {
                restrict_to_selection,
            } => restrict_to_selection,
            RadiusInput::Fixed {
                curve,
                display,
                object,
            } => return hit(object, curve, display),
        };
        let object = document
            .selectable_objects()
            .filter(|object| {
                (!restrict_to_selection || document.is_selected(object.id()))
                    && object.geometry().curve_ref().is_some()
            })
            .filter_map(|object| {
                let display = self.display_cache.borrow_mut().get(
                    object,
                    document.tolerance(),
                    document.subd_control_net(),
                );
                let distance = display
                    .wires()
                    .iter()
                    .map(|&[a, b]| self.selection_line_distance(pointer, a, b, rect))
                    .fold(f32::INFINITY, f32::min);
                (distance <= PICK_CAPTURE_PIXELS).then_some((object, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))?
            .0;
        let curve = object.geometry().curve_ref()?;
        match curve {
            viboceros_geometry::CurveRef::NurbsCurve(display) => hit(object.id(), curve, display),
            _ => hit(object.id(), curve, &curve.to_nurbs().ok()?),
        }
    }
}
