//! Straight wall and cap wires cached for the surface distance getter.
use super::{curve_sampling::ViewportCurve, surface_apex::FaceDraft, *};
use viboceros_command::{
    extrude_surface::{SurfaceExtrusionOptions, surface_extrusion_reference_normal},
    translation::DestinationConstraint,
};
use viboceros_geometry::{BoundingBox3, Brep};

#[derive(Debug)]
pub(crate) struct SurfaceExtrusionDraft {
    center: Point3,
    faces: Vec<SweepFace>,
}

#[derive(Debug)]
struct SweepFace {
    outline: FaceDraft,
    normal: Option<Vector3>,
    cap: Vec<[Point3; 2]>,
    tangent_seams: Vec<Point3>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SurfaceExtrusionPreview<'a> {
    pub draft: &'a SurfaceExtrusionDraft,
    pub constraint: DestinationConstraint,
    pub plane: Frame3,
    pub options: SurfaceExtrusionOptions,
    pub hover: Option<f64>,
}

impl SurfaceExtrusionDraft {
    pub(crate) fn capture(document: &Document, targets: &[(ObjectId, usize)]) -> Option<Self> {
        let tolerance = document.tolerance();
        let mut bounds = None::<BoundingBox3>;
        let mut faces = Vec::new();
        for &(id, index) in targets {
            let brep = match document.object(id)?.geometry() {
                Geometry::Brep(brep) => brep.duplicate_faces(&[index], tolerance).ok()?,
                Geometry::NurbsSurface(surface) => {
                    Brep::try_surface_face(surface.clone(), tolerance).ok()?
                }
                _ => return None,
            };
            let face = brep.faces().first()?;
            let face_bounds = brep
                .tight_bounds(tolerance)
                .unwrap_or_else(|_| brep.bounds());
            bounds = Some(if let Some(bounds) = bounds {
                bounds.union(face_bounds).ok()?
            } else {
                face_bounds
            });
            let outline = FaceDraft::capture(&brep, face, false, tolerance)?;
            let tangent_seams = brep
                .sweep_boundary_splits(true)
                .unwrap_or_default()
                .into_iter()
                .flat_map(|(edge, parameters)| {
                    let curve = brep.edges()[edge].curve();
                    parameters
                        .into_iter()
                        .filter_map(move |parameter| curve.evaluate(parameter).ok())
                })
                .collect();
            let normal = surface_extrusion_reference_normal(&brep, tolerance)
                .ok()
                .flatten();
            let u = face.surface().domain_u();
            let v = face.surface().domain_v();
            let mut cap = Vec::new();
            for curve in [
                face.isocurve_u_segments(v.start().midpoint(*v.end()), tolerance),
                face.isocurve_v_segments(u.start().midpoint(*u.end()), tolerance),
            ]
            .into_iter()
            .flatten()
            .flatten()
            {
                curve.visit_segments(|a, b| cap.push([a, b]));
            }
            faces.push(SweepFace {
                outline,
                normal,
                cap,
                tangent_seams,
            });
        }
        Some(Self {
            center: bounds?.center().ok()?,
            faces,
        })
    }

    pub(crate) fn constraint(
        &self,
        base: Option<Point3>,
        options: SurfaceExtrusionOptions,
        plane: Frame3,
    ) -> DestinationConstraint {
        let direction = options
            .direction
            .or(self.faces[0].normal)
            .and_then(|axis| axis.normalized_nonzero().ok())
            .unwrap_or(plane.z_axis());
        DestinationConstraint {
            anchor: base.unwrap_or(self.center),
            direction: Some(direction),
            distance: None,
        }
    }
}

pub(crate) fn signed_height(constraint: DestinationConstraint, point: Point3) -> Option<f64> {
    constraint
        .anchor
        .vector_to(point)
        .ok()?
        .dot(constraint.direction?.as_vector())
        .ok()
}

impl Viewport {
    pub(super) fn surface_extrusion_end_on_point(
        &self,
        constraint: DestinationConstraint,
        pointer: Pos2,
        rect: Rect,
    ) -> Option<Point3> {
        let axis = constraint.direction?.as_vector();
        let (offset, view) =
            self.drafting_view_line_relative_to(constraint.anchor, pointer, rect)?;
        let view = view.normalized_nonzero().ok()?.as_vector();
        if axis.cross(view).ok()?.length().ok()? > 1e-12 {
            return None;
        }
        // End-on views measure radial distance in the view plane. Upward
        // picks and picks at the guide's screen height use the positive side.
        let radial = offset.cross(axis).ok()?.length().ok()?;
        let anchor = self.project(constraint.anchor, rect)?;
        let amount = if pointer.y > anchor.y {
            -radial
        } else {
            radial
        };
        constraint.anchor.translated(axis.scaled(amount).ok()?).ok()
    }

    pub(super) fn paint_surface_extrusion(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        preview: SurfaceExtrusionPreview<'_>,
        cursor: Option<Point3>,
        tolerance: Tolerance,
    ) {
        let Some(height) = cursor
            .and_then(|point| signed_height(preview.constraint, point))
            .or(preview.hover)
            .filter(|height| height.abs() > tolerance.absolute())
        else {
            return;
        };
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        let paint = |a, b| {
            if let Some(segment) = self.project_segment(a, b, rect) {
                painter.line_segment(segment, stroke);
            }
        };
        for face in &preview.draft.faces {
            let axis = preview
                .options
                .direction
                .or(face.normal)
                .unwrap_or(preview.plane.z_axis().as_vector());
            if face.normal.is_some_and(|normal| {
                axis.dot(normal)
                    .is_ok_and(|cosine| cosine.abs() <= tolerance.angular())
            }) {
                continue;
            }
            let Ok(displacement) = axis.scaled(height) else {
                continue;
            };
            let start_displacement = if preview.options.both_sides {
                displacement.scaled(-1.).ok()
            } else {
                None
            };
            let start = |p: Point3| start_displacement.map_or(Some(p), |v| p.translated(v).ok());
            let end = |p: Point3| p.translated(displacement).ok();
            let cap = preview
                .options
                .solid
                .then_some(face.cap.as_slice())
                .into_iter()
                .flatten()
                .copied();
            for [a, b] in face.outline.rim().chain(cap) {
                if let (Some(a), Some(b)) = (start(a), start(b)) {
                    paint(a, b);
                }
                if let (Some(a), Some(b)) = (end(a), end(b)) {
                    paint(a, b);
                }
            }
            let seams = preview
                .options
                .split_at_tangents
                .then_some(face.tangent_seams.as_slice())
                .into_iter()
                .flatten()
                .copied();
            for p in face.outline.boundary_rulings().chain(seams) {
                if let (Some(a), Some(b)) = (start(p), end(p)) {
                    paint(a, b);
                }
            }
        }
    }
}
