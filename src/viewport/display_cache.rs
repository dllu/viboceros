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
    entries: HashMap<ObjectId, Rc<DisplayGeometry>>,
}

impl DisplayCache {
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
    }
}

pub(super) struct DisplayGeometry {
    pub(super) geometry: GeometrySnapshot,
    wire_density: i32,
    tolerance: Tolerance,
    wires: OnceCell<Vec<[Point3; 2]>>,
    mesh: OnceCell<Option<TriangleMesh>>,
    normals: OnceCell<Vec<[NaVector3<Real>; 3]>>,
    edges: OnceCell<Vec<Vec<[Point3; 2]>>>,
}

impl DisplayGeometry {
    pub(super) fn new(geometry: GeometrySnapshot, wire_density: i32, tolerance: Tolerance) -> Self {
        Self {
            geometry,
            wire_density,
            tolerance,
            wires: OnceCell::new(),
            mesh: OnceCell::new(),
            normals: OnceCell::new(),
            edges: OnceCell::new(),
        }
    }

    pub(super) fn wires(&self) -> &[[Point3; 2]] {
        self.wires.get_or_init(|| {
            let mut lines = Vec::new();
            let mut visit = |a, b| lines.push([a, b]);
            match &*self.geometry {
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
                Geometry::Mesh(mesh) => {
                    if let Ok(edges) = mesh.wireframe_lines(self.tolerance) {
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
        self.mesh
            .get_or_init(|| match &*self.geometry {
                Geometry::NurbsSurface(surface) => surface
                    .tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)
                    .ok(),
                Geometry::Brep(brep) => brep
                    .tessellate(SURFACE_SAMPLES_PER_SPAN, self.tolerance)
                    .ok(),
                _ => None,
            })
            .as_ref()
    }

    pub(super) fn normals(&self) -> &[[NaVector3<Real>; 3]] {
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
        [
            ViewKind::Top,
            ViewKind::Perspective,
            ViewKind::Front,
            ViewKind::Right,
        ]
        .map(|kind| {
            let mut view = Self::new(kind);
            view.display_cache = Rc::clone(&cache);
            view
        })
    }
}

#[cfg(test)]
mod tests;
