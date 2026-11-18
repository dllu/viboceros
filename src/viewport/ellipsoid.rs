//! Read-only ellipsoid sections built from the same axes as the resulting surface.

use super::*;
use viboceros_command::{
    ellipse_input::{Input, Mode, Slot},
    ellipsoid::{Definition, ThirdAxis},
    mesh_ellipsoid::face_count,
    translation::DestinationConstraint,
};
use viboceros_geometry::MeshEllipsoidOptions;

const MAX_DRAFT_MESH_FACES: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub enum EllipsoidPreview {
    Base(Input, Option<MeshEllipsoidOptions>),
    Third(ThirdAxis, Option<MeshEllipsoidOptions>),
}

impl EllipsoidPreview {
    pub(super) fn axis_constraint(self) -> Option<DestinationConstraint> {
        match self {
            Self::Base(..) => None,
            Self::Third(axis, _) => Some(axis.axis_constraint()),
        }
    }

    pub(super) fn base_mouse_plane(self) -> Option<Frame3> {
        let Self::Base(input, _) = self else {
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
        let (definition, topology) = match input {
            EllipsoidPreview::Base(getter, topology) => {
                self.paint_draft_points(painter, rect, &getter.markers());
                let Some(base) = point.and_then(|point| getter.preview(point)) else {
                    return;
                };
                if getter.mode() != Mode::FromFoci {
                    self.paint_curve_draft(painter, rect, &base.ellipse);
                    return;
                }
                (
                    Definition::focal(getter, base.ellipse, tolerance).ok(),
                    topology,
                )
            }
            EllipsoidPreview::Third(axis, topology) => {
                let definition = point.and_then(|point| axis.point(point).ok());
                if definition.is_none() {
                    self.paint_curve_draft(painter, rect, &axis.base);
                }
                (definition, topology)
            }
        };
        if let Some(definition) = definition {
            if let Some(options) = topology
                .filter(|o| face_count(*o).is_some_and(|count| count <= MAX_DRAFT_MESH_FACES))
            {
                if let Ok(mesh) = definition.mesh(options, tolerance) {
                    self.paint_mesh_ellipsoid_draft(painter, rect, &mesh);
                }
            } else {
                for curve in definition.sections().into_iter().flatten() {
                    self.paint_curve_draft(painter, rect, &curve);
                }
            }
            if let Some(foci) = definition.foci {
                self.paint_draft_points(painter, rect, &foci);
            }
        }
    }

    fn paint_mesh_ellipsoid_draft(&self, painter: &egui::Painter, rect: Rect, mesh: &TriangleMesh) {
        let mut drawn = std::collections::HashSet::new();
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        for face in mesh.faces() {
            let vertices = face.indices();
            for (&a, &b) in vertices.iter().zip(vertices.iter().cycle().skip(1)) {
                if a != b
                    && drawn.insert((a.min(b), a.max(b)))
                    && let Some(segment) = self.project_segment(
                        mesh.vertices()[a as usize],
                        mesh.vertices()[b as usize],
                        rect,
                    )
                {
                    painter.line_segment(segment, stroke);
                }
            }
        }
    }
}
