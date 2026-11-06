//! Shared transient plane definition and prompt state.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum PlaneMode {
    Choose,
    Rectangle,
    ThreePoint,
    Vertical,
    Through,
    Object,
    AroundCurve,
    Named,
}

impl PlaneMode {
    pub(in crate::app) const fn point_hint(self, step: usize) -> &'static str {
        match (self, step) {
            (Self::Through, _) => "IP: pick the through point",
            (Self::AroundCurve, _) => "IP: pick a point on the reference curve",
            (_, 0) => "IP: pick the first corner",
            (Self::Rectangle, _) => "IP: pick the opposite corner",
            (Self::Vertical, _) => "IP: pick the end of the vertical plane",
            (_, 1) => "IP: pick the adjacent corner",
            _ => "IP: pick the third plane point",
        }
    }
}

#[derive(Clone, Debug)]
pub(in crate::app) struct PlaneGetter {
    pub(super) mode: PlaneMode,
    pub(super) frame: Frame3,
    pub(super) points: Vec<Point3>,
    pub(super) curve: Option<ObjectId>,
}
impl PlaneGetter {
    pub(super) fn points_active(&self) -> bool {
        matches!(
            self.mode,
            PlaneMode::Rectangle | PlaneMode::ThreePoint | PlaneMode::Vertical | PlaneMode::Through
        ) || self.mode == PlaneMode::AroundCurve && self.curve.is_some()
    }
    pub(in crate::app) fn filter(&self) -> ObjectSelectionFilter {
        if self.mode == PlaneMode::AroundCurve {
            ObjectSelectionFilter::Curves
        } else {
            ObjectSelectionFilter::Any
        }
    }
    pub(in crate::app) fn hint(&self) -> &'static str {
        match self.mode {
            PlaneMode::Choose => {
                "IP: choose Object, Rectangle, 3Point, Vertical, ThroughPoint, AroundCurve, NamedCPlane or World/CPlane axes; Enter uses CPlaneXY"
            }
            PlaneMode::Object => {
                "IP: select a planar object (id Face=index chooses a mesh/B-rep face)"
            }
            PlaneMode::Named => "IP: enter a saved CPlane name",
            PlaneMode::AroundCurve if self.curve.is_none() => "IP: select the reference curve",
            mode => mode.point_hint(self.points.len()),
        }
    }
    pub(in crate::app) fn interpolation_hint(&self) -> &'static str {
        if self.mode == PlaneMode::Choose {
            "IP: choose Object, Rectangle, 3Point, Vertical, ThroughPoint, AroundCurve, NamedCPlane or World/CPlane axes; Enter returns to surface selection"
        } else {
            self.hint()
        }
    }
}
