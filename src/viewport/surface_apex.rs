//! Cached source boundaries for the live ruled surface-to-point preview.
use super::{curve_sampling::ViewportCurve, *};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
use viboceros_geometry::{Brep, BrepFace, MeshProjectionIndex, Plane};

mod apertures;

#[derive(Debug)]
pub(crate) struct SurfaceApexDraft {
    faces: Vec<FaceDraft>,
    pub(crate) hover: Option<Point3>,
    boundary: Option<BoundaryDraft>,
}

#[derive(Debug)]
struct BoundaryDraft {
    projection: Option<BoundaryProjection>,
    apertures: Option<apertures::SectionApertures>,
    frame: RefCell<Option<BoundaryFrame>>,
}

#[derive(Debug)]
enum BoundaryProjection {
    Mesh(MeshProjectionIndex<'static>),
    Planar { plane: Plane, face: Box<BrepFace> },
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
    corners: [Point3; 2],
    endpoints: [Point3; 2],
    midpoint: Option<Point3>,
}

impl SurfaceApexDraft {
    pub(crate) fn capture(
        document: &Document,
        targets: &[(ObjectId, usize)],
        split_at_tangents: bool,
    ) -> Self {
        Self {
            faces: FaceDraft::capture_sources(document, targets, split_at_tangents),
            hover: None,
            boundary: None,
        }
    }

    pub(crate) fn set_tangent_splits(
        &mut self,
        document: &Document,
        targets: &[(ObjectId, usize)],
        split_at_tangents: bool,
    ) {
        self.faces = FaceDraft::capture_sources(document, targets, split_at_tangents);
        if let Some(boundary) = &mut self.boundary {
            *boundary.frame.get_mut() = None;
        }
    }

    pub(crate) fn set_boundary(&mut self, boundary: Option<&Brep>, tolerance: Tolerance) {
        self.hover = None;
        self.boundary = boundary.map(|brep| BoundaryDraft {
            projection: BoundaryProjection::capture(brep, tolerance),
            apertures: apertures::SectionApertures::capture(brep, tolerance),
            frame: RefCell::new(None),
        });
    }
}

impl BoundaryDraft {
    fn project(
        &self,
        start: Point3,
        apex: Point3,
        reject_competing: bool,
        tolerance: Tolerance,
    ) -> Option<Point3> {
        match self.projection.as_ref()? {
            BoundaryProjection::Mesh(mesh) => {
                BoundaryProjection::mesh_hit(mesh, start, apex, reject_competing, tolerance)
            }
            BoundaryProjection::Planar { plane, face } => {
                BoundaryProjection::planar_hit(*plane, face, start, apex, tolerance)
            }
        }
    }

    fn clipped_edges(
        &self,
        source: &SurfaceApexDraft,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Option<Vec<[Point3; 2]>> {
        let mut edges = Vec::new();
        for face in &source.faces {
            let mut section = Vec::new();
            let near_plane = face.plane.is_some_and(|plane| {
                plane
                    .signed_distance_to(apex)
                    .is_ok_and(|distance| distance.abs() <= tolerance.absolute())
            });
            for &[a, b] in face.walls.iter().flat_map(|wall| &wall.rim) {
                let cut = [
                    self.project(a, apex, near_plane, tolerance)?,
                    self.project(b, apex, near_plane, tolerance)?,
                ];
                edges.extend([[a, b], cut]);
                if self.apertures.is_some() {
                    section.push(cut);
                }
            }
            for start in FaceDraft::rulings(face.walls.iter()) {
                edges.push([start, self.project(start, apex, near_plane, tolerance)?]);
            }
            if let Some(apertures) = &self.apertures {
                apertures.append(&section, apex, face.plane, &mut edges)?;
            }
        }
        Some(edges)
    }
}

impl BoundaryProjection {
    fn capture(brep: &Brep, tolerance: Tolerance) -> Option<Self> {
        if let Some(mesh) = brep
            .display_mesh(8, tolerance)
            .ok()
            .and_then(|mesh| MeshProjectionIndex::new_owned(mesh).ok())
        {
            return Some(Self::Mesh(mesh));
        }
        // A valid trimmed plane can contain detail too small for a
        // display mesh. Use the actual face trims to check those hits.
        let [face] = brep.faces() else {
            return None;
        };
        Some(Self::Planar {
            plane: face.surface().plane(tolerance).ok()??,
            face: Box::new(face.clone()),
        })
    }

    fn mesh_hit(
        mesh: &MeshProjectionIndex<'_>,
        start: Point3,
        apex: Point3,
        reject_competing: bool,
        tolerance: Tolerance,
    ) -> Option<Point3> {
        let direction = start.direction_to(apex).ok()?.as_vector();
        let length = start.distance_to(apex).ok()?;
        let hits = mesh.project_point(start, direction, tolerance).ok()?;
        // Mesh projection returns hits in increasing line parameter.
        let mut hits = hits.into_iter().filter_map(|point| {
            let distance = direction.dot(start.vector_to(point).ok()?).ok()?;
            (distance > tolerance.absolute() && distance < length - tolerance.absolute())
                .then_some(point)
        });
        let first = hits.next()?;
        if reject_competing && hits.any(|other| !other.is_near(first, tolerance)) {
            return None;
        }
        Some(first)
    }

    fn planar_hit(
        plane: Plane,
        face: &BrepFace,
        start: Point3,
        apex: Point3,
        tolerance: Tolerance,
    ) -> Option<Point3> {
        let a = plane.signed_distance_to(start).ok()?;
        let b = plane.signed_distance_to(apex).ok()?;
        let fraction = a / (a - b);
        let margin = tolerance.absolute() / start.distance_to(apex).ok()?;
        if !(margin < fraction && fraction < 1. - margin) {
            return None;
        }
        let point = start
            .translated(start.vector_to(apex).ok()?.scaled(fraction).ok()?)
            .ok()?;
        let (u, v) = face.surface().closest_parameters(point, tolerance).ok()?;
        let on_surface = face.surface().evaluate(u, v).ok()?;
        (on_surface.is_near(point, tolerance) && face.contains_parameters(u, v, tolerance).ok()?)
            .then_some(point)
    }
}

impl FaceDraft {
    fn capture_sources(
        document: &Document,
        targets: &[(ObjectId, usize)],
        split_at_tangents: bool,
    ) -> Vec<Self> {
        let tolerance = document.tolerance();
        targets
            .iter()
            .filter_map(|&(id, index)| match document.object(id)?.geometry() {
                Geometry::Brep(brep) => {
                    Self::capture(brep, brep.faces().get(index)?, split_at_tangents, tolerance)
                }
                Geometry::NurbsSurface(surface) => {
                    let brep = Brep::try_surface_face(surface.clone(), tolerance).ok()?;
                    Self::capture(&brep, brep.faces().first()?, split_at_tangents, tolerance)
                }
                _ => None,
            })
            .collect()
    }

    fn capture(
        brep: &Brep,
        face: &BrepFace,
        split_at_tangents: bool,
        tolerance: Tolerance,
    ) -> Option<Self> {
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
            .flat_map(|(index, _)| {
                let edge = &brep.edges()[index];
                let corners = edge.vertices().map(|v| brep.vertices()[v].point());
                WallDraft::capture_edge(edge.curve(), corners, split_at_tangents)
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
            !coplanar || !(wall.endpoints.contains(&apex) || wall.corners.contains(&apex))
        })
    }

    fn rulings<'a>(walls: impl Iterator<Item = &'a WallDraft>) -> impl Iterator<Item = Point3> {
        let mut corners = BTreeSet::new();
        walls.flat_map(move |wall| {
            let endpoints = wall.corners.map(|point| {
                let key = point
                    .to_array()
                    .map(|x| if x == 0.0 { 0 } else { x.to_bits() });
                corners.insert(key).then_some(point)
            });
            endpoints.into_iter().flatten().chain(wall.midpoint)
        })
    }
}

impl WallDraft {
    fn capture_edge(
        source: &NurbsCurve,
        corners: [Point3; 2],
        split_at_tangents: bool,
    ) -> Vec<Self> {
        let segments = source
            .surface_apex_wall_segments(split_at_tangents)
            .unwrap_or_else(|_| vec![source.clone()]);
        let last = segments.len() - 1;
        segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| {
                let domain = segment.domain();
                // Use the unsplit curve at both sides of an interior break
                // so adjacent pieces share one preview ruling.
                let corners = [
                    if index == 0 {
                        corners[0]
                    } else {
                        source.evaluate(*domain.start()).ok()?
                    },
                    if index == last {
                        corners[1]
                    } else {
                        source.evaluate(*domain.end()).ok()?
                    },
                ];
                let endpoints = [
                    segment.evaluate(*domain.start()).unwrap_or(corners[0]),
                    segment.evaluate(*domain.end()).unwrap_or(corners[1]),
                ];
                let mut rim = Vec::new();
                segment.visit_segments(|a, b| rim.push([a, b]));
                (!rim.is_empty()).then_some(Self {
                    rim,
                    corners,
                    endpoints,
                    midpoint: segment
                        .evaluate(domain.start().midpoint(*domain.end()))
                        .ok(),
                })
            })
            .collect()
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
        let draft = FaceDraft::capture(&face, &face.faces()[0], false, tolerance).unwrap();
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
