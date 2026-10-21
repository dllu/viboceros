//! Camera-independent display data shared by the four viewports.
//!
//! Immutable document snapshots give constant-time identity checks without
//! rescanning geometry. Edits install new snapshots; history restores the old
//! handles. Retaining ownership prevents address reuse from masquerading as a hit.

use super::*;
use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use viboceros_document::{GeometrySnapshot, Object};

#[derive(Default)]
pub(super) struct DisplayCache {
    analyses: Vec<AnalysisDrawing>,
    entries: HashMap<ObjectId, Rc<DisplayGeometry>>,
    temporary_sources: HashMap<ObjectId, Rc<DisplayGeometry>>,
}

struct AnalysisDrawing {
    edges: std::sync::Arc<[viboceros_command::edge_analysis::Edge]>,
    segments: HashMap<(ObjectId, usize), Vec<[Point3; 2]>>,
}

#[cfg(test)]
mod edge_analysis_tests {
    use super::*;
    #[test]
    fn persistent_panel_and_transient_zoom_reuse_both_sample_buffers() {
        let registry = viboceros_command::CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 2,0,0 2,3,0 0,3,0")
            .unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry.execute(&mut doc, "ShowEdges").unwrap();
        let panel = registry.edge_analysis_view(&doc).unwrap().unwrap();
        registry.execute(&mut doc, "ZoomNaked").unwrap();
        let zoom = registry.edge_zoom_view(&doc).unwrap().unwrap();
        let key = (panel.edges[0].object, panel.edges[0].index);
        let mut cache = DisplayCache::default();
        let first = cache.analysis_segments(&panel.edges)[&key].as_ptr();
        let second = cache.analysis_segments(&zoom.edges)[&key].as_ptr();
        assert_eq!(cache.analyses.len(), 2);
        for _ in 0..4 {
            assert_eq!(cache.analysis_segments(&panel.edges)[&key].as_ptr(), first);
            assert_eq!(cache.analysis_segments(&zoom.edges)[&key].as_ptr(), second);
        }
        assert_eq!(cache.analyses.len(), 2);
    }
}

impl DisplayCache {
    pub(super) fn analysis_segments(
        &mut self,
        edges: &std::sync::Arc<[viboceros_command::edge_analysis::Edge]>,
    ) -> &HashMap<(ObjectId, usize), Vec<[Point3; 2]>> {
        if let Some(index) = self
            .analyses
            .iter()
            .position(|cache| std::sync::Arc::ptr_eq(&cache.edges, edges))
        {
            return &self.analyses[index].segments;
        }
        {
            let mut segments = HashMap::new();
            for edge in edges.iter() {
                let mut lines = Vec::new();
                if edge.curve.degree() == 1
                    && edge.curve.control_points().iter().all(|p| p.weight() > 0.)
                {
                    lines.extend(
                        edge.curve
                            .control_points()
                            .windows(2)
                            .map(|p| [p[0].point(), p[1].point()]),
                    );
                } else {
                    edge.curve.visit_segments(|a, b| lines.push([a, b]));
                }
                segments.insert((edge.object, edge.index), lines);
            }
            if self.analyses.len() == 2 {
                self.analyses.remove(0);
            }
            self.analyses.push(AnalysisDrawing {
                edges: edges.clone(),
                segments,
            });
        }
        &self.analyses.last().unwrap().segments
    }

    pub(super) fn get(&mut self, object: &Object, tolerance: Tolerance) -> Rc<DisplayGeometry> {
        let density = object.attributes().wire_density();
        let entry = self.entries.entry(object.id()).or_insert_with(|| {
            Rc::new(DisplayGeometry::new(
                object.geometry_snapshot().clone(),
                density,
                tolerance,
            ))
        });
        if !entry
            .geometry
            .shares_storage_with(object.geometry_snapshot())
            || entry.wire_density != density
            || entry.tolerance != tolerance
        {
            *entry = Rc::new(DisplayGeometry::new(
                object.geometry_snapshot().clone(),
                density,
                tolerance,
            ));
        }
        Rc::clone(entry)
    }

    pub(super) fn retain_visible(&mut self, ids: &HashSet<ObjectId>) {
        self.entries.retain(|id, _| ids.contains(id));
        self.temporary_sources.retain(|id, _| ids.contains(id));
    }

    pub(super) fn temporary_source(
        &mut self,
        object: &Object,
        geometry: &GeometrySnapshot,
        tolerance: Tolerance,
    ) -> Rc<DisplayGeometry> {
        let density = object.attributes().wire_density();
        let entry = self
            .temporary_sources
            .entry(object.id())
            .or_insert_with(|| Rc::new(DisplayGeometry::new(geometry.clone(), density, tolerance)));
        if !entry.geometry.shares_storage_with(geometry)
            || entry.wire_density != density
            || entry.tolerance != tolerance
        {
            *entry = Rc::new(DisplayGeometry::new(geometry.clone(), density, tolerance));
        }
        Rc::clone(entry)
    }
}

pub(super) struct DisplayGeometry {
    pub(super) geometry: GeometrySnapshot,
    bounds: OnceCell<viboceros_geometry::BoundingBox3>,
    wire_density: i32,
    tolerance: Tolerance,
    wires: OnceCell<Vec<[Point3; 2]>>,
    mesh: OnceCell<Option<GeneratedDisplayMesh>>,
    normals: OnceCell<Vec<[NaVector3<Real>; 3]>>,
    edges: OnceCell<Vec<Vec<[Point3; 2]>>>,
    block_members: OnceCell<Vec<Rc<DisplayGeometry>>>,
    pub(super) component_regions: OnceCell<super::polygon_components::FaceRegions>,
}

struct GeneratedDisplayMesh {
    mesh: TriangleMesh,
    normals: Option<Vec<[NaVector3<Real>; 3]>>,
    face_sources: Option<Vec<usize>>,
}

impl DisplayGeometry {
    pub(super) fn with_wires(
        geometry: GeometrySnapshot,
        wire_density: i32,
        tolerance: Tolerance,
        wires: Vec<[Point3; 2]>,
    ) -> Self {
        let result = Self::new(geometry, wire_density, tolerance);
        if let Ok(bounds) =
            viboceros_geometry::BoundingBox3::from_points(wires.iter().flatten().copied())
        {
            let _ = result.bounds.set(bounds);
        }
        let _ = result.wires.set(wires);
        result
    }
    pub(super) fn new(geometry: GeometrySnapshot, wire_density: i32, tolerance: Tolerance) -> Self {
        Self {
            geometry,
            bounds: OnceCell::new(),
            wire_density,
            tolerance,
            wires: OnceCell::new(),
            mesh: OnceCell::new(),
            normals: OnceCell::new(),
            edges: OnceCell::new(),
            block_members: OnceCell::new(),
            component_regions: OnceCell::new(),
        }
    }

    pub(super) fn bounds(&self) -> viboceros_geometry::BoundingBox3 {
        *self.bounds.get_or_init(|| self.geometry.bounds())
    }

    pub(super) fn block_members(&self) -> &[Rc<DisplayGeometry>] {
        self.block_members.get_or_init(|| match &*self.geometry {
            Geometry::BlockInstance(instance) => instance
                .members()
                .iter()
                .map(|member| {
                    Rc::new(Self::new(
                        member.geometry.clone(),
                        member.attributes.wire_density(),
                        self.tolerance,
                    ))
                })
                .collect(),
            _ => Vec::new(),
        })
    }

    pub(super) fn wires(&self) -> &[[Point3; 2]] {
        self.wires.get_or_init(|| {
            let mut lines = Vec::new();
            let mut visit = |a, b| lines.push([a, b]);
            match &*self.geometry {
                Geometry::BlockInstance(_) => {
                    for display in self.block_members() {
                        lines.extend_from_slice(display.wires());
                    }
                }
                Geometry::Point(_) | Geometry::PointCloud(_) => {}
                Geometry::Line(line) => visit(line.start(), line.end()),
                Geometry::Circle(circle) => curve_sampling::visit_parametric_segments(
                    CIRCLE_SAMPLES,
                    |t| circle.point_at_angle(std::f64::consts::TAU * t),
                    &mut visit,
                ),
                Geometry::Arc(arc) => curve_sampling::visit_parametric_segments(
                    circular_arc_samples(*arc),
                    |t| arc.point_at(t),
                    &mut visit,
                ),
                Geometry::Ellipse(ellipse) => curve_sampling::visit_parametric_segments(
                    CIRCLE_SAMPLES,
                    |t| ellipse.point_at_angle(std::f64::consts::TAU * t),
                    &mut visit,
                ),
                Geometry::Polyline(curve) => {
                    for segment in curve.segments() {
                        visit(segment.start(), segment.end());
                    }
                }
                Geometry::NurbsCurve(curve) => curve.visit_segments(visit),
                Geometry::PolyCurve(curve) => {
                    for segment in curve.segments() {
                        segment.visit_segments(&mut visit);
                    }
                }
                Geometry::NurbsSurface(surface) => {
                    if let Ok(curves) = surface.wireframe_curves(self.wire_density) {
                        for curve in curves {
                            curve.visit_segments(&mut visit);
                        }
                    }
                }
                Geometry::Brep(brep) => {
                    if let Ok(curves) = brep.wireframe_curves(self.wire_density, self.tolerance) {
                        for curve in curves {
                            curve.visit_segments(&mut visit);
                        }
                    }
                }
                Geometry::SubD(_) => {
                    if let Some(mesh) = self.mesh()
                        && let Ok(edges) = mesh.visible_wireframe_lines(self.tolerance)
                    {
                        for edge in edges {
                            visit(edge.start(), edge.end());
                        }
                    }
                }
                Geometry::Mesh(mesh) => {
                    if let Ok(edges) = mesh.visible_wireframe_lines(self.tolerance) {
                        for edge in edges {
                            visit(edge.start(), edge.end());
                        }
                    }
                }
            }
            lines
        })
    }

    pub(super) fn mesh(&self) -> Option<&TriangleMesh> {
        // Mesh objects already own exactly the display mesh; don't copy it twice.
        if let Geometry::Mesh(mesh) = &*self.geometry {
            return Some(mesh);
        }
        self.generated_mesh().map(|generated| &generated.mesh)
    }

    pub(super) fn face_sources(&self) -> Option<&[usize]> {
        self.generated_mesh()?.face_sources.as_deref()
    }

    fn generated_mesh(&self) -> Option<&GeneratedDisplayMesh> {
        self.mesh
            .get_or_init(|| match &*self.geometry {
                Geometry::SubD(subd) => Self::subd_display(subd),
                Geometry::NurbsSurface(surface) => surface
                    .tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)
                    .ok()
                    .map(|mesh| GeneratedDisplayMesh {
                        mesh,
                        normals: None,
                        face_sources: None,
                    }),
                Geometry::Brep(brep) => brep
                    .display_mesh_with_face_sources(SURFACE_SAMPLES_PER_SPAN, self.tolerance)
                    .ok()
                    .map(|(mesh, face_sources)| GeneratedDisplayMesh {
                        mesh,
                        normals: None,
                        face_sources: Some(face_sources),
                    }),
                _ => None,
            })
            .as_ref()
    }

    fn subd_display(subd: &viboceros_geometry::SubD) -> Option<GeneratedDisplayMesh> {
        let display = subd.display_mesh_with_normals().ok()?;
        let mesh = display.mesh;
        if display.face_corner_normals.len() != mesh.faces().len()
            || display.face_sources.len() != mesh.face_count()
        {
            return None;
        }
        let mut normals = Vec::with_capacity(mesh.triangles().len());
        for (face_index, (face, native)) in mesh
            .faces()
            .iter()
            .zip(display.face_corner_normals)
            .enumerate()
        {
            let indices = face.indices();
            for (offset, triangle) in mesh.triangles()[2 * face_index..2 * face_index + 2]
                .iter()
                .enumerate()
            {
                let fallback = mesh
                    .face_normal(2 * face_index + offset)
                    .map(|n| NaVector3::new(n.x(), n.y(), n.z()))
                    .unwrap_or_else(|_| NaVector3::new(0., 0., 1.));
                normals.push(triangle.map(|vertex| {
                    let n = native[indices.iter().position(|i| *i == vertex).unwrap()].to_array();
                    NaVector3::new(n[0], n[1], n[2])
                        .try_normalize(Real::EPSILON)
                        .unwrap_or(fallback)
                }));
            }
        }
        Some(GeneratedDisplayMesh {
            mesh,
            normals: Some(normals),
            face_sources: Some(display.face_sources),
        })
    }
    pub(super) fn normals(&self) -> &[[NaVector3<Real>; 3]] {
        if let Some(normals) = self
            .generated_mesh()
            .and_then(|mesh| mesh.normals.as_deref())
        {
            return normals;
        }
        self.normals.get_or_init(|| {
            self.mesh()
                .map(scene::smooth_corner_normals)
                .unwrap_or_default()
        })
    }

    /// Boundary components only: surface isocurves are display aids, not edges.
    /// Sampling matches the displayed wires and is shared across viewports.
    pub(super) fn edges(&self) -> &[Vec<[Point3; 2]>] {
        self.edges.get_or_init(|| {
            let converted;
            let brep = match &*self.geometry {
                Geometry::Brep(brep) => brep,
                Geometry::NurbsSurface(surface) => {
                    let Ok(brep) =
                        viboceros_geometry::Brep::try_surface_face(surface.clone(), self.tolerance)
                    else {
                        return Vec::new();
                    };
                    converted = brep;
                    &converted
                }
                _ => return Vec::new(),
            };
            brep.edges()
                .iter()
                .map(|edge| {
                    let mut segments = Vec::new();
                    edge.curve().visit_segments(|a, b| segments.push([a, b]));
                    segments
                })
                .collect()
        })
    }
}

impl Viewport {
    pub(crate) fn standard_views() -> [Self; 4] {
        let cache = Rc::new(RefCell::new(DisplayCache::default()));
        let grips = Rc::new(RefCell::new(
            super::grip_preview::GripPreviewCache::default(),
        ));
        [
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right,
        ]
        .map(|kind| {
            let mut view = Self::new(kind);
            view.display_cache = Rc::clone(&cache);
            view.grip_preview_cache = Rc::clone(&grips);
            view
        })
    }
}

#[cfg(test)]
mod tests;
