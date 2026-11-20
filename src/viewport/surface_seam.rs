//! Exact isocurves guide a seam placement on the captured source surface.
use super::*;
use viboceros_command::surface_seam::Source;
use viboceros_geometry::SurfaceKnotDirection as Direction;

pub(crate) fn seam_curves(
    surface: &NurbsSurface,
    mode: Direction,
    [u, v]: [Real; 2],
) -> [Option<NurbsCurve>; 2] {
    [
        if mode != Direction::V {
            surface.isocurve_u(v).ok()
        } else {
            None
        },
        if mode != Direction::U {
            surface.isocurve_v(u).ok()
        } else {
            None
        },
    ]
}

#[derive(Debug)]
pub(crate) struct Preview {
    pub(crate) uv: [Real; 2],
    pub(crate) point: Point3,
    pub(crate) curves: [Option<NurbsCurve>; 2],
}

impl Preview {
    pub(crate) fn at(surface: &NurbsSurface, mode: Direction, uv: [Real; 2]) -> Option<Self> {
        Some(Self {
            point: surface.evaluate(uv[0], uv[1]).ok()?,
            curves: seam_curves(surface, mode, uv),
            uv,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Overlay<'a> {
    pub(crate) source: &'a Source,
    pub(crate) original: &'a [Option<NurbsCurve>; 2],
    pub(crate) preview: Option<&'a Preview>,
    pub(crate) picking: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Contact {
    pub(crate) uv: [Real; 2],
    pub(crate) accept: bool,
}

impl Viewport {
    pub(super) fn surface_seam_contact(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        response: &egui::Response,
        document: &Document,
        overlay: Overlay<'_>,
    ) -> Option<Contact> {
        let mut guides = ProjectedPrimitives::default();
        for curve in overlay.original.iter().flatten() {
            self.add_projected_nurbs_curve(&mut guides, rect, curve);
        }
        let color = Color32::from_rgb(180, 95, 195);
        for segment in guides.segments.drain(..) {
            painter.line_segment(segment, Stroke::new(1.5, color));
        }
        if let Some(preview) = overlay.preview {
            for curve in preview.curves.iter().flatten() {
                self.add_projected_nurbs_curve(&mut guides, rect, curve);
            }
            let color = Color32::from_rgb(20, 115, 190);
            for segment in guides.segments {
                painter.line_segment(segment, Stroke::new(2., color));
            }
            if let Some(point) = self.project(preview.point, rect) {
                painter.circle_filled(point, 4., color);
            }
        }

        if !overlay.picking {
            return None;
        }
        let accept = response.clicked_by(PointerButton::Primary);
        let pointer = if accept {
            response.interact_pointer_pos()?
        } else {
            response.hover_pos()?
        };
        let (distance, hint) =
            self.surface_mesh_hint(pointer, rect, document, overlay.source.id())?;
        if distance > PICK_CAPTURE_PIXELS {
            return None;
        }
        let surface = overlay.source.surface();
        let uv = self.surface_view_uv(pointer, rect, surface, hint, document.tolerance())?;
        let point = surface.evaluate(uv[0], uv[1]).ok()?;
        // The shared mesh picker also reports screen-near edges. The source
        // surface must reach the cursor after refining its tessellated hit.
        (self.project(point, rect)?.distance(pointer) <= 0.75).then_some(Contact { uv, accept })
    }
}
