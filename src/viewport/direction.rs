//! Direction arrows use staged geometry while picks retain document identities.
use super::*;
use std::collections::BTreeSet;
use viboceros_command::direction::{DirectionSession, marker_at};
pub(crate) use viboceros_command::direction::{Marker, collect_markers};
mod subd;
#[cfg(test)]
mod subd_tests;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Overlay<'a> {
    pub edits: &'a DirectionSession,
    pub ids: &'a BTreeSet<ObjectId>,
    pub markers: &'a [Marker],
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
                brep.faces().get(face_index)?;
                let point = self
                    .normal_surface_point(pointer, rect, document, (id, Some(face_index), false))
                    .or(point);
                marker_at(id, geometry, face_index, point, document.tolerance())
            }
            Geometry::NurbsSurface(_) => {
                let point = self
                    .normal_surface_point(pointer, rect, document, (id, None, false))
                    .or(point);
                marker_at(id, geometry, face_index, point, document.tolerance())
            }
            Geometry::Mesh(_) => marker_at(id, geometry, face_index, point, document.tolerance()),
            Geometry::SubD(staged) => {
                self.subd_direction_marker(pointer, rect, document, id, face_index, staged)
            }
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
        self.paint_direction_markers(
            painter,
            rect,
            overlay
                .markers
                .iter()
                .filter(|m| hover.is_none_or(|h| h.object != m.object))
                .chain(hover),
            &viboceros_command::direction::analysis::Options::default(),
        );
    }

    pub(super) fn paint_direction_analysis(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        view: &viboceros_command::direction::analysis::View,
        excluded: Option<&BTreeSet<ObjectId>>,
    ) {
        self.paint_direction_markers(
            painter,
            rect,
            view.markers
                .iter()
                .filter(|m| excluded.is_none_or(|ids| !ids.contains(&m.object))),
            &view.options,
        );
    }

    fn paint_direction_markers<'a>(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        markers: impl Iterator<Item = &'a Marker>,
        options: &viboceros_command::direction::analysis::Options,
    ) {
        if !options.direction && !options.u && !options.v {
            return;
        }
        for marker in markers {
            let Some(origin) = self
                .project(marker.point, rect)
                .filter(|p| rect.contains(*p))
            else {
                continue;
            };
            let length = 36. / f64::from(self.pixels_per_model_unit_at_origin(rect)).max(1e-12);
            for (index, axis) in marker.axes.iter().enumerate() {
                let enabled = if marker.surface {
                    [options.u, options.v, options.direction][index]
                } else {
                    options.direction
                };
                if !enabled {
                    continue;
                }
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
                let rgb = if marker.surface && index < 2 {
                    [[210, 60, 55], [40, 145, 60]][index]
                } else {
                    options.color
                };
                let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
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
