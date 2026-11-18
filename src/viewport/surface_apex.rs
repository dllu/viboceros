//! Cached source boundaries for the live ruled surface-to-point preview.
use super::{curve_sampling::ViewportCurve, *};
use std::collections::BTreeMap;
use viboceros_geometry::{Brep, BrepFace};

#[derive(Debug)]
pub(crate) struct SurfaceApexDraft {
    faces: Vec<FaceDraft>,
    pub(crate) hover: Option<Point3>,
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
        Self { faces, hover: None }
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
    ) {
        let stroke = Stroke::new(2., Color32::from_rgb(20, 115, 190));
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
