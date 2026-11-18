//! Offset wires use the result for the hovered point and current settings.
use super::*;
use viboceros_command::offset_curve::{Draft, Kind, Settings, Target};
use viboceros_geometry::{Curve3, NurbsCurve, UnitVector3};

#[derive(Clone, Copy, Debug)]
pub(crate) struct OffsetDraft<'a> {
    pub kind: Kind,
    pub settings: Settings,
    pub normal: UnitVector3,
    pub tolerance: Tolerance,
    pub analytic_sources: Option<&'a [Curve3]>,
    pub rendered: Option<&'a (Draft, Vec<NurbsCurve>)>,
}

impl Viewport {
    pub(super) fn paint_curve_offset(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: OffsetDraft<'_>,
        point: Option<Point3>,
    ) -> Option<Draft> {
        let target = if preview.settings.through {
            Target::Through(point?)
        } else if let Some(point) = point {
            Target::Side(point)
        } else if preview.settings.both_sides {
            Target::BothSides
        } else {
            return None;
        };
        let draft = Draft {
            kind: preview.kind,
            settings: preview.settings,
            normal: preview.normal,
            tolerance: preview.tolerance,
            target,
        };
        if let Some(sources) = preview.analytic_sources {
            self.paint_offset_wires(painter, rect, &draft.preview(sources));
            return None;
        }
        if let Some((_, curves)) = preview.rendered.filter(|(key, _)| *key == draft) {
            self.paint_offset_wires(painter, rect, curves);
        }
        Some(draft)
    }

    fn paint_offset_wires(&self, painter: &egui::Painter, rect: Rect, curves: &[NurbsCurve]) {
        let mut projected = ProjectedPrimitives::default();
        for curve in curves {
            self.add_projected_nurbs_curve(&mut projected, rect, curve);
        }
        for segment in projected.segments {
            painter.line_segment(segment, Stroke::new(2., Color32::from_rgb(20, 115, 190)));
        }
    }
}
