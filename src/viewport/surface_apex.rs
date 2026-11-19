//! Cached source boundaries for the live ruled surface-to-point preview.
use super::{curve_sampling::ViewportCurve, *};
use std::{cell::RefCell, collections::BTreeMap};
use viboceros_geometry::{Brep, BrepFace, MeshProjectionIndex};

#[derive(Debug)]
pub(crate) struct SurfaceApexDraft {
    faces: Vec<FaceDraft>,
    pub(crate) hover: Option<Point3>,
    boundary: Option<BoundaryDraft>,
}

#[derive(Debug)]
struct BoundaryDraft {
    mesh: Option<MeshProjectionIndex<'static>>,
    frame: RefCell<Option<BoundaryFrame>>,
}

#[derive(Debug)]
struct BoundaryFrame {
    apex: Point3,
    tolerance: Tolerance,
    edges: Vec<[Point3; 2]>,
}

#[derive(Debug)]
struct FaceDraft {
    rim: Vec<[Point3; 2]>,
    rulings: Vec<Point3>,
}

impl SurfaceApexDraft {
    pub(crate) fn capture(document: &Document, faces: &[(ObjectId, usize)]) -> Self {
        let tolerance = document.tolerance();
        let faces = faces
            .iter()
            .filter_map(|&(id, index)| match document.object(id)?.geometry() {
                Geometry::Brep(brep) => FaceDraft::capture(brep, brep.faces().get(index)?),
                Geometry::NurbsSurface(surface) => {
                    let brep = Brep::try_surface_face(surface.clone(), tolerance).ok()?;
                    FaceDraft::capture(&brep, brep.faces().first()?)
                }
                _ => None,
            })
            .collect();
        Self {
            faces,
            hover: None,
            boundary: None,
        }
    }

    pub(crate) fn set_boundary(&mut self, boundary: Option<&Brep>, tolerance: Tolerance) {
        self.hover = None;
        self.boundary = boundary.map(|brep| BoundaryDraft {
            mesh: brep
                .display_mesh(8, tolerance)
                .ok()
                .and_then(|mesh| MeshProjectionIndex::new_owned(mesh).ok()),
            frame: RefCell::new(None),
        });
    }
}

impl BoundaryDraft {
    fn project(&self, start: Point3, apex: Point3, tolerance: Tolerance) -> Option<Point3> {
        let direction = start.direction_to(apex).ok()?.as_vector();
        let length = start.distance_to(apex).ok()?;
        self.mesh
            .as_ref()?
            .project_point(start, direction, tolerance)
            .ok()?
            .into_iter()
            .filter_map(|point| {
                let distance = direction.dot(start.vector_to(point).ok()?).ok()?;
                (distance > tolerance.absolute() && distance < length - tolerance.absolute())
                    .then_some((distance, point))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, point)| point)
    }

    fn clipped_edges(
        &self,
        source: &SurfaceApexDraft,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Option<Vec<[Point3; 2]>> {
        let mut edges = Vec::new();
        for face in &source.faces {
            for &[a, b] in &face.rim {
                edges.push([a, b]);
                edges.push([
                    self.project(a, apex, tolerance)?,
                    self.project(b, apex, tolerance)?,
                ]);
            }
            for &start in &face.rulings {
                edges.push([start, self.project(start, apex, tolerance)?]);
            }
        }
        Some(edges)
    }
}

impl FaceDraft {
    fn capture(brep: &Brep, face: &BrepFace) -> Option<Self> {
        let mut uses = BTreeMap::<usize, usize>::new();
        for edge in face
            .loops()
            .iter()
            .flat_map(|boundary| boundary.trims())
            .filter_map(|trim| trim.edge())
        {
            *uses.entry(edge).or_default() += 1;
        }
        let mut draft = Self {
            rim: Vec::new(),
            rulings: Vec::new(),
        };
        let mut corners = std::collections::BTreeSet::new();
        // Same-face seams and singular trims generate no lateral wall.
        for (index, count) in uses {
            if count != 1 {
                continue;
            }
            let edge = &brep.edges()[index];
            corners.extend(edge.vertices());
            let curve = edge.curve();
            curve.visit_segments(|a, b| draft.rim.push([a, b]));
            let parameter = curve.domain();
            if let Ok(point) = curve.evaluate(parameter.start().midpoint(*parameter.end())) {
                draft.rulings.push(point);
            }
        }
        draft.rulings.extend(
            corners
                .into_iter()
                .map(|index| brep.vertices()[index].point()),
        );
        (!draft.rim.is_empty()).then_some(draft)
    }
}

impl Viewport {
    pub(super) fn paint_surface_apex(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        draft: &SurfaceApexDraft,
        apex: Point3,
        tolerance: Tolerance,
    ) {
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
        if let Some(boundary) = &draft.boundary {
            let mut cache = boundary.frame.borrow_mut();
            if cache
                .as_ref()
                .is_none_or(|frame| frame.apex != apex || frame.tolerance != tolerance)
            {
                let edges = boundary
                    .clipped_edges(draft, apex, tolerance)
                    .unwrap_or_default();
                *cache = Some(BoundaryFrame {
                    apex,
                    tolerance,
                    edges,
                });
            }
            for &[a, b] in &cache.as_ref().unwrap().edges {
                if let Some(segment) = self.project_segment(a, b, rect) {
                    painter.line_segment(segment, stroke);
                }
            }
            self.paint_draft_points(painter, rect, &[apex]);
            return;
        }
        for face in &draft.faces {
            let rulings = face.rulings.iter().map(|&start| [start, apex]);
            // The halfway section lies on each ruled wall. Averaging finite
            // endpoints this way also handles model coordinates of opposite sign.
            let section = face
                .rim
                .iter()
                .filter_map(|&[a, b]| Some([a.midpoint(apex).ok()?, b.midpoint(apex).ok()?]));
            for [a, b] in face.rim.iter().copied().chain(rulings).chain(section) {
                if let Some(segment) = self.project_segment(a, b, rect) {
                    painter.line_segment(segment, stroke);
                }
            }
        }
        if !draft.faces.is_empty() {
            self.paint_draft_points(painter, rect, &[apex]);
        }
    }
}
