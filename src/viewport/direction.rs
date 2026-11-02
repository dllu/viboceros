//! Direction arrows use staged geometry while picks retain document identities.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::direction::DirectionSession;
use viboceros_geometry::{BrepFace, NurbsSurface};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(crate) struct Marker {
    pub object: ObjectId,
    pub point: Point3,
    // Curve tangent or surface U, V, oriented normal.
    pub axes: [Option<Vector3>; 3],
    pub surface: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Overlay<'a> {
    pub edits: &'a DirectionSession,
    pub ids: &'a BTreeSet<ObjectId>,
    pub markers: &'a [Marker],
}

pub(crate) fn collect_markers(edits: &DirectionSession, tolerance: Tolerance) -> Vec<Marker> {
    let mut markers = Vec::new();
    for (id, geometry) in edits.geometries() {
        if let Some(curve) = geometry.curve_ref() {
            let domain = curve.domain();
            for fraction in [0.15, 0.5, 0.85] {
                let t = *domain.start() * (1. - fraction) + *domain.end() * fraction;
                if let Ok(sample) = curve.evaluate_with_tangent(t) {
                    markers.push(Marker {
                        object: id,
                        point: sample.point(),
                        axes: [Some(sample.tangent().as_vector()), None, None],
                        surface: false,
                    });
                }
            }
        } else {
            match geometry {
                Geometry::NurbsSurface(surface) => {
                    if let Some(marker) = surface_marker(id, surface, None, None, tolerance) {
                        markers.push(marker);
                    }
                }
                Geometry::Brep(brep) => {
                    for face in brep.faces() {
                        if let Some(marker) =
                            surface_marker(id, face.surface(), Some(face), None, tolerance)
                        {
                            markers.push(marker);
                        }
                    }
                }
                Geometry::Mesh(mesh) => {
                    // Bound annotation density independently of triangle count.
                    for face in (0..mesh.triangles().len())
                        .step_by(mesh.triangles().len().div_ceil(256).max(1))
                    {
                        if let Some(marker) = mesh_marker(id, mesh, face) {
                            markers.push(marker);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    markers
}

fn surface_marker(
    id: ObjectId,
    surface: &NurbsSurface,
    face: Option<&BrepFace>,
    point: Option<Point3>,
    tolerance: Tolerance,
) -> Option<Marker> {
    let (u, v) = if let Some(point) = point {
        surface.closest_parameters(point, tolerance).ok()?
    } else {
        let mut parameter = None;
        'outer: for u in [0.5, 0.25, 0.75, 0.125, 0.875] {
            for v in [0.5, 0.25, 0.75, 0.125, 0.875] {
                let u = *surface.domain_u().start() * (1. - u) + *surface.domain_u().end() * u;
                let v = *surface.domain_v().start() * (1. - v) + *surface.domain_v().end() * v;
                if face.is_none_or(|f| f.contains_parameters(u, v, tolerance).unwrap_or(false)) {
                    parameter = Some((u, v));
                    break 'outer;
                }
            }
        }
        parameter?
    };
    if face.is_some_and(|f| !f.contains_parameters(u, v, tolerance).unwrap_or(false)) {
        return None;
    }
    let (point, du, dv) = surface.evaluate_with_derivatives(u, v).ok()?;
    let normal = surface.normal_at(u, v).ok()?.as_vector();
    Some(Marker {
        object: id,
        point,
        surface: true,
        axes: [
            du.normalized_nonzero().ok().map(|v| v.as_vector()),
            dv.normalized_nonzero().ok().map(|v| v.as_vector()),
            Some(if face.is_some_and(|f| f.is_reversed()) {
                normal.scaled(-1.).ok()?
            } else {
                normal
            }),
        ],
    })
}

fn mesh_marker(id: ObjectId, mesh: &TriangleMesh, face: usize) -> Option<Marker> {
    let [a, b, c] = mesh
        .triangles()
        .get(face)?
        .map(|i| mesh.vertices()[i as usize]);
    let point = Point3::try_new(
        a.x() / 3. + b.x() / 3. + c.x() / 3.,
        a.y() / 3. + b.y() / 3. + c.y() / 3.,
        a.z() / 3. + b.z() / 3. + c.z() / 3.,
    )
    .ok()?;
    Some(Marker {
        object: id,
        point,
        axes: [None, None, Some(mesh.face_normal(face).ok()?.as_vector())],
        surface: false,
    })
}

impl Viewport {
    pub(super) fn direction_hit(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        overlay: Overlay<'_>,
    ) -> Option<(ObjectId, Option<Marker>)> {
        if !pointer.is_finite() || !rect.contains(pointer) {
            return None;
        }
        // A Dir surface remains pickable across its face in wireframe too.
        let face_hit = self.pick_selected_face_with_point_restricted(
            pointer,
            rect,
            document,
            FacePickMode::Any,
            None,
            Some(overlay.ids),
        );
        let curve_hit = overlay
            .edits
            .geometries()
            .filter(|(_, g)| g.curve_ref().is_some())
            .filter_map(|(id, g)| {
                let curve = g.curve_ref()?.to_nurbs().ok()?;
                let t = self.pick_edge_parameter(&curve, pointer, rect, false)?;
                let sample = g.curve_ref()?.evaluate_with_tangent(t).ok()?;
                let distance = self.project(sample.point(), rect)?.distance(pointer);
                (distance <= PICK_CAPTURE_PIXELS).then_some((
                    distance,
                    id,
                    Marker {
                        object: id,
                        point: sample.point(),
                        axes: [Some(sample.tangent().as_vector()), None, None],
                        surface: false,
                    },
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, id, marker)) = curve_hit {
            return Some((id, Some(marker)));
        }
        let (id, face_index, point) = face_hit?;
        let geometry = overlay.edits.geometries().find(|(key, _)| *key == id)?.1;
        let marker = match geometry {
            Geometry::Brep(brep) => {
                let face = brep.faces().get(face_index)?;
                let point = self
                    .normal_surface_point(pointer, rect, document, (id, Some(face_index), false))
                    .or(point);
                surface_marker(id, face.surface(), Some(face), point, document.tolerance())
            }
            Geometry::NurbsSurface(surface) => {
                let point = self
                    .normal_surface_point(pointer, rect, document, (id, None, false))
                    .or(point);
                surface_marker(id, surface, None, point, document.tolerance())
            }
            Geometry::Mesh(mesh) => mesh_marker(id, mesh, face_index),
            _ => None,
        };
        Some((id, marker))
    }

    pub(super) fn paint_direction(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        overlay: Overlay<'_>,
        hover: Option<&Marker>,
    ) {
        for marker in overlay
            .markers
            .iter()
            .filter(|m| hover.is_none_or(|h| h.object != m.object))
            .chain(hover)
        {
            let Some(origin) = self
                .project(marker.point, rect)
                .filter(|p| rect.contains(*p))
            else {
                continue;
            };
            let length = 36. / f64::from(self.pixels_per_model_unit_at_origin(rect)).max(1e-12);
            for (index, axis) in marker.axes.iter().enumerate() {
                let Some(axis) = axis else {
                    continue;
                };
                let Some(end) = axis
                    .scaled(length)
                    .ok()
                    .and_then(|v| marker.point.translated(v).ok())
                    .and_then(|p| self.project(p, rect))
                else {
                    continue;
                };
                let color = if marker.surface {
                    [
                        Color32::from_rgb(210, 60, 55),
                        Color32::from_rgb(40, 145, 60),
                        Color32::from_rgb(130, 65, 210),
                    ][index]
                } else {
                    Color32::from_rgb(130, 65, 210)
                };
                let delta = end - origin;
                if delta.length() < 2. {
                    // Dot means toward the camera; cross means away.
                    painter.circle_stroke(origin, 4., Stroke::new(1.5, color));
                    if axis.dot(self.viewward_direction()).unwrap_or(0.) >= 0. {
                        painter.circle_filled(origin, 1.5, color);
                    } else {
                        for diagonal in [Vec2::new(2.5, 2.5), Vec2::new(2.5, -2.5)] {
                            painter.line_segment(
                                [origin - diagonal, origin + diagonal],
                                Stroke::new(1.5, color),
                            );
                        }
                    }
                } else {
                    painter.arrow(origin, delta, Stroke::new(2., color));
                    if marker.surface {
                        painter.text(
                            end + Vec2::new(4., -4.),
                            Align2::LEFT_BOTTOM,
                            ["U", "V", "N"][index],
                            FontId::proportional(12.),
                            color,
                        );
                    }
                }
            }
        }
    }
}
