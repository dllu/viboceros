//! Independent public Mesh.GetOutlines captures; contour order and seam are free.
use serde_json::{Value, json};
use viboceros_geometry::{
    Frame3, GeometryError, MeshFace, MeshOutlineProjection, Point3, Tolerance, TriangleMesh,
    Vector3,
};

#[allow(dead_code)]
#[path = "support/mesh_outline.rs"]
mod reference;
use reference::{compare, frame, mesh, point, vector};

fn projection(value: &Value) -> MeshOutlineProjection {
    let plane = frame(&value["plane"]);
    if value["input"]["method"] == "plane" {
        return MeshOutlineProjection::Parallel(plane);
    }
    let v = &value["view"];
    let direction = vector(&v["direction"])
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    let normal = direction.scaled(-1.).unwrap();
    let perspective = v["perspective"].as_bool().unwrap();
    if !perspective
        && normal
            .cross(plane.z_axis().as_vector())
            .unwrap()
            .length()
            .unwrap()
            < 1e-12
    {
        return MeshOutlineProjection::Parallel(plane);
    }
    let [near, far] = [
        v["frustum"][4].as_f64().unwrap(),
        v["frustum"][5].as_f64().unwrap(),
    ];
    // NDC depth zero: arithmetic mean in parallel, harmonic in perspective.
    let distance = if perspective {
        2. / (near.recip() + far.recip())
    } else {
        near.midpoint(far)
    };
    let eye = point(&v["location"]);
    let right = direction
        .cross(vector(&v["up"]))
        .unwrap()
        .normalized_nonzero()
        .unwrap()
        .as_vector();
    let up = right.cross(direction).unwrap();
    let plane = Frame3::try_from_directions(
        eye.translated(direction.scaled(distance).unwrap()).unwrap(),
        right,
        up,
        Tolerance::DEFAULT,
    )
    .unwrap();
    if perspective {
        MeshOutlineProjection::Perspective { plane, eye }
    } else {
        MeshOutlineProjection::Parallel(plane)
    }
}

#[test]
fn public_plane_and_viewport_outline_references() {
    let mut cases = 0;
    for (family, text) in [
        (
            "sdk",
            include_str!("../../../tools/rhino_oracle/observations/mesh_outline_sdk.json"),
        ),
        (
            "projections",
            include_str!("../../../tools/rhino_oracle/observations/mesh_outline_projections.json"),
        ),
    ] {
        let data: Value = serde_json::from_str(text).unwrap();
        for row in data["results"].as_array().unwrap() {
            let value = &row["value"];
            // The exploratory viewport captures precede explicit frustum
            // recording. They are retained as evidence, not full comparisons.
            if family == "sdk" && value["input"]["method"] != "plane" {
                continue;
            }
            let source = mesh(&value["input"]["mesh"]);
            let before = source.clone();
            let actual = source
                .outlines(projection(value), Tolerance::MESH_VALIDATION)
                .unwrap();
            let max_error = compare(&actual, &value["outlines"], 1e-7);
            assert_eq!(source, before);
            assert_eq!(value["source_unchanged"], true);
            assert_eq!(value["returned_null"], actual.is_empty());
            eprintln!(
                "MESH_OUTLINE_SDK {}",
                json!({"case":row["id"],"max_error":max_error,
                    "outlines":actual.iter().map(|p| p.vertices().iter().map(|v|v.to_array()).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "source_unchanged":source == before})
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 61);
}

#[test]
fn eye_plane_and_unused_vertices_are_handled_without_changing_sources() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let plane = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let projection = MeshOutlineProjection::Perspective {
        plane,
        eye: p(0., 0., 10.),
    };
    let source = TriangleMesh::try_new(
        vec![p(0., 0., 2.), p(4., 0., 2.), p(0., 4., 2.), p(1., 1., 10.)],
        vec![[0, 1, 2]],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap();
    let before = source.clone();
    let result = source
        .outlines(projection, Tolerance::MESH_VALIDATION)
        .unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].vertices().iter().all(|p| p.z() == 0.));
    assert_eq!(source, before);
    let crossing = TriangleMesh::try_new(
        vec![p(0., 0., 9.), p(4., 0., 12.), p(0., 4., 9.)],
        vec![[0, 1, 2]],
        Tolerance::MESH_VALIDATION,
    )
    .unwrap();
    assert_eq!(
        crossing.outlines(projection, Tolerance::MESH_VALIDATION),
        Err(GeometryError::MeshOutlineCrossesEyePlane)
    );
}

#[test]
fn collapsed_and_out_of_range_projection_is_explicit() {
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let plane = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = TriangleMesh::try_from_face_records(
        vec![p(0., 0., 0.), p(1., 0., 1.), p(2., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    assert!(
        source
            .outlines(
                MeshOutlineProjection::Parallel(plane),
                Tolerance::MESH_VALIDATION
            )
            .unwrap()
            .is_empty()
    );
    let huge = TriangleMesh::try_from_face_records(
        vec![p(1e200, 0., 0.), p(0., 1e200, 0.), p(0., 0., 0.)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    assert_eq!(
        huge.outlines(
            MeshOutlineProjection::Parallel(plane),
            Tolerance::MESH_VALIDATION
        ),
        Err(GeometryError::MeshOutlineCoordinateRange)
    );
    let remote = TriangleMesh::try_from_face_records(
        vec![p(0., 0., -1e308), p(4., 0., -1e308), p(0., 4., -1e308)],
        vec![MeshFace::Triangle([0, 1, 2])],
    )
    .unwrap();
    assert_eq!(
        remote.outlines(
            MeshOutlineProjection::Perspective {
                plane,
                eye: p(0., 0., 1e308)
            },
            Tolerance::MESH_VALIDATION,
        ),
        Err(GeometryError::MeshOutlineCoordinateRange)
    );
}
