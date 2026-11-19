//! Cached source boundaries for the live ruled surface-to-point preview.
use super::{curve_sampling::ViewportCurve, *};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
use viboceros_geometry::{Brep, BrepFace, MeshProjectionIndex, Plane};

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
    walls: Vec<WallDraft>,
    plane: Option<Plane>,
}

#[derive(Debug)]
struct WallDraft {
    rim: Vec<[Point3; 2]>,
    corners: [(usize, Point3); 2],
    endpoints: [Point3; 2],
    midpoint: Option<Point3>,
}

impl SurfaceApexDraft {
    pub(crate) fn capture(document: &Document, faces: &[(ObjectId, usize)]) -> Self {
        let tolerance = document.tolerance();
        let faces = faces
            .iter()
            .filter_map(|&(id, index)| match document.object(id)?.geometry() {
                Geometry::Brep(brep) => {
                    FaceDraft::capture(brep, brep.faces().get(index)?, tolerance)
                }
                Geometry::NurbsSurface(surface) => {
                    let brep = Brep::try_surface_face(surface.clone(), tolerance).ok()?;
                    FaceDraft::capture(&brep, brep.faces().first()?, tolerance)
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
            for &[a, b] in face.walls.iter().flat_map(|wall| &wall.rim) {
                edges.push([a, b]);
                edges.push([
                    self.project(a, apex, tolerance)?,
                    self.project(b, apex, tolerance)?,
                ]);
            }
            for start in FaceDraft::rulings(face.walls.iter()) {
                edges.push([start, self.project(start, apex, tolerance)?]);
            }
        }
        Some(edges)
    }
}

impl FaceDraft {
    fn capture(brep: &Brep, face: &BrepFace, tolerance: Tolerance) -> Option<Self> {
        let mut uses = BTreeMap::<usize, usize>::new();
        for edge in face
            .loops()
            .iter()
            .flat_map(|boundary| boundary.trims())
            .filter_map(|trim| trim.edge())
        {
            *uses.entry(edge).or_default() += 1;
        }
        // Same-face seams and singular trims generate no lateral wall.
        let walls: Vec<_> = uses
            .into_iter()
            .filter(|&(_, count)| count == 1)
            .filter_map(|(index, _)| {
                let edge = &brep.edges()[index];
                let curve = edge.curve();
                let domain = curve.domain();
                let corners = edge.vertices().map(|v| (v, brep.vertices()[v].point()));
                let endpoints = [
                    curve.evaluate(*domain.start()).unwrap_or(corners[0].1),
                    curve.evaluate(*domain.end()).unwrap_or(corners[1].1),
                ];
                let mut rim = Vec::new();
                curve.visit_segments(|a, b| rim.push([a, b]));
                (!rim.is_empty()).then_some(WallDraft {
                    rim,
                    corners,
                    endpoints,
                    midpoint: curve.evaluate(domain.start().midpoint(*domain.end())).ok(),
                })
            })
            .collect();
        (!walls.is_empty()).then(|| Self {
            walls,
            plane: face.surface().plane(tolerance).ok().flatten(),
        })
    }

    fn visible_walls(
        &self,
        apex: Point3,
        tolerance: Tolerance,
    ) -> impl Iterator<Item = &WallDraft> + Clone {
        let coplanar = self
            .plane
            .and_then(|plane| plane.signed_distance_to(apex).ok())
            .is_some_and(|distance| distance.abs() <= tolerance.absolute());
        self.walls.iter().filter(move |wall| {
            !coplanar
                || !(wall.endpoints.contains(&apex) || wall.corners.iter().any(|&(_, p)| p == apex))
        })
    }

    fn rulings<'a>(walls: impl Iterator<Item = &'a WallDraft>) -> impl Iterator<Item = Point3> {
        let mut corners = BTreeSet::new();
        walls.flat_map(move |wall| {
            let endpoints = wall
                .corners
                .map(|(index, point)| corners.insert(index).then_some(point));
            endpoints.into_iter().flatten().chain(wall.midpoint)
        })
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
            let walls = face.visible_walls(apex, tolerance);
            let rims = walls.clone().flat_map(|wall| wall.rim.iter().copied());
            let rulings = FaceDraft::rulings(walls).map(|start| [start, apex]);
            // The halfway section lies on each ruled wall. Averaging finite
            // endpoints this way also handles model coordinates of opposite sign.
            let section = rims
                .clone()
                .filter_map(|[a, b]| Some([a.midpoint(apex).ok()?, b.midpoint(apex).ok()?]));
            for [a, b] in rims.chain(rulings).chain(section) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::NurbsSurface;

    #[test]
    fn surface_apex_preview_omits_walls_at_an_exact_coplanar_endpoint() {
        let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let tolerance = Tolerance::DEFAULT;
        let corner = point(0., 0., 0.);
        let surface = NurbsSurface::try_bilinear([
            corner,
            point(2., 0., 0.),
            point(2., 3., 0.),
            point(0., 3., 0.),
        ])
        .unwrap();
        let face = Brep::try_surface_face(surface, tolerance).unwrap();
        let draft = FaceDraft::capture(&face, &face.faces()[0], tolerance).unwrap();
        assert_eq!(draft.visible_walls(corner, tolerance).count(), 2);
        for nearby in [
            point(0., 0., 1e-8),
            point(1e-8, 1e-8, 0.),
            point(1., 0., 0.),
        ] {
            assert_eq!(draft.visible_walls(nearby, tolerance).count(), 4);
        }
    }
}
