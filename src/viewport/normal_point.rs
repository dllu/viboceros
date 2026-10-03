//! A chosen normal reference constrains location picks without changing selection.
use super::*;

impl Viewport {
    pub(crate) fn normal_surface_point(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        target: (ObjectId, Option<usize>, bool),
    ) -> Option<Point3> {
        let geometry = document.object(target.0)?.geometry();
        // A projected surface mesh gives a local seed; the location is resolved
        // against the original surface, never accepted from tessellation alone.
        let display = self
            .display_cache
            .borrow_mut()
            .get(document.object(target.0)?, document.tolerance());
        let hint = display
            .mesh()
            .and_then(|mesh| self.mesh_face_pick_with_point(pointer, rect, mesh))
            .and_then(|(_, _, point)| point)
            .or_else(|| self.unproject_drafting_plane(pointer, rect, None))?;
        let tolerance = document.tolerance();
        let (surface, face) = match geometry {
            Geometry::NurbsSurface(surface) => (surface, 0),
            Geometry::Brep(brep) => {
                let face = match target.1 {
                    Some(face) => face,
                    None if brep.faces().len() == 1 => 0,
                    None => brep.closest_face_parameters(hint, tolerance).ok()??.0,
                };
                (brep.faces().get(face)?.surface(), face)
            }
            _ => return None,
        };
        let (origin, direction) = self.drafting_view_line_relative_to(
            surface.control_points().first()?.point(),
            pointer,
            rect,
        )?;
        let (mut u, mut v) = surface.closest_parameters(hint, tolerance).ok()?;
        let anchor = surface.control_points().first()?.point();
        let view = NaVector3::from(direction.normalized_nonzero().ok()?.as_vector().to_array());
        let origin = NaVector3::from(origin.to_array());
        for _ in 0..20 {
            let (point, du, dv) = surface.evaluate_with_derivatives(u, v).ok()?;
            let delta = NaVector3::from(anchor.vector_to(point).ok()?.to_array()) - origin;
            let residual = delta - view * delta.dot(&view);
            let du = NaVector3::from(du.to_array());
            let dv = NaVector3::from(dv.to_array());
            let du = du - view * du.dot(&view);
            let dv = dv - view * dv.dot(&view);
            let matrix = nalgebra::Matrix2::new(du.dot(&du), du.dot(&dv), du.dot(&dv), dv.dot(&dv));
            let Some(step) = matrix.lu().solve(&nalgebra::Vector2::new(
                -du.dot(&residual),
                -dv.dot(&residual),
            )) else {
                break;
            };
            if !step.iter().all(|value| value.is_finite()) {
                break;
            }
            // Keep the local projected solve descending. A singular view or
            // a clamped boundary must not jump to an unrelated surface root.
            let error = residual.norm_squared();
            let mut scale = 1.0;
            let mut accepted = None;
            for _ in 0..16 {
                let next_u = (u + scale * step.x)
                    .clamp(*surface.domain_u().start(), *surface.domain_u().end());
                let next_v = (v + scale * step.y)
                    .clamp(*surface.domain_v().start(), *surface.domain_v().end());
                if next_u == u && next_v == v {
                    break;
                }
                let candidate = surface.evaluate(next_u, next_v).ok()?;
                let delta = NaVector3::from(anchor.vector_to(candidate).ok()?.to_array()) - origin;
                let residual = delta - view * delta.dot(&view);
                if residual.norm_squared() < error {
                    accepted = Some((next_u, next_v));
                    break;
                }
                scale *= 0.5;
            }
            let Some(next) = accepted else {
                break;
            };
            (u, v) = next;
        }
        let point = surface.evaluate(u, v).ok()?;
        if let Geometry::Brep(brep) = geometry
            && !target.2
        {
            let (u, v) = brep
                .closest_parameters_on_face(face, point, tolerance)
                .ok()??;
            surface.evaluate(u, v).ok()
        } else {
            Some(point)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_command::ObjectSelectionFilter;
    use viboceros_geometry::{Circle3, NurbsSurface};

    fn frame(
        context: &egui::Context,
        view: &mut Viewport,
        document: &Document,
        input: ViewportInput<'_>,
        events: Vec<egui::Event>,
    ) -> ViewportOutput {
        let mut output = ViewportOutput::default();
        context
            .run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                    events,
                    ..Default::default()
                },
                |ui| output = view.show(ui, document, input, &[], 0, true),
            )
            .drop_without_applying_deltas();
        output
    }

    fn click(pointer: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pointer),
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn normal_reference_clicks_offer_curves_and_faces_without_changing_selection() {
        for surface in [false, true] {
            let mut document = Document::default();
            let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
            let source = document
                .add_geometry(Geometry::Point(p(-10., -10., 0.)))
                .unwrap();
            let geometry = if surface {
                Geometry::NurbsSurface(
                    NurbsSurface::try_bilinear([
                        p(0., 0., 0.),
                        p(6., 0., 0.),
                        p(6., 6., 4.),
                        p(0., 6., 0.),
                    ])
                    .unwrap(),
                )
            } else {
                Geometry::Circle(
                    Circle3::try_new(
                        p(0., 0., 0.),
                        3.,
                        WorldPlane::Top.frame().z_axis(),
                        document.tolerance(),
                    )
                    .unwrap(),
                )
            };
            let target = document.add_geometry(geometry).unwrap();
            document
                .select_objects_direct([source], SelectionMode::Replace)
                .unwrap();
            let context = egui::Context::default();
            let mut view = Viewport::new(ViewKind::Top);
            view.display_mode = DisplayMode::Shaded;
            let input = ViewportInput {
                object_filter: Some(ObjectSelectionFilter::Parametric),
                face_pick: Some(FacePickMode::SurfaceAndBrepAny),
                ..Default::default()
            };
            frame(&context, &mut view, &document, input, vec![]);
            let aim = if surface {
                p(3., 3., 1.)
            } else {
                p(3., 0., 0.)
            };
            let pointer = view.project(aim, view.last_rect.unwrap()).unwrap();
            let output = frame(&context, &mut view, &document, input, click(pointer));
            if surface {
                assert_eq!(output.face_click, Some((target, 0)));
            } else {
                assert_eq!(output.selection_click.unwrap().object_id, Some(target));
            }
            assert!(document.is_selected(source));
            assert!(!document.is_selected(target));
            assert!(output.picked_point.is_none());
        }
    }

    #[test]
    fn normal_base_clicks_return_a_curve_parameter_or_polished_surface_location() {
        for surface in [false, true] {
            let mut document = Document::default();
            let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
            let geometry = if surface {
                Geometry::NurbsSurface(
                    NurbsSurface::try_bilinear([
                        p(0., 0., 0.),
                        p(6., 0., 0.),
                        p(6., 6., 4.),
                        p(0., 6., 0.),
                    ])
                    .unwrap(),
                )
            } else {
                Geometry::Circle(
                    Circle3::try_new(
                        p(0., 0., 0.),
                        3.,
                        WorldPlane::Top.frame().z_axis(),
                        document.tolerance(),
                    )
                    .unwrap(),
                )
            };
            let curve = geometry.converted_to_nurbs_curve().unwrap();
            let target = document.add_geometry(geometry).unwrap();
            let context = egui::Context::default();
            let mut view = Viewport::new(ViewKind::Top);
            let input = ViewportInput {
                drafting: DraftingInput {
                    active: true,
                    ..Default::default()
                },
                edge_curve: curve.as_ref(),
                normal_surface: surface.then_some((target, None, false)),
                ..Default::default()
            };
            frame(&context, &mut view, &document, input, vec![]);
            let aim = if surface {
                p(2.25, 3.5, 0.875)
            } else {
                p(3., 0., 0.)
            };
            let pointer = view.project(aim, view.last_rect.unwrap()).unwrap();
            let output = frame(&context, &mut view, &document, input, click(pointer));
            let location = if surface {
                output.picked_point.unwrap()
            } else {
                curve
                    .unwrap()
                    .evaluate(output.edge_parameter.unwrap())
                    .unwrap()
            };
            assert!(
                location.distance_to(aim).unwrap() < 1e-5,
                "{location:?} {aim:?}"
            );
            assert!(output.face_click.is_none());
            assert!(!document.is_selected(target));
        }
    }

    #[test]
    fn whole_polysurface_base_uses_the_visible_face_instead_of_face_zero() {
        let mut document = Document::default();
        let target = document
            .add_geometry(Geometry::Brep(
                viboceros_geometry::Brep::try_box(
                    WorldPlane::Top.frame(),
                    [[0., 6.], [0., 6.], [0., 4.]],
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.));
        let expected = Point3::try_new(3., 3., 4.).unwrap();
        let pointer = view.project(expected, rect).unwrap();
        let location = view
            .normal_surface_point(pointer, rect, &document, (target, None, false))
            .unwrap();
        assert!(location.distance_to(expected).unwrap() < 1e-5);
    }
}
