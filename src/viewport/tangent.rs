//! Read-only circle contacts, reference spheres and final arc-choice previews.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct TangentPreview<'a> {
    pub sphere: bool,
    pub contacts: &'a viboceros_command::circle_tangent_input::Input,
    pub output: Option<viboceros_command::circle_deform_input::Output>,
    pub arc_output: Option<viboceros_command::arc_deform_input::Output>,
    pub arc: Option<&'a viboceros_command::arc_tangent_input::Support>,
}
impl TangentPreview<'_> {
    pub(crate) fn circle_curves(
        self,
        circle: viboceros_geometry::Circle3,
    ) -> Result<Vec<viboceros_geometry::NurbsCurve>, viboceros_geometry::GeometryError> {
        if self.sphere {
            use viboceros_geometry::{Circle3, Tolerance};
            let center = circle.center();
            let radius = circle.radius();
            let z = circle.normal()?;
            return [
                Ok(circle),
                Circle3::try_from_frame(
                    center,
                    radius,
                    circle.y_axis(),
                    circle.x_axis(),
                    Tolerance::NUMERICAL_VALIDATION,
                ),
                Circle3::try_from_frame(
                    center,
                    radius,
                    z,
                    circle.y_axis(),
                    Tolerance::NUMERICAL_VALIDATION,
                ),
            ]
            .into_iter()
            .map(|c| c?.to_nurbs())
            .collect();
        }
        Ok(vec![
            self.output
                .map_or_else(|| circle.to_nurbs(), |o| o.curve(circle))?,
        ])
    }
}
impl Viewport {
    pub(super) fn paint_tangent_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        input: TangentPreview<'_>,
        pointer: Option<Pos2>,
        cursor: Option<Point3>,
    ) {
        let getter = input.contacts;
        let curves = if let Some(support) = input.arc {
            self.paint_draft_points(painter, rect, &support.contact_points);
            cursor
                .and_then(|p| support.choose(p, document.tolerance()).ok())
                .and_then(|a| {
                    input
                        .arc_output
                        .map_or_else(
                            || a.to_nurbs().map_err(viboceros_command::CommandError::from),
                            |o| o.curve(a),
                        )
                        .ok()
                })
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            if let Some(anchor) = getter.anchor() {
                self.paint_draft_points(painter, rect, &[anchor]);
            }
            let circle = if getter.wants_point() {
                cursor.and_then(|p| getter.preview_point(p))
            } else if getter.wants_curve() {
                pointer
                    .and_then(|pointer| {
                        let reference = self
                            .pick_edges(pointer, rect, document)
                            .first()
                            .map(|p| (p.object, Some(p.edge)))
                            .or_else(|| {
                                self.pick_object_candidates_matching_preview_excluding(
                                    pointer,
                                    rect,
                                    document,
                                    ObjectSelectionFilter::Curves,
                                    None,
                                    None,
                                )
                                .first()
                                .map(|id| (*id, None))
                            })?;
                        let geometry = document.object(reference.0)?.geometry();
                        let point = self.curve_pick_point(
                            geometry,
                            reference.1,
                            pointer,
                            document.tolerance(),
                        )?;
                        getter.preview_curve(document, reference.0, reference.1, point)
                    })
                    .or_else(|| getter.preview_current())
            } else {
                getter.preview_current()
            };
            circle
                .and_then(|c| input.circle_curves(c).ok())
                .unwrap_or_default()
        };
        for curve in curves {
            let mut projected = ProjectedPrimitives::default();
            self.add_projected_nurbs_curve(&mut projected, rect, &curve);
            for segment in projected.segments {
                painter.line_segment(segment, Stroke::new(2., Color32::from_rgb(20, 115, 190)));
            }
        }
    }
}
