//! Read-only ellipsoid sections built from the same axes as the resulting surface.

use super::*;
use viboceros_command::{
    ellipse_input::{Input, Mode, Slot},
    ellipsoid::{Definition, ThirdAxis},
    translation::DestinationConstraint,
};

#[derive(Clone, Copy, Debug)]
pub enum EllipsoidPreview {
    Base(Input),
    Third(ThirdAxis),
}

impl EllipsoidPreview {
    pub(super) fn axis_constraint(self) -> Option<DestinationConstraint> {
        match self {
            Self::Base(_) => None,
            Self::Third(axis) => Some(axis.axis_constraint()),
        }
    }

    pub(super) fn base_mouse_plane(self) -> Option<Frame3> {
        let Self::Base(input) = self else {
            return None;
        };
        if input.mode() == Mode::AroundCurve && !input.requests_curve()
            || input.vertical() && input.slot() == Slot::SecondAxis
        {
            Some(input.mouse_plane())
        } else {
            None
        }
    }
}

impl Viewport {
    pub(super) fn paint_ellipsoid_draft(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: EllipsoidPreview,
        point: Option<Point3>,
        tolerance: Tolerance,
    ) {
        let definition = match input {
            EllipsoidPreview::Base(getter) => {
                self.paint_draft_points(painter, rect, &getter.markers());
                let Some(base) = point.and_then(|point| getter.preview(point)) else {
                    return;
                };
                if getter.mode() != Mode::FromFoci {
                    self.paint_curve_draft(painter, rect, &base.ellipse);
                    return;
                }
                Definition::focal(getter, base.ellipse, tolerance).ok()
            }
            EllipsoidPreview::Third(axis) => {
                let definition = point.and_then(|point| axis.point(point).ok());
                if definition.is_none() {
                    self.paint_curve_draft(painter, rect, &axis.base);
                }
                definition
            }
        };
        if let Some(definition) = definition {
            for curve in definition.sections().into_iter().flatten() {
                self.paint_curve_draft(painter, rect, &curve);
            }
            if let Some(foci) = definition.foci {
                self.paint_draft_points(painter, rect, &foci);
            }
        }
    }
}
