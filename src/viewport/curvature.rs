//! Camera-space curve/surface locations for pointwise curvature inspection.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_command::{CurvatureFeature, CurvatureSample};
mod surface;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Input<'a> {
    pub(crate) sources: &'a std::collections::BTreeSet<ObjectId>,
    pub(crate) curve: Option<&'a CapturedCurve>,
    pub(crate) features: &'a [CurvatureFeature],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Location {
    Curve { parameter: Real, inflection: bool },
    Surface { face: usize, uv: [Real; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hit {
    pub(crate) object: ObjectId,
    pub(crate) location: Location,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Click {
    pub(crate) viewport: usize,
    pub(crate) pixel: Pos2,
    pub(crate) hit: Option<Hit>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Guides<'a> {
    pub(crate) sample: CurvatureSample,
    pub(crate) markers: &'a [Geometry],
}

impl Viewport {
    pub(super) fn curvature_hit(
        &self,
        input: Input<'_>,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        osnap: viboceros_drafting::ObjectSnapOptions,
    ) -> Option<Hit> {
        if let Some(curve) = input.curve {
            let feature = input
                .features
                .iter()
                .filter_map(|feature| {
                    let distance = self
                        .project_selection_point(feature.point, rect)?
                        .distance(pointer);
                    (distance <= PICK_CAPTURE_PIXELS).then_some((distance, feature))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let object = curve.object_id();
            if let Some((_, feature)) = feature {
                return Some(Hit {
                    object,
                    location: Location::Curve {
                        parameter: feature.parameter,
                        inflection: feature.inflection,
                    },
                });
            }
            let input = super::radius::RadiusInput::Fixed {
                curve: curve.curve_ref(),
                display: curve.curve(),
                object,
            };
            let hit = self.radius_hit(input, pointer, rect, document, osnap)?;
            if self
                .project_selection_point(hit.point, rect)?
                .distance(pointer)
                > PICK_CAPTURE_PIXELS
            {
                return None;
            }
            return Some(Hit {
                object,
                location: Location::Curve {
                    parameter: hit.parameter,
                    inflection: false,
                },
            });
        }

        let (object, face, point) = self.pick_selected_face_with_point_restricted(
            pointer,
            rect,
            document,
            FacePickMode::SurfaceAndBrepAny,
            None,
            Some(input.sources),
        )?;
        let geometry = document.object(object)?.geometry();
        let surface = match geometry {
            Geometry::NurbsSurface(surface) => surface,
            Geometry::Brep(brep) => brep.faces().get(face)?.surface(),
            _ => return None,
        };
        let tolerance = document.tolerance();
        let (u, v) = surface.closest_parameters(point?, tolerance).ok()?;
        let mut uv = self
            .refine_surface_pick(surface, [u, v], pointer, rect)
            .unwrap_or([u, v]);
        if let Geometry::Brep(brep) = geometry {
            let face = &brep.faces()[face];
            if !face.contains_parameters(uv[0], uv[1], tolerance).ok()? {
                if !face.contains_parameters(u, v, tolerance).ok()? {
                    return None;
                }
                uv = [u, v];
            }
        }
        Some(Hit {
            object,
            location: Location::Surface { face, uv },
        })
    }

    pub(super) fn paint_curvature_guides(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        input: Input<'_>,
        guides: Option<Guides<'_>>,
    ) {
        for feature in input.features {
            if let Some(pixel) = self.project_selection_point(feature.point, rect) {
                let color = if feature.inflection {
                    Color32::BLACK
                } else {
                    Color32::WHITE
                };
                painter.circle(pixel, 3.5, color, Stroke::new(1., PICK_PREVIEW_COLOR));
            }
        }
        let Some(guides) = guides else { return };
        for marker in guides.markers {
            match marker {
                Geometry::Circle(circle) => self.paint_curve_draft(painter, rect, circle),
                Geometry::Arc(arc) => {
                    self.paint_curve_draft(painter, rect, &CurveSegment3::Arc(*arc))
                }
                Geometry::Line(line) => {
                    self.paint_curve_draft(painter, rect, &CurveSegment3::Line(*line))
                }
                _ => {}
            }
        }
        if let Some(pixel) = self.project_selection_point(guides.sample.point(), rect) {
            painter.circle_filled(pixel, 3.5, PICK_PREVIEW_COLOR);
        }
        if let CurvatureSample::Curve { point, tangent, .. } = guides.sample {
            let length = 36. / Real::from(self.pixels_per_model_unit_at_origin(rect));
            if let Ok(delta) = tangent.as_vector().scaled(length)
                && let Ok(start) = delta
                    .scaled(-1.)
                    .and_then(|offset| point.translated(offset))
                && let Ok(end) = point.translated(delta)
                && let Some(segment) = self.project_segment(start, end, rect)
            {
                painter.line_segment(segment, Stroke::new(1.5, Color32::from_rgb(220, 153, 48)));
            }
        }
    }
}
