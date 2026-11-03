use super::*;
use serde_json::Value;
fn points(value: &Value) -> Vec<Point3> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            Point3::try_new(
                p[0].as_f64().unwrap(),
                p[1].as_f64().unwrap(),
                p[2].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect()
}
fn source(value: &Value) -> SubD {
    let faces = value["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let f = f
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            if f.len() == 4 {
                MeshFace::Quad(f.try_into().unwrap())
            } else {
                MeshFace::Triangle(f.try_into().unwrap())
            }
        })
        .collect();
    let mesh = TriangleMesh::try_from_face_records(points(&value["points"]), faces).unwrap();
    let s = SubD::try_from_mesh(&mesh, Default::default(), crate::Tolerance::DEFAULT).unwrap();
    let tags = value["corners"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| (i.as_u64().unwrap() as usize, 3))
        .collect::<Vec<_>>();
    let s = s.try_with_vertex_tags(&tags).unwrap();
    let creases = value["creases"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| [p[0].as_u64().unwrap() as u32, p[1].as_u64().unwrap() as u32])
        .collect::<Vec<_>>();
    let edits = s
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, p)| creases.iter().any(|c| p == &c || p == &&[c[1], c[0]]))
        .map(|(i, _)| (i, 2))
        .collect::<Vec<_>>();
    s.try_with_edge_tags(&edits).unwrap()
}
fn check(s: &SubD, expected: &Value, id: &str) {
    assert_eq!(
        s.control_points(),
        points(&expected["points"]),
        "{id}: controls"
    );
    let near = points(&expected["surface_points"]);
    assert_eq!(s.surface_points().len(), near.len(), "{id}: limit count");
    for (a, b) in s.surface_points().iter().zip(near) {
        assert!(a.distance_to(b).unwrap() < 1e-10, "{id}: {a:?} {b:?}");
    }
    assert_eq!(
        serde_json::to_value(s.faces()).unwrap(),
        expected["faces"],
        "{id}: faces"
    );
    assert_eq!(
        serde_json::to_value(s.edges()).unwrap(),
        expected["edges"],
        "{id}: edges"
    );
    assert_eq!(
        serde_json::to_value(s.edge_tags()).unwrap(),
        expected["edge_tags"],
        "{id}: edge tags"
    );
    assert_eq!(
        serde_json::to_value(s.vertex_tags()).unwrap(),
        expected["vertex_tags"],
        "{id}: vertex tags"
    );
}
#[test]
fn subd_join_sdk_matches_native_topology_tags_control_and_limit_points() {
    let file = std::fs::File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/rhino_oracle/observations/subd_join_sdk.json.gz"
    ))
    .unwrap();
    let data: Value = serde_json::from_reader(flate2::read::GzDecoder::new(file)).unwrap();
    for row in data["results"].as_array().unwrap() {
        let v = &row["value"];
        let id = row["id"].as_str().unwrap();
        let sources = v["input"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(source)
            .collect::<Vec<_>>();
        for (s, input) in sources.iter().zip(v["before"].as_array().unwrap()) {
            check(s, input, id);
        }
        let archives = sources
            .iter()
            .map(|s| s.archive().to_vec())
            .collect::<Vec<_>>();
        let output = join_subds(
            &sources.iter().collect::<Vec<_>>(),
            v["input"]["tolerance"].as_f64().unwrap(),
            v["input"]["crease"].as_bool().unwrap(),
        )
        .unwrap();
        let expected = v["outputs"].as_array().unwrap();
        assert_eq!(output.len(), expected.len(), "{id}");
        for (s, e) in output.iter().zip(expected) {
            check(&s.subd, e, id);
        }
        for (s, a) in sources.iter().zip(archives) {
            assert_eq!(s.archive(), a);
        }
    }
}

#[test]
fn subd_join_retains_appended_soft_edges_and_unrelated_corners() {
    let geometry = serde_json::json!({"points":[[0.,0.,0.],[4.,0.,0.],[8.,0.,0.],[0.,3.,0.],[4.,3.,0.],[8.,3.,0.]],"faces":[[0,1,4,3],[1,2,5,4]]});
    let source = source(&geometry).try_with_vertex_tags(&[(5, 3)]).unwrap();
    let edge = source
        .edges()
        .iter()
        .position(|&p| p == [1, 4] || p == [4, 1])
        .unwrap();
    let source = source
        .try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
        .unwrap()
        .0;
    let shift =
        crate::AffineTransform3::from_translation(crate::Vector3::try_new(4., 0., 0.).unwrap());
    let source = source.transformed(shift).unwrap();
    let seed = SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let output = join_subds(&[&seed, &source], 0.001, false).unwrap();
    assert_eq!(output.len(), 1);
    let result = &output[0].subd;
    let index = |p: Point3| {
        result
            .control_points()
            .iter()
            .position(|q| *q == p)
            .unwrap() as u32
    };
    let ends = source.edges()[edge].map(|v| index(source.control_points()[v as usize]));
    let (i, oriented) = result
        .edges()
        .iter()
        .enumerate()
        .find_map(|(i, &p)| {
            if p == ends {
                Some((i, true))
            } else if p == [ends[1], ends[0]] {
                Some((i, false))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        result.edge_sharpness()[i],
        if oriented { [1.25, 2.5] } else { [2.5, 1.25] }
    );
    assert_eq!(
        result.vertex_tags()[index(source.control_points()[5]) as usize],
        3
    );
    assert!(join_subds(&[&seed, &source], f64::NAN, false).is_err());
    assert!(join_subds(&[&seed, &source], -1., false).is_err());
}

#[test]
fn subd_join_large_disjoint_batches_retain_each_source_once() {
    let seed = SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let sources = (0..2000)
        .map(|i| {
            seed.transformed(crate::AffineTransform3::from_translation(
                crate::Vector3::try_new(i as f64 * 12., 0., 0.).unwrap(),
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let output = join_subds(&sources.iter().collect::<Vec<_>>(), 0.001, false).unwrap();
    assert_eq!(output.len(), sources.len());
    for (i, part) in output.iter().enumerate() {
        assert_eq!(part.source_indices, [i]);
        assert_eq!(part.subd.archive(), sources[i].archive());
    }
}

#[test]
fn subd_join_dense_duplicate_admission_stops_at_its_work_budget() {
    let seed = SubD::try_from_polygons(
        [[0., 0., 0.], [4., 0., 0.], [4., 3., 0.], [0., 3., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
        vec![vec![0, 1, 2, 3]],
    )
    .unwrap();
    let before = seed.archive().to_vec();
    let error = join_subds(&vec![&seed; 250], 0.001, false).unwrap_err();
    assert!(error.to_string().contains("admission work limit"));
    assert_eq!(seed.archive(), before);
}
