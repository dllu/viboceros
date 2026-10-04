use super::*;
use serde_json::Value;

fn point(p: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(p.clone()).unwrap()).unwrap()
}

fn record(snapshot: &Value) -> TriangleMesh {
    let vertices = snapshot["mesh"]["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect();
    let faces = snapshot["mesh"]["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let indices: Vec<u32> = serde_json::from_value(f.clone()).unwrap();
            match *indices.as_slice() {
                [a, b, c] => MeshFace::Triangle([a, b, c]),
                [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                _ => panic!("unsupported polygon"),
            }
        })
        .collect();
    TriangleMesh::try_from_face_records(vertices, faces)
        .unwrap()
        .try_with_vertex_colors(Some(
            serde_json::from_value(snapshot["colors"].clone()).unwrap(),
        ))
        .unwrap()
}

fn compare(mesh: &TriangleMesh, snapshot: &Value) {
    assert_eq!(*mesh, record(snapshot));
    assert_eq!(
        mesh.topology().is_closed(),
        snapshot["closed"].as_bool().unwrap()
    );
    let data = mesh.topology_data();
    // SDK and kernel number edges differently; compare locations and all uses.
    let mut actual = data
        .edges
        .iter()
        .map(|(&(a, b), uses)| {
            let mut points = [
                data.topological_points[a].to_array(),
                data.topological_points[b].to_array(),
            ];
            points.sort_by(|a, b| a.partial_cmp(b).unwrap());
            (points, uses.uses().map(|u| u.face).collect::<Vec<_>>())
        })
        .collect::<Vec<_>>();
    let mut expected = snapshot["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            let mut points: [[Real; 3]; 2] =
                serde_json::from_value(edge["points"].clone()).unwrap();
            points.sort_by(|a, b| a.partial_cmp(b).unwrap());
            (
                points,
                serde_json::from_value::<Vec<usize>>(edge["faces"].clone()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort_by(|a, b| a.partial_cmp(b).unwrap());
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(actual, expected);
    for (i, native) in snapshot["face_normals"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let expected: [Real; 3] = serde_json::from_value(native.clone()).unwrap();
        if expected == [0.; 3] {
            assert!(mesh.polygon_face_normal(i).is_err());
        } else {
            assert_eq!(
                mesh.polygon_face_normal(i).unwrap().as_vector().to_array(),
                expected
            );
        }
    }
    let area = snapshot["mass"]["area"].as_f64().unwrap();
    assert!((mesh.area().unwrap() - area).abs() < 1e-12);
    let mass = mesh.area_mass_properties().unwrap();
    assert!((mass.area().unwrap() - area).abs() < 1e-12);
    if area > 0. {
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(point(&snapshot["mass"]["centroid"]))
                .unwrap()
                < 1e-12
        );
    } else {
        // RhinoCommon reports Origin. The mathematical centroid is undefined.
        assert!(mass.centroid().is_err());
    }
    let target = Point3::try_new(1., -1., 1.).unwrap();
    let index = (0..mesh.face_count())
        .min_by(|&a, &b| {
            target.compare_distances(
                mesh.closest_point_on_face(a, target).unwrap(),
                mesh.closest_point_on_face(b, target).unwrap(),
            )
        })
        .unwrap();
    assert_eq!(
        index,
        snapshot["closest"]["face"].as_u64().unwrap() as usize
    );
    assert!(
        mesh.closest_point_on_face(index, target)
            .unwrap()
            .distance_to(point(&snapshot["closest"]["point"]))
            .unwrap()
            < 1e-12
    );
    assert!(MeshSolid::from_closed_mesh(mesh).is_none());
}

#[test]
fn stored_mesh_collapse_and_recovery_match_six_native_grip_recipes() {
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/mesh_edit_records.json"
    ))
    .unwrap();
    assert_eq!(captured["results"].as_array().unwrap().len(), 6);
    for row in captured["results"].as_array().unwrap() {
        let v = &row["value"];
        let source = record(&v["before"]);
        source.validate_face_geometry(Tolerance::DEFAULT).unwrap();
        let mut vertices = source.vertices().to_vec();
        let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
        match row["id"].as_str().unwrap() {
            "mesh-records-mixed_mirror" => {
                vertices[0] = p(-2., 0.);
                vertices[2] = p(2., 0.);
            }
            "mesh-records-triangle_line" => vertices[2] = p(1., 0.),
            "mesh-records-triangle_point" => vertices.fill(p(0., 0.)),
            "mesh-records-quad_edge" => vertices[1] = p(0., 0.),
            "mesh-records-quad_line" => vertices
                .iter_mut()
                .enumerate()
                .for_each(|(i, v)| *v = p(i as Real, 0.)),
            "mesh-records-quad_point" => vertices.fill(p(0., 0.)),
            id => panic!("unknown recipe {id}"),
        }
        let edited = source.try_with_edited_vertices(vertices).unwrap();
        assert!(edited.signed_volume().unwrap().abs() < 1e-12);
        let data = edited.topology_data();
        let sides = topology_face_edge_indices(&edited, &data);
        for (face, edges) in edited.faces().iter().zip(&sides) {
            let expected = face
                .indices()
                .iter()
                .zip(face.indices().iter().cycle().skip(1))
                .take(face.vertex_count())
                .filter(|(a, b)| edited.vertices()[**a as usize] != edited.vertices()[**b as usize])
                .count();
            assert_eq!(edges.len(), expected);
        }
        // These operations may reject collapsed geometry. They must finish
        // without topology indexing/ordering panics or modifying the source.
        let _ = edited.unwelded_vertices(std::f64::consts::PI);
        let _ = edited.unwelded_topology_edges(&(0..data.edges.len()).collect::<Vec<_>>());
        let _ = edited
            .unwelded_topology_vertices(&(0..data.topological_vertex_count).collect::<Vec<_>>());
        compare(&source, &v["before"]);
        compare(&edited, &v["edited"]);
        let restored = edited
            .try_with_edited_vertices(source.vertices().to_vec())
            .unwrap();
        compare(&restored, &v["restored"]);
        assert_eq!(restored, source);
        let transform = AffineTransform3::try_nonuniform_scale(p(0., 0.), [2., 3., 4.]).unwrap();
        let mapped = edited.transformed(transform, Tolerance::DEFAULT).unwrap();
        assert_eq!(mapped.faces(), edited.faces());
        assert_eq!(mapped.vertex_colors(), edited.vertex_colors());
        for (&old, &new) in edited.vertices().iter().zip(mapped.vertices()) {
            assert_eq!(new, transform.transform_point(old).unwrap());
        }
    }
}

#[test]
fn unwelding_an_affected_collapsed_corner_returns_an_error_before_rebuilding() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let mesh = TriangleMesh::try_from_face_records(
        vec![p(0., 0.), p(0., 0.), p(2., 2.), p(0., 2.), p(-2., 0.)],
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([0, 3, 4])],
    )
    .unwrap();
    let before = mesh.clone();
    assert!(mesh.unwelded_topology_vertices(&[0]).is_err());
    assert_eq!(mesh, before);
}

#[test]
fn stored_records_preserve_ngons_and_strict_generation_validation() {
    let p = |x, y| Point3::try_new(x, y, 0.).unwrap();
    let source = TriangleMesh::try_new_faces(
        vec![p(0., 0.), p(2., 0.), p(2., 2.), p(0., 2.)],
        vec![MeshFace::Quad([0, 1, 2, 3])],
        Tolerance::DEFAULT,
    )
    .unwrap()
    .try_with_ngons(vec![MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0])])
    .unwrap();
    let collapsed = vec![p(0., 0.); 4];
    assert!(
        source
            .try_with_mapped_vertices(collapsed.clone(), Tolerance::DEFAULT)
            .is_err()
    );
    assert!(
        TriangleMesh::try_new_faces(
            collapsed.clone(),
            source.faces().to_vec(),
            Tolerance::DEFAULT
        )
        .is_err()
    );
    let edited = source.try_with_edited_vertices(collapsed).unwrap();
    assert_eq!(edited.ngons(), source.ngons());
    assert_eq!(edited.triangles(), source.triangles());
    assert_eq!(edited.face_aspect_ratio(0).unwrap(), Real::INFINITY);
    assert_eq!(
        edited
            .try_with_edited_vertices(source.vertices().to_vec())
            .unwrap(),
        source
    );
    assert!(source.try_with_edited_vertices(vec![]).is_err());
    for face in [
        MeshFace::Triangle([0, 1, 4]),
        MeshFace::Triangle([0, 1, 0]),
        MeshFace::Quad([0, 1, 2, 4]),
        MeshFace::Quad([0, 1, 2, 1]),
    ] {
        assert!(
            TriangleMesh::try_from_face_records(source.vertices().to_vec(), vec![face]).is_err()
        );
    }
    assert!(TriangleMesh::try_from_face_records(vec![], vec![]).is_err());
    // A tetrahedron collapsed to a point cannot certify a solid.
    let point_shell = TriangleMesh::try_from_face_records(
        vec![p(0., 0.); 4],
        vec![
            MeshFace::Triangle([0, 1, 2]),
            MeshFace::Triangle([0, 3, 1]),
            MeshFace::Triangle([0, 2, 3]),
            MeshFace::Triangle([1, 3, 2]),
        ],
    )
    .unwrap();
    assert!(!point_shell.topology().is_closed());
    assert!(MeshSolid::from_closed_mesh(&point_shell).is_none());
}

#[test]
fn collapsed_closest_queries_keep_extreme_and_subnormal_points_finite() {
    for scale in [Real::from_bits(1), 1., 1e308] {
        let p = |x, y| Point3::try_new(x * scale, y * scale, 0.).unwrap();
        let mesh = TriangleMesh::try_from_face_records(
            vec![p(-1., 0.), p(1., 0.), p(0., 0.)],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        assert_eq!(mesh.closest_point_on_face(0, p(0., 1.)).unwrap(), p(0., 0.));
        assert_eq!(mesh.closest_point_on_face(0, p(1., 1.)).unwrap(), p(1., 0.));
    }
}

#[test]
fn exact_closest_fallback_covers_all_triangle_regions_and_thin_interiors() {
    for scale in [1e-200, 1., 1e200, 1e308] {
        let p = |x, y, z| Point3::try_new(x * scale, y * scale, z * scale).unwrap();
        let mesh = TriangleMesh::try_from_face_records(
            vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1., 0.)],
            vec![MeshFace::Triangle([0, 1, 2])],
        )
        .unwrap();
        for (query, expected) in [
            ([-0.5, -0.5, 0.5], [0., 0., 0.]),
            ([1., -0.5, 0.5], [1., 0., 0.]),
            ([-0.5, 1., 0.5], [0., 1., 0.]),
            ([0.5, -0.5, 0.5], [0.5, 0., 0.]),
            ([-0.5, 0.5, 0.5], [0., 0.5, 0.]),
            ([0.75, 0.75, 0.5], [0.5, 0.5, 0.]),
            ([0.25, 0.25, 0.5], [0.25, 0.25, 0.]),
        ] {
            let actual = mesh
                .closest_point_on_face(0, p(query[0], query[1], query[2]))
                .unwrap();
            let expected = p(expected[0], expected[1], expected[2]);
            for (a, b) in actual.to_array().into_iter().zip(expected.to_array()) {
                assert!((a / scale - b / scale).abs() < 1e-14);
            }
        }
    }
    // Very unequal perpendicular edges also require the fallback: angular
    // testing alone misses underflow of the short edge's dot products.
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let mesh = TriangleMesh::try_from_face_records(
        vec![p(0., 0., 0.), p(1., 0., 0.), p(0., 1e-200, 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    assert_eq!(
        mesh.closest_point_on_face(0, p(0.25, 2.5e-201, 1.))
            .unwrap(),
        p(0.25, 2.5e-201, 0.)
    );
}
