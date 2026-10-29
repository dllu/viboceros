use viboceros_geometry::{MeshFace, Point3, PolygonEdgePoint, Tolerance, TriangleMesh};
fn point(p: [f64; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn source() -> TriangleMesh {
    TriangleMesh::try_from_face_records(
        [
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [0., 2., 0.],
            [4., 1., 0.],
            [5., 0., 0.],
            [6., 1., 0.],
            [5., 2., 0.],
        ]
        .into_iter()
        .map(point)
        .collect(),
        vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Quad([4, 5, 6, 7])],
    )
    .unwrap()
}
fn pick(mesh: &TriangleMesh, p: [f64; 3]) -> PolygonEdgePoint {
    let p = point(p);
    mesh.wireframe_lines(Tolerance::MESH_VALIDATION)
        .unwrap()
        .iter()
        .enumerate()
        .map(|(edge, line)| {
            let parameter = line
                .closest_parameter(p, Tolerance::MESH_VALIDATION)
                .unwrap();
            (
                line.point_at(parameter).unwrap().distance_to(p).unwrap(),
                PolygonEdgePoint { edge, parameter },
            )
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
        .1
}
#[test]
fn flat_corner_sources_can_be_retessellated_or_preserved_without_relaxing_new_faces() {
    let source = source();
    let before = source.clone();
    let tol = Tolerance::MESH_VALIDATION;
    assert!(source.validate_face_geometry(tol).is_err());
    for (p, preserved) in [([1., 1., 0.], false), ([5.5, 1.5, 0.], true)] {
        let out = source
            .try_insert_edge_path(&[pick(&source, p)], None, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(out.vertices().len(), 9);
        assert_eq!(
            out.faces()
                .iter()
                .filter(|&&f| f == source.faces()[0])
                .count(),
            usize::from(preserved)
        );
        for &face in out.faces() {
            if preserved && face == source.faces()[0] {
                continue;
            }
            TriangleMesh::try_new_faces(out.vertices().to_vec(), vec![face], tol).unwrap();
        }
    }
    assert_eq!(
        source
            .try_insert_edge_path(&[], None, Tolerance::DEFAULT)
            .unwrap(),
        source
    );
    assert_eq!(source, before);
}
#[test]
fn invalid_source_boundaries_and_later_picks_remain_atomic_errors() {
    let source = source();
    let before = source.clone();
    let good = pick(&source, [1., 1., 0.]);
    assert!(
        source
            .try_insert_edge_path(
                &[
                    good,
                    PolygonEdgePoint {
                        edge: usize::MAX,
                        ..good
                    }
                ],
                None,
                Tolerance::DEFAULT
            )
            .is_err()
    );
    assert_eq!(source, before);
    for (points, face) in [
        (
            vec![[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]],
            MeshFace::Triangle([0, 1, 2]),
        ),
        (
            vec![[0., 0., 0.], [1., 0., 0.], [2., 0., 0.], [3., 0., 0.]],
            MeshFace::Quad([0, 1, 2, 3]),
        ),
        (
            vec![[0., 0., 0.], [0., 1., 0.], [4., 3., 0.], [0., 3., 0.]],
            MeshFace::Quad([0, 1, 2, 3]),
        ),
        (
            vec![[0., 0., 0.], [0., 0., 0.], [2., 0., 0.], [0., 2., 0.]],
            MeshFace::Quad([0, 1, 2, 3]),
        ),
    ] {
        let bad = TriangleMesh::try_from_face_records(
            points.into_iter().map(point).collect(),
            vec![face],
        )
        .unwrap();
        assert!(
            bad.try_insert_edge_path(
                &[PolygonEdgePoint {
                    edge: 0,
                    parameter: 0.5
                }],
                None,
                Tolerance::DEFAULT
            )
            .is_err()
        );
    }
}

#[test]
fn public_native_quad_conversion_retains_raw_faces_and_vertex_storage() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
    let assets: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("fixtures/mesh_insert_validation_assets.json")).unwrap(),
    )
    .unwrap();
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("observations/mesh_insert_conversion.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["results"].as_array().unwrap().len(), 18);
    for row in record["results"].as_array().unwrap() {
        let input = &assets[row["id"].as_str().unwrap()];
        let vertices: Vec<[f64; 3]> = serde_json::from_value(input["vertices"].clone()).unwrap();
        let records: Vec<Vec<u32>> = serde_json::from_value(input["faces"].clone()).unwrap();
        let faces = records
            .iter()
            .map(|f| match f.as_slice() {
                [a, b, c] => MeshFace::Triangle([*a, *b, *c]),
                [a, b, c, d] => MeshFace::Quad([*a, *b, *c, *d]),
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        let source =
            TriangleMesh::try_from_face_records(vertices.into_iter().map(point).collect(), faces)
                .unwrap();
        let before = source.clone();
        let (out, count) = source.triangulate_quads(Tolerance::DEFAULT).unwrap();
        assert_eq!(count, source.faces().iter().filter(|f| f.is_quad()).count());
        assert_eq!(
            serde_json::json!(
                out.vertices()
                    .iter()
                    .map(|p| p.to_array())
                    .collect::<Vec<_>>()
            ),
            row["value"]["mesh"]["vertices"]
        );
        assert_eq!(
            serde_json::json!(out.faces().iter().map(|f| f.indices()).collect::<Vec<_>>()),
            row["value"]["mesh"]["faces"],
            "{}",
            row["id"]
        );
        assert_eq!(source, before);
    }
}
