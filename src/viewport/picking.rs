//! Screen capture and depth ordering for mesh and tessellated surface hits.

use super::screen::{point_in_triangle, point_segment_distance, signed_area};
use super::*;

#[derive(Clone, Copy)]
pub(super) struct PickHit {
    pub distance: f32,
    priority: u8,
    depth: Real,
}

impl PickHit {
    pub(super) fn screen(priority: u8, distance: f32) -> Self {
        Self {
            distance,
            priority,
            depth: Real::INFINITY,
        }
    }

    pub(super) fn is_better_than(self, other: Self) -> bool {
        self.distance < other.distance
            || (self.distance == other.distance
                && (self.priority < other.priority
                    || (self.priority == other.priority && self.depth < other.depth)))
    }

    pub(super) fn is_ambiguous_with(self, other: Self) -> bool {
        if self.priority != other.priority || (self.distance - other.distance).abs() > 0.75 {
            return false;
        }
        self.depth == other.depth
    }

    pub(super) fn rank(self, other: Self) -> std::cmp::Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| self.depth.total_cmp(&other.depth))
    }
}

/// Perspective projection interpolates reciprocal view depth in screen space.
/// Inputs are a clipped triangle and a cursor already accepted inside it.
fn triangle_depth(
    pointer: Pos2,
    screen: [Pos2; 3],
    depths: [Real; 3],
    perspective: bool,
) -> Option<Real> {
    let area = signed_area(screen[0], screen[1], screen[2]);
    if !area.is_finite() || area.abs() <= Real::EPSILON {
        return None;
    }
    if !pointer.is_finite()
        || depths
            .iter()
            .any(|d| !d.is_finite() || (perspective && *d <= 0.0))
    {
        return None;
    }
    if depths[0] == depths[1] && depths[1] == depths[2] {
        return Some(depths[0]);
    }
    let weights = [
        signed_area(screen[1], screen[2], pointer) / area,
        signed_area(screen[2], screen[0], pointer) / area,
        signed_area(screen[0], screen[1], pointer) / area,
    ];
    let depth = if perspective {
        // The inside predicate tolerates a small edge overshoot. Negative
        // weights are not meaningful depths and can reverse the reciprocal sum.
        let weights = weights.map(|w| w.max(0.0));
        let weight_sum = weights.into_iter().sum::<Real>();
        if !weight_sum.is_finite() || weight_sum <= 0.0 {
            return None;
        }
        // Scale by the nearest contributing vertex, not an unused vertex with
        // zero weight. Every ratio is then in [0, 1], without forming 1/depth.
        let minimum = weights
            .into_iter()
            .zip(depths)
            .filter(|(w, _)| *w > 0.0)
            .map(|(_, d)| d)
            .fold(Real::INFINITY, Real::min);
        let maximum = depths.into_iter().fold(Real::NEG_INFINITY, Real::max);
        let reciprocal = weights
            .into_iter()
            .zip(depths)
            .filter(|(w, _)| *w > 0.0)
            .map(|(w, d)| w * (minimum / d))
            .sum::<Real>()
            / weight_sum;
        (minimum / reciprocal).clamp(minimum, maximum)
    } else {
        let minimum = depths.into_iter().fold(Real::INFINITY, Real::min);
        let maximum = depths.into_iter().fold(Real::NEG_INFINITY, Real::max);
        let ordinary = weights
            .into_iter()
            .zip(depths)
            .map(|(w, d)| w * d)
            .sum::<Real>();
        if ordinary.is_finite() {
            // A covered triangle's depth is a convex combination. Screen-area
            // rounding (including the capture tolerance at edges) can overshoot.
            ordinary.clamp(minimum, maximum)
        } else {
            let scale = minimum.abs().max(maximum.abs());
            let weights = weights.map(|w| w.max(0.0));
            let sum = weights.into_iter().sum::<Real>();
            if !sum.is_finite() || sum <= 0.0 {
                return None;
            }
            let normalized = weights
                .into_iter()
                .zip(depths)
                .map(|(w, d)| w * (d / scale))
                .sum::<Real>()
                / sum;
            (normalized.clamp(minimum / scale, maximum / scale) * scale).clamp(minimum, maximum)
        }
    };
    (depth.is_finite() && (!perspective || depth > 0.0)).then_some(depth)
}

impl Viewport {
    pub(super) fn has_unmeshed_selected_face_source(
        &self,
        document: &Document,
        mode: FacePickMode,
    ) -> bool {
        document.selected_objects().any(|object| {
            if !selection_candidate(document, object, None)
                || !(matches!(object.geometry(), Geometry::Brep(_))
                    || mode == FacePickMode::SurfaceAndBrep
                        && matches!(object.geometry(), Geometry::NurbsSurface(_)))
            {
                return false;
            }
            let display = self
                .display_cache
                .borrow_mut()
                .get(object, document.tolerance());
            display.mesh().is_none()
                || (matches!(object.geometry(), Geometry::Brep(_))
                    && display.brep_face_sources().is_none())
        })
    }

    pub(super) fn pick_selected_face(
        &self,
        pointer: Pos2,
        rect: Rect,
        document: &Document,
        mode: FacePickMode,
    ) -> Option<(ObjectId, usize)> {
        let mut nearest: Option<(PickHit, ObjectId, usize)> = None;
        for object in document.selected_objects() {
            if !selection_candidate(document, object, None) {
                continue;
            }
            let display = if matches!(
                mode,
                FacePickMode::MeshAndBrep | FacePickMode::SurfaceAndBrep
            ) && matches!(object.geometry(), Geometry::Brep(_))
                || mode == FacePickMode::SurfaceAndBrep
                    && matches!(object.geometry(), Geometry::NurbsSurface(_))
            {
                Some(
                    self.display_cache
                        .borrow_mut()
                        .get(object, document.tolerance()),
                )
            } else {
                None
            };
            let (mesh, sources, single_surface) = match object.geometry() {
                Geometry::Mesh(mesh) if mode != FacePickMode::SurfaceAndBrep => (mesh, None, false),
                Geometry::Brep(_) if mode != FacePickMode::Mesh => {
                    let Some(display) = display.as_ref() else {
                        continue;
                    };
                    let (Some(mesh), Some(sources)) = (display.mesh(), display.brep_face_sources())
                    else {
                        continue;
                    };
                    (mesh, Some(sources), false)
                }
                Geometry::NurbsSurface(_) if mode == FacePickMode::SurfaceAndBrep => {
                    let Some(mesh) = display.as_ref().and_then(|display| display.mesh()) else {
                        continue;
                    };
                    (mesh, None, true)
                }
                _ => continue,
            };
            let Some((hit, mesh_face)) = self.mesh_face_pick(pointer, rect, mesh) else {
                continue;
            };
            if hit.distance > PICK_CAPTURE_PIXELS {
                continue;
            }
            let face = if let Some(sources) = sources {
                let Some(&face) = sources.get(mesh_face) else {
                    continue;
                };
                face
            } else if single_surface {
                0
            } else {
                mesh_face
            };
            if nearest.is_none_or(|(best, _, _)| hit.rank(best).is_lt()) {
                nearest = Some((hit, object.id(), face));
            }
        }
        nearest.map(|(_, object, face)| (object, face))
    }

    pub(super) fn mesh_pick(
        &self,
        pointer: Pos2,
        rect: Rect,
        mesh: &TriangleMesh,
        tolerance: Tolerance,
    ) -> PickHit {
        if self.display_mode == DisplayMode::Wireframe {
            let distance = mesh
                .visible_wireframe_lines(tolerance)
                .map(|lines| {
                    lines
                        .into_iter()
                        .filter_map(|line| self.project_segment(line.start(), line.end(), rect))
                        .map(|[start, end]| point_segment_distance(pointer, start, end))
                        .fold(f32::INFINITY, f32::min)
                })
                .unwrap_or(f32::INFINITY);
            return PickHit::screen(2, distance);
        }
        self.mesh_face_pick(pointer, rect, mesh)
            .map_or(PickHit::screen(2, f32::INFINITY), |(hit, _)| hit)
    }

    /// Return the closest visible polygon face under a screen point. The face
    /// index refers to stored triangles/quads, rather than tessellation triangles.
    pub(super) fn mesh_face_pick(
        &self,
        pointer: Pos2,
        rect: Rect,
        mesh: &TriangleMesh,
    ) -> Option<(PickHit, usize)> {
        let mut nearest = None;
        let mut triangle_index = 0;
        for (face_index, face) in mesh.faces().iter().enumerate() {
            for _ in 0..if face.is_triangle() { 1 } else { 2 } {
                let Some(points) = mesh.triangle_points(triangle_index) else {
                    triangle_index += 1;
                    continue;
                };
                triangle_index += 1;
                for points in self.clip_triangle(points).into_iter().flatten() {
                    let [Some(first), Some(second), Some(third)] =
                        points.map(|point| self.project(point, rect))
                    else {
                        continue;
                    };
                    let hit = if point_in_triangle(pointer, first, second, third) {
                        let Some(depth) = triangle_depth(
                            pointer,
                            [first, second, third],
                            points.map(|p| self.view_depth(p)),
                            !self.kind.is_parallel(),
                        ) else {
                            continue;
                        };
                        PickHit {
                            distance: 0.0,
                            priority: 2,
                            depth,
                        }
                    } else {
                        PickHit::screen(
                            2,
                            point_segment_distance(pointer, first, second)
                                .min(point_segment_distance(pointer, second, third))
                                .min(point_segment_distance(pointer, third, first)),
                        )
                    };
                    if nearest.is_none_or(|(best, _): (PickHit, usize)| hit.is_better_than(best)) {
                        nearest = Some((hit, face_index));
                    }
                }
            }
        }
        nearest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::SelectionMode;
    use viboceros_geometry::MeshFace;

    #[test]
    fn selected_surface_face_pick_uses_tessellation_and_excludes_mesh() {
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let point = |x, y, z| Point3::try_new(x, y, z).unwrap();
        let surface = NurbsSurface::try_bilinear([
            point(-1.0, -1.0, 1.0),
            point(1.0, -1.0, 1.0),
            point(1.0, 1.0, 1.0),
            point(-1.0, 1.0, 1.0),
        ])
        .unwrap();
        let surface_id = document
            .add_geometry(Geometry::NurbsSurface(surface))
            .unwrap();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                point(-1.0, -1.0, 2.0),
                point(1.0, -1.0, 2.0),
                point(0.0, 1.0, 2.0),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            document.tolerance(),
        )
        .unwrap();
        let mesh_id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([surface_id, mesh_id], SelectionMode::Replace)
            .unwrap();
        let pointer = view.project(point(0.0, 0.0, 0.0), rect).unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::SurfaceAndBrep),
            Some((surface_id, 0))
        );
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::Mesh),
            Some((mesh_id, 0))
        );
    }

    #[test]
    fn selected_mesh_face_pick_uses_visible_depth_and_stored_quad_index() {
        let view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let tolerance = document.tolerance();
        let triangle = |z| {
            TriangleMesh::try_new_faces(
                vec![
                    Point3::try_new(0.0, 0.0, z).unwrap(),
                    Point3::try_new(1.0, 0.0, z).unwrap(),
                    Point3::try_new(0.0, 1.0, z).unwrap(),
                ],
                vec![MeshFace::Triangle([0, 1, 2])],
                tolerance,
            )
            .unwrap()
        };
        let back = document
            .add_geometry(Geometry::Mesh(triangle(0.0)))
            .unwrap();
        let front = document
            .add_geometry(Geometry::Mesh(triangle(2.0)))
            .unwrap();
        document
            .select_objects_direct([back, front], SelectionMode::Replace)
            .unwrap();
        let pointer = view
            .project(Point3::try_new(0.2, 0.2, 0.0).unwrap(), rect)
            .unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::Mesh),
            Some((front, 0))
        );

        document
            .select_objects_direct([back], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::Mesh),
            Some((back, 0))
        );
        let quad = TriangleMesh::try_new_faces(
            vec![
                Point3::try_new(4.0, 0.0, 0.0).unwrap(),
                Point3::try_new(5.0, 0.0, 0.0).unwrap(),
                Point3::try_new(4.0, 1.0, 0.0).unwrap(),
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 1.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            ],
            vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Quad([3, 4, 5, 6])],
            document.tolerance(),
        )
        .unwrap();
        let quad_id = document.add_geometry(Geometry::Mesh(quad)).unwrap();
        document
            .select_objects_direct([quad_id], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::Mesh),
            Some((quad_id, 1))
        );
    }

    #[test]
    fn selected_brep_face_pick_maps_display_triangles_to_the_visible_source_face() {
        let mut view = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let frame = Frame3::try_from_normal(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 1.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let box_brep = Brep::try_box(
            frame,
            [[-2.0, 2.0], [-2.0, 2.0], [0.0, 3.0]],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let top_face = box_brep
            .faces()
            .iter()
            .enumerate()
            .max_by(|(_, first), (_, second)| {
                let midpoint = |face: &viboceros_geometry::BrepFace| {
                    let u = face.surface().domain_u();
                    let v = face.surface().domain_v();
                    face.surface()
                        .evaluate((u.start() + u.end()) / 2.0, (v.start() + v.end()) / 2.0)
                        .unwrap()
                        .z()
                };
                midpoint(first).total_cmp(&midpoint(second))
            })
            .unwrap()
            .0;
        let brep = document.add_geometry(Geometry::Brep(box_brep)).unwrap();
        let pointer = view
            .project(Point3::try_new(0.25, 0.25, 3.0).unwrap(), rect)
            .unwrap();
        document
            .select_objects_direct([brep], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::MeshAndBrep),
            Some((brep, top_face))
        );
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::SurfaceAndBrep),
            Some((brep, top_face))
        );
        let context = egui::Context::default();
        let mut frame = |events| {
            let mut output = ViewportOutput::default();
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        output = view.show(
                            ui,
                            &document,
                            ViewportInput {
                                face_pick: Some(FacePickMode::MeshAndBrep),
                                object_filter: None,
                                drafting: DraftingInput {
                                    active: true,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                )
                .drop_without_applying_deltas();
            output
        };
        let event = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![]);
        frame(vec![
            egui::Event::PointerMoved(pointer),
            event(pointer, true),
        ]);
        let output = frame(vec![event(pointer, false)]);
        assert_eq!(output.face_click, Some((brep, top_face)));
        assert!(output.picked_point.is_none());
        let empty = rect.center() + Vec2::new(155.0, -135.0);
        frame(vec![egui::Event::PointerMoved(empty), event(empty, true)]);
        let empty_output = frame(vec![event(empty, false)]);
        assert!(empty_output.face_click.is_none());
        assert!(empty_output.picked_point.is_none());
        drop(frame);

        let front = TriangleMesh::try_new_faces(
            vec![
                Point3::try_new(0.0, 0.0, 4.0).unwrap(),
                Point3::try_new(1.0, 0.0, 4.0).unwrap(),
                Point3::try_new(0.0, 1.0, 4.0).unwrap(),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            document.tolerance(),
        )
        .unwrap();
        let mesh = document.add_geometry(Geometry::Mesh(front)).unwrap();
        document
            .select_objects_direct([brep, mesh], SelectionMode::Replace)
            .unwrap();
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::MeshAndBrep),
            Some((mesh, 0))
        );
        assert_eq!(
            view.pick_selected_face(pointer, rect, &document, FacePickMode::SurfaceAndBrep),
            Some((brep, top_face))
        );
    }

    #[test]
    fn viewport_click_emits_face_hit_instead_of_a_drafting_point() {
        let mut viewport = Viewport::new(ViewKind::Top);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let mut document = Document::default();
        let mesh = TriangleMesh::try_new_faces(
            vec![
                Point3::try_new(0.0, 0.0, 0.0).unwrap(),
                Point3::try_new(1.0, 0.0, 0.0).unwrap(),
                Point3::try_new(0.0, 1.0, 0.0).unwrap(),
            ],
            vec![MeshFace::Triangle([0, 1, 2])],
            document.tolerance(),
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document
            .select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let pointer = viewport
            .project(Point3::try_new(0.2, 0.2, 0.0).unwrap(), rect)
            .unwrap();
        let context = egui::Context::default();
        let mut frame = |events| {
            let mut output = ViewportOutput::default();
            context
                .run_ui(
                    egui::RawInput {
                        screen_rect: Some(rect),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        output = viewport.show(
                            ui,
                            &document,
                            ViewportInput {
                                face_pick: Some(FacePickMode::Mesh),
                                object_filter: None,
                                drafting: DraftingInput {
                                    active: true,
                                    ..Default::default()
                                },
                                ..Default::default()
                            },
                            &[],
                            0,
                            true,
                        );
                    },
                )
                .drop_without_applying_deltas();
            output
        };
        let event = |pressed| egui::Event::PointerButton {
            pos: pointer,
            pressed,
            button: PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![]);
        frame(vec![egui::Event::PointerMoved(pointer), event(true)]);
        let output = frame(vec![event(false)]);
        assert_eq!(output.face_click, Some((id, 0)));
        assert!(output.picked_point.is_none());
        assert!(output.selection_click.is_none());
    }

    #[test]
    fn perspective_depth_is_bounded_at_captured_edges() {
        let screen = [Pos2::ZERO, Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0)];
        let pointer = Pos2::new(-5e-13, 0.0);
        assert!(point_in_triangle(pointer, screen[0], screen[1], screen[2]));
        assert_eq!(
            triangle_depth(pointer, screen, [1e14, 1.0, 1e14], true),
            Some(1e14)
        );
    }

    #[test]
    fn perspective_depth_handles_extreme_positive_vertex_ranges() {
        let screen = [Pos2::ZERO, Pos2::new(8.0, 0.0), Pos2::new(0.0, 8.0)];
        for depths in [
            [Real::from_bits(1), Real::from_bits(2), Real::from_bits(4)],
            [Real::MAX, Real::MAX.next_down(), Real::MAX / 2.0],
            [Real::from_bits(1), 1.0, Real::MAX],
        ] {
            for (vertex, expected) in screen.into_iter().zip(depths) {
                assert_eq!(triangle_depth(vertex, screen, depths, true), Some(expected));
            }
            let minimum = depths.into_iter().fold(Real::INFINITY, Real::min);
            let maximum = depths.into_iter().fold(Real::NEG_INFINITY, Real::max);
            for x in 0..=8 {
                for y in 0..=8 - x {
                    let actual =
                        triangle_depth(Pos2::new(x as f32, y as f32), screen, depths, true);
                    assert!(
                        actual.is_some_and(|d| (minimum..=maximum).contains(&d)),
                        "depths={depths:?}, ({x},{y}), actual={actual:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn perspective_depth_matches_scaled_analytic_harmonic_mean() {
        let screen = [Pos2::ZERO, Pos2::new(8.0, 0.0), Pos2::new(0.0, 8.0)];
        // At (2, 2), barycentric weights are 1/2, 1/4, 1/4, so
        // 1 / (1/(2s) + 1/(8s) + 1/(16s)) = 16s/11.
        for exponent in [-1000, -500, 0, 500, 1000] {
            let scale = 2.0_f64.powi(exponent);
            let actual = triangle_depth(
                Pos2::new(2.0, 2.0),
                screen,
                [scale, 2.0 * scale, 4.0 * scale],
                true,
            )
            .unwrap();
            let expected = scale * (16.0 / 11.0);
            assert!((actual / expected - 1.0).abs() <= 2.0 * Real::EPSILON);
        }
    }

    #[test]
    fn captured_edge_depth_survives_an_overflowing_weighted_product() {
        let screen = [Pos2::ZERO, Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0)];
        let pointer = Pos2::new(-5e-13, 0.0);
        assert!(point_in_triangle(pointer, screen[0], screen[1], screen[2]));
        let first_weight = signed_area(screen[1], screen[2], pointer);
        assert!((first_weight * Real::MAX).is_infinite());
        for sign in [-1.0, 1.0] {
            let depths = [
                Real::MAX,
                Real::MAX.next_down(),
                Real::MAX.next_down().next_down(),
            ]
            .map(|d| sign * d);
            assert_eq!(
                triangle_depth(pointer, screen, depths, false),
                Some(sign * Real::MAX)
            );
        }
        assert!(triangle_depth(Pos2::new(f32::NAN, 0.0), screen, [1.0; 3], false).is_none());
        assert!(triangle_depth(Pos2::ZERO, screen, [Real::NAN; 3], false).is_none());
    }

    #[test]
    fn overlapping_constant_depth_faces_choose_the_nearer_f64_plane() {
        let mut viewport = Viewport::new(ViewKind::Top);
        viewport.display_mode = DisplayMode::Shaded;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let pointer = viewport
            .project(Point3::try_new(1.0, 1.0, 0.0).unwrap(), rect)
            .unwrap();
        for order in [[0, 1], [1, 0]] {
            let mut document = Document::default();
            let ids = order.map(|index| {
                let (depth, size) = if index == 0 {
                    (Real::MAX, 3.0)
                } else {
                    (Real::MAX.next_down(), 17.0)
                };
                let vertices = [(0.0, 0.0), (size, 0.0), (0.0, size)]
                    .map(|(x, y)| Point3::try_new(x, y, -depth).unwrap());
                document
                    .add_geometry(Geometry::Mesh(
                        TriangleMesh::try_new(
                            vertices.to_vec(),
                            vec![[0, 1, 2]],
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                    ))
                    .unwrap()
            });
            assert_eq!(
                viewport.pick_object(pointer, rect, &document),
                Some(ids[usize::from(order[0] == 0)])
            );
            assert_eq!(
                viewport.pick_object_candidates_matching_preview(
                    pointer,
                    rect,
                    &document,
                    ObjectSelectionFilter::Any,
                    None,
                ),
                [ids[usize::from(order[0] == 0)]]
            );
        }
    }

    #[test]
    fn constant_parallel_face_depth_is_exact_throughout_the_triangle() {
        for size in 3..=24 {
            let screen = [
                Pos2::ZERO,
                Pos2::new(size as f32, 0.0),
                Pos2::new(0.0, size as f32),
            ];
            for x in 0..=size {
                for y in 0..=size - x {
                    for depth in [Real::MAX, -Real::MAX, 1e200, Real::from_bits(1)] {
                        assert_eq!(
                            triangle_depth(
                                Pos2::new(x as f32, y as f32),
                                screen,
                                [depth; 3],
                                false
                            ),
                            Some(depth),
                            "size={size}, ({x},{y}), depth={depth}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn varying_parallel_face_depth_stays_inside_finite_vertex_bounds() {
        for size in 3..=24 {
            let screen = [
                Pos2::ZERO,
                Pos2::new(size as f32, 0.0),
                Pos2::new(0.0, size as f32),
            ];
            for sign in [-1.0, 1.0] {
                let depths = [
                    Real::MAX,
                    Real::MAX.next_down(),
                    Real::MAX.next_down().next_down(),
                ]
                .map(|d| d * sign);
                let minimum = depths.into_iter().fold(Real::INFINITY, Real::min);
                let maximum = depths.into_iter().fold(Real::NEG_INFINITY, Real::max);
                for x in 0..=size {
                    for y in 0..=size - x {
                        let actual =
                            triangle_depth(Pos2::new(x as f32, y as f32), screen, depths, false);
                        assert!(
                            actual.is_some_and(|d| (minimum..=maximum).contains(&d)),
                            "size={size}, ({x},{y}), sign={sign}, depth={actual:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mesh_face_hit_uses_nearest_triangle_even_when_it_is_last() {
        let mut viewport = Viewport::new(ViewKind::Top);
        viewport.display_mode = DisplayMode::Shaded;
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(256.0));
        let vertices = [1.0, 5.0]
            .into_iter()
            .flat_map(|z| {
                [(-1.0, -1.0), (1.0, -1.0), (0.0, 1.0)]
                    .map(|(x, y)| Point3::try_new(x, y, z).unwrap())
            })
            .collect::<Vec<_>>();
        for triangles in [vec![[0, 1, 2], [3, 4, 5]], vec![[3, 4, 5], [0, 1, 2]]] {
            let mesh =
                TriangleMesh::try_new(vertices.clone(), triangles, Tolerance::DEFAULT).unwrap();
            let hit = viewport.mesh_pick(rect.center(), rect, &mesh, Tolerance::DEFAULT);
            assert_eq!(hit.distance, 0.0);
            assert_eq!(hit.depth, -5.0);
        }
    }

    #[test]
    fn face_depth_interpolation_is_perspective_correct_and_winding_independent() {
        let screen = [
            Pos2::new(0.0, 0.0),
            Pos2::new(2.0, 0.0),
            Pos2::new(0.0, 2.0),
        ];
        let depths = [2.0, 4.0, 8.0];
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let screen = order.map(|i| screen[i]);
            let depths = order.map(|i| depths[i]);
            assert_eq!(
                triangle_depth(Pos2::new(0.5, 0.5), screen, depths, false),
                Some(4.0)
            );
            assert!(
                (triangle_depth(Pos2::new(0.5, 0.5), screen, depths, true).unwrap() - 32.0 / 11.0)
                    .abs()
                    < 1e-14
            );
        }
        assert!(triangle_depth(Pos2::ZERO, [Pos2::ZERO; 3], depths, true).is_none());
        assert!(triangle_depth(Pos2::ZERO, screen, [-1.0, 4.0, 8.0], true).is_none());
    }
}
