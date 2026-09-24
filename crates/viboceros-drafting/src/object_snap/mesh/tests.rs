use super::super::{ObjectSnapCache, ObjectSnapOptions};
use super::*;
use viboceros_document::Document;
use viboceros_geometry::{MeshFace, Tolerance, TriangleMesh};

fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 7.).unwrap()
}
fn quad(triangulated: bool) -> Geometry {
    Geometry::Mesh(
        TriangleMesh::try_new_faces(
            vec![p(2., -2.), p(8., -2.), p(8., -8.), p(2., -8.)],
            if triangulated {
                vec![MeshFace::Triangle([0, 1, 2]), MeshFace::Triangle([0, 2, 3])]
            } else {
                vec![MeshFace::Quad([0, 1, 2, 3])]
            },
            Tolerance::DEFAULT,
        )
        .unwrap(),
    )
}
fn query(
    cache: &mut ObjectSnapCache,
    doc: &Document,
    cursor: [Real; 2],
    modes: ObjectSnapModes,
    enabled: bool,
) -> Option<super::super::ObjectSnap> {
    cache
        .nearest_projected_with_options(
            doc,
            cursor,
            0.2,
            |p| Some([p.x(), p.y()]),
            ObjectSnapOptions {
                modes,
                mesh_edges: enabled,
            },
        )
        .unwrap()
}

#[test]
fn mesh_policy_modes_priority_and_real_face_boundaries_are_independent() {
    let mut doc = Document::default();
    doc.add_geometry(quad(false)).unwrap();
    let mut cache = ObjectSnapCache::default();
    let near = ObjectSnapModes::only(ObjectSnapKind::Near);
    let mid = ObjectSnapModes::only(ObjectSnapKind::Mid);
    assert!(query(&mut cache, &doc, [3.5, -2.], near, false).is_none());
    assert!(query(&mut cache, &doc, [3.5, -2.], ObjectSnapModes::NONE, true).is_none());
    assert_eq!(cache.meshes.builds, 0);
    assert_eq!(
        query(&mut cache, &doc, [3.5, -2.], near, true)
            .unwrap()
            .point(),
        p(3.5, -2.)
    );
    assert!(query(&mut cache, &doc, [3.5, -2.], mid, true).is_none());
    assert_eq!(
        query(&mut cache, &doc, [5., -2.], mid, true)
            .unwrap()
            .point(),
        p(5., -2.)
    );
    assert_eq!(
        query(
            &mut cache,
            &doc,
            [3.5, -2.],
            mid.with(ObjectSnapKind::Near, true),
            true
        )
        .unwrap()
        .kind(),
        ObjectSnapKind::Near
    );
    assert_eq!(
        query(
            &mut cache,
            &doc,
            [4.95, -2.],
            mid.with(ObjectSnapKind::Near, true),
            true
        )
        .unwrap()
        .point(),
        p(5., -2.)
    );
    assert!(query(&mut cache, &doc, [4., -4.], near, true).is_none());
    assert_eq!(cache.meshes.builds, 1);
    assert!(
        query(
            &mut cache,
            &doc,
            [2., -2.],
            ObjectSnapModes::only(ObjectSnapKind::End),
            true
        )
        .is_none()
    );
    let mut triangles = Document::default();
    triangles.add_geometry(quad(true)).unwrap();
    assert_eq!(
        query(&mut cache, &triangles, [4., -4.], near, true)
            .unwrap()
            .point(),
        p(4., -4.)
    );
}

#[test]
fn vertex_snap_is_independent_of_mesh_wire_switch_and_other_landmarks() {
    let mut doc = Document::default();
    let id = doc.add_geometry(quad(false)).unwrap();
    let mut cache = ObjectSnapCache::default();
    let vertex = ObjectSnapModes::only(ObjectSnapKind::Vertex);
    let hit = query(&mut cache, &doc, [2.05, -2.04], vertex, false).unwrap();
    assert_eq!(hit.object_id(), id);
    assert_eq!(hit.kind(), ObjectSnapKind::Vertex);
    assert_eq!(hit.point(), p(2., -2.));
    assert_eq!(cache.meshes.builds, 0);
    assert_eq!(cache.meshes.vertex_builds, 1);
    let near = query(
        &mut cache,
        &doc,
        [2.05, -2.04],
        ObjectSnapModes::only(ObjectSnapKind::Near),
        true,
    )
    .unwrap();
    assert!(near.distance() < hit.distance());
    let mixed = query(
        &mut cache,
        &doc,
        [2.05, -2.04],
        vertex.with(ObjectSnapKind::Near, true),
        true,
    )
    .unwrap();
    assert_eq!(mixed.kind(), ObjectSnapKind::Vertex);
    assert_eq!(mixed.point(), p(2., -2.));
    assert_eq!(
        query(&mut cache, &doc, [2.05, -2.04], vertex, true)
            .unwrap()
            .point(),
        p(2., -2.)
    );
    assert_eq!(cache.meshes.vertex_builds, 1);
    for kind in [ObjectSnapKind::Point, ObjectSnapKind::End] {
        assert!(
            query(
                &mut cache,
                &doc,
                [2.05, -2.04],
                ObjectSnapModes::only(kind),
                true
            )
            .is_none()
        );
    }
    doc.set_objects_visibility([id], false).unwrap();
    assert!(query(&mut cache, &doc, [2.05, -2.04], vertex, false).is_none());
    doc.set_objects_visibility([id], true).unwrap();
    doc.replace_object_geometries([(id, quad(true))]).unwrap();
    assert!(query(&mut cache, &doc, [2.05, -2.04], vertex, false).is_some());
    assert_eq!(cache.meshes.vertex_builds, 2);
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, [2.05, -2.04], vertex, false).is_some());
    assert_eq!(cache.meshes.vertex_builds, 3);
    doc.delete_objects([id]).unwrap();
    assert!(query(&mut cache, &doc, [2.05, -2.04], vertex, false).is_none());
    assert!(cache.meshes.is_empty());
}

#[test]
fn vertex_index_prunes_large_meshes_without_building_wire_index() {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            let base = vertices.len() as u32;
            vertices.extend(
                [[2., -2.], [8., -2.], [8., -8.], [2., -8.]]
                    .map(|[a, b]| p(a + 10. * x as Real, b + 10. * y as Real)),
            );
            faces.push(MeshFace::Quad([base, base + 1, base + 2, base + 3]));
        }
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(
        TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap();
    let mut cache = ObjectSnapCache::default();
    for i in 0..100 {
        let x = ((i * 17) % 64) as Real * 10.;
        let y = ((i * 31) % 64) as Real * 10.;
        cache.meshes.visited_vertices = 0;
        let hit = query(
            &mut cache,
            &doc,
            [x + 2.05, y - 2.04],
            ObjectSnapModes::only(ObjectSnapKind::Vertex),
            false,
        )
        .unwrap();
        assert_eq!(hit.point(), p(x + 2., y - 2.));
        assert!(cache.meshes.visited_vertices < 64);
    }
    assert_eq!(cache.meshes.vertex_builds, 1);
    assert_eq!(cache.meshes.builds, 0);
}

#[test]
fn vertex_index_matches_exhaustive_perspective_capture() {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let base = vertices.len() as u32;
            let depth = if (x + y) % 7 == 0 {
                -1.
            } else {
                1. + (x % 5) as Real
            };
            for [dx, dy] in [[0., 0.], [0.3, 0.], [0., 0.3]] {
                vertices.push(Point3::try_new(x as Real + dx, y as Real + dy, depth).unwrap());
            }
            faces.push(MeshFace::Triangle([base, base + 1, base + 2]));
        }
    }
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(
        TriangleMesh::try_new_faces(vertices.clone(), faces, Tolerance::DEFAULT).unwrap(),
    ))
    .unwrap();
    let project = |p: Point3| (p.z() > 0.).then(|| [p.x() / p.z(), p.y() / p.z()]);
    let mut cache = ObjectSnapCache::default();
    for i in 0..100 {
        let cursor = [(i * 17 % 33) as Real * 0.4, (i * 29 % 31) as Real * 0.4];
        let expected = vertices
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(order, point)| {
                let image = project(point)?;
                let offset = [image[0] - cursor[0], image[1] - cursor[1]];
                (offset[0].abs().max(offset[1].abs()) <= 0.2).then_some((
                    offset[0].hypot(offset[1]),
                    order,
                    point,
                ))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, _, point)| point);
        let actual = cache
            .nearest_projected_with_options(
                &doc,
                cursor,
                0.2,
                project,
                ObjectSnapOptions {
                    modes: ObjectSnapModes::only(ObjectSnapKind::Vertex),
                    mesh_edges: false,
                },
            )
            .unwrap()
            .map(|snap| snap.point());
        assert_eq!(actual, expected, "cursor {cursor:?}");
    }
    assert_eq!(cache.meshes.vertex_builds, 1);
    assert_eq!(cache.meshes.builds, 0);
}

#[test]
fn mesh_wire_cache_tracks_snapshots_visibility_undo_conversion_and_deletion() {
    let mut doc = Document::default();
    let id = doc.add_geometry(quad(false)).unwrap();
    let mut cache = ObjectSnapCache::default();
    let modes = ObjectSnapModes::only(ObjectSnapKind::Near);
    for _ in 0..5 {
        assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_some());
    }
    assert_eq!(cache.meshes.builds, 1);
    doc.set_objects_locked([id], true).unwrap();
    assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_some());
    doc.set_objects_visibility([id], false).unwrap();
    assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_none());
    doc.set_objects_visibility([id], true).unwrap();
    doc.set_objects_locked([id], false).unwrap();
    doc.set_tolerance(Tolerance::try_new(0.01, 1e-12, 1e-10).unwrap());
    assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_some());
    assert_eq!(
        cache.meshes.builds, 1,
        "existing wires do not depend on creation tolerance"
    );
    doc.replace_object_geometries([(id, quad(true))]).unwrap();
    assert!(query(&mut cache, &doc, [4., -4.], modes, true).is_some());
    assert_eq!(cache.meshes.builds, 2);
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, [4., -4.], modes, true).is_none());
    assert_eq!(cache.meshes.builds, 3);
    doc.replace_object_geometries([(id, Geometry::Point(p(4., -4.)))])
        .unwrap();
    query(&mut cache, &doc, [4., -4.], modes, true);
    assert!(cache.meshes.is_empty());
    doc.undo().unwrap();
    assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_some());
    doc.delete_objects([id]).unwrap();
    assert!(query(&mut cache, &doc, [3.5, -2.], modes, true).is_none());
    assert!(cache.meshes.is_empty());
}

#[test]
fn hierarchy_matches_individual_wires_across_perspective_clipping_and_ties() {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for y in -4..4 {
        for x in -4..4 {
            let base = vertices.len() as u32;
            vertices.extend(
                [[0., 0.], [2., 0.], [2., 2.], [0., 2.]]
                    .map(|[a, b]| p(a + 3. * x as Real, b + 3. * y as Real)),
            );
            faces.push(MeshFace::Quad([base, base + 1, base + 2, base + 3]));
        }
    }
    let mesh = TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap();
    let mut wires = Document::default();
    // Document order reproduces canonical topology-edge tie order, without
    // the mesh hierarchy's partition or shared best-feature state.
    for [a, b] in mesh.topology_edge_points() {
        wires
            .add_geometry(Geometry::Line(
                viboceros_geometry::LineSegment::try_new(a, b, Tolerance::DEFAULT).unwrap(),
            ))
            .unwrap();
    }
    let mut meshes = Document::default();
    meshes.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let mut mesh_cache = ObjectSnapCache::default();
    let mut wire_cache = ObjectSnapCache::default();
    for offset in [-3., 0., 20.] {
        let project = |p: Point3| {
            let depth = 0.4 * p.x() + 0.6 * p.y() + offset;
            (depth > 0.25).then(|| [10. * p.x() / depth, 10. * p.y() / depth])
        };
        for kind in [ObjectSnapKind::Mid, ObjectSnapKind::Near] {
            for y in -4..=4 {
                for x in -4..=4 {
                    let cursor = [x as Real * 4., y as Real * 4.];
                    let actual = mesh_cache
                        .nearest_projected_with_options(
                            &meshes,
                            cursor,
                            0.75,
                            project,
                            ObjectSnapOptions {
                                modes: ObjectSnapModes::only(kind),
                                mesh_edges: true,
                            },
                        )
                        .unwrap();
                    let expected = if kind == ObjectSnapKind::Near {
                        let metric = super::super::ProjectedSnapMetric {
                            cursor,
                            capture_radius: 0.75,
                            project,
                        };
                        let mut best: Option<(Point3, Real)> = None;
                        // Exhaustive canonical wire traversal, without any
                        // hierarchy rejection or partition ordering. Mesh Near
                        // differs from curve Near; its independent arithmetic
                        // reference is tested in projected_line::tests.
                        for object in wires.objects() {
                            let Geometry::Line(wire) = object.geometry() else {
                                panic!()
                            };
                            line(wire.start(), wire.end(), &metric, &mut |point, distance| {
                                if best.is_none_or(|(_, d)| distance < d) {
                                    best = Some((point, distance));
                                }
                            });
                        }
                        best.map(|(point, _)| (point, kind))
                    } else {
                        wire_cache
                            .nearest_projected_with_modes(
                                &wires,
                                cursor,
                                0.75,
                                project,
                                // Point has no targets on lines; including it
                                // disables the curve-only whole-wire Mid hover.
                                ObjectSnapModes::only(kind).with(ObjectSnapKind::Point, true),
                            )
                            .unwrap()
                            .map(|s| (s.point(), s.kind()))
                    };
                    assert_eq!(
                        actual.map(|s| (s.point(), s.kind())),
                        expected,
                        "offset={offset} kind={kind:?} cursor={cursor:?}"
                    );
                }
            }
        }
    }
    assert_eq!(mesh_cache.meshes.builds, 1);
}

#[test]
fn wire_hierarchy_prunes_large_meshes_without_inventing_quad_diagonals() {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            let base = vertices.len() as u32;
            vertices.extend(
                [[2., -2.], [8., -2.], [8., -8.], [2., -8.]]
                    .map(|[a, b]| p(a + 10. * x as Real, b + 10. * y as Real)),
            );
            faces.push(MeshFace::Quad([base, base + 1, base + 2, base + 3]));
        }
    }
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Mesh(
            TriangleMesh::try_new_faces(vertices, faces, Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let mut cache = ObjectSnapCache::default();
    let modes = ObjectSnapModes::only(ObjectSnapKind::Near);
    for i in 0..100 {
        let x = ((i * 17) % 64) as Real * 10.;
        let y = ((i * 31) % 64) as Real * 10.;
        cache.meshes.visited = 0;
        let hit = query(&mut cache, &doc, [x + 3.5, y - 1.9], modes, true).unwrap();
        assert_eq!(hit.point(), p(x + 3.5, y - 2.));
        assert!(
            cache.meshes.visited < 64,
            "visited {} wires",
            cache.meshes.visited
        );
        assert!(query(&mut cache, &doc, [x + 5., y - 5.], modes, true).is_none());
    }
    assert_eq!(cache.meshes.builds, 1);
    assert_eq!(cache.meshes.entries[&id].wires.len(), 16384);
}

#[test]
fn existing_short_wires_remain_snappable_under_a_larger_document_tolerance() {
    let mesh = TriangleMesh::try_new_faces(
        vec![p(0., 0.), p(1e-10, 0.), p(1e-10, 1e-10), p(0., 1e-10)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::try_new(1e-15, 1e-12, 1e-10).unwrap(),
    )
    .unwrap();
    assert_eq!(mesh.topology_edge_points().len(), 4);
    let mut doc = Document::default();
    doc.add_geometry(Geometry::Mesh(mesh)).unwrap();
    let hit = ObjectSnapCache::default()
        .nearest_projected_with_options(
            &doc,
            [5e-11, 0.],
            1e-13,
            |p| Some([p.x(), p.y()]),
            ObjectSnapOptions {
                modes: ObjectSnapModes::only(ObjectSnapKind::Mid),
                mesh_edges: true,
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(hit.point(), p(5e-11, 0.));
}
