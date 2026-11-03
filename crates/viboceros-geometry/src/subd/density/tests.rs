use super::*;
use crate::{Point3, SubD};
use serde_json::Value;

fn capture() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/rhino_oracle/observations/analysis_mesh_parameters.json"
    )))
    .unwrap()
}
#[test]
fn analysis_subd_density_matches_native_defaults_and_adaptive_boundaries() {
    let native = capture();
    let v = &native["results"][0]["value"];
    assert_eq!(v["current"], v["default"]);
    assert_eq!(
        SubDMeshDensity::default().level(),
        v["default"]["subd_level"].as_u64().unwrap() as u8
    );
    assert_eq!(
        SubDMeshDensity::default().is_adaptive(),
        v["default"]["subd_adaptive"].as_bool().unwrap()
    );
    assert_eq!(
        SubDMeshDensity::ADAPTIVE_QUAD_MAXIMUM,
        v["adaptive_quad_maximum"].as_u64().unwrap()
    );
    for row in v["mappings"].as_array().unwrap() {
        let level = row["level"].as_u64().unwrap() as u8;
        for count in row["counts"].as_array().unwrap() {
            let faces = count["faces"].as_u64().unwrap() as usize;
            assert_eq!(
                SubDMeshDensity::new(level, true)
                    .unwrap()
                    .effective_level(faces),
                count["effective"].as_u64().unwrap() as u32
            );
            assert_eq!(
                SubDMeshDensity::new(level, false)
                    .unwrap()
                    .effective_level(faces),
                u32::from(level)
            );
        }
    }
    assert!(SubDMeshDensity::new(7, true).is_none());
    assert_eq!(
        SubDMeshDensity::new(6, true)
            .unwrap()
            .effective_level(usize::MAX),
        1
    );
}

#[test]
fn analysis_subd_density_face_counts_match_native_meshes_at_every_slider_level() {
    let native = capture();
    for row in native["results"][0]["value"]["meshes"].as_array().unwrap() {
        let name = row["source"].as_str().unwrap();
        let subd = source(name);
        let archive = subd.archive().to_vec();
        for record in row["levels"].as_array().unwrap() {
            let level = record["level"].as_u64().unwrap() as u8;
            let mesh = subd
                .curvature_mesh_with_density(SubDMeshDensity::new(level, true).unwrap())
                .unwrap();
            assert_eq!(
                mesh.mesh.faces().len() as u64,
                record["mesh"]["faces"].as_u64().unwrap(),
                "{name} level {level}"
            );
            if level == 4 {
                assert_eq!(record["mesh"], row["extraction"]);
            }
            assert_eq!(subd.archive(), archive);
        }
    }
}

fn source(name: &str) -> SubD {
    let (points, faces) = match name {
        "plane" => (
            vec![[0., 0., 0.], [3., 0., 0.], [3., 2., 0.], [0., 2., 0.]],
            vec![vec![0, 1, 2, 3]],
        ),
        "box" => (
            vec![
                [-2., -2., -2.],
                [2., -2., -2.],
                [2., 2., -2.],
                [-2., 2., -2.],
                [-2., -2., 2.],
                [2., -2., 2.],
                [2., 2., 2.],
                [-2., 2., 2.],
            ],
            vec![
                vec![0, 3, 2, 1],
                vec![4, 5, 6, 7],
                vec![0, 1, 5, 4],
                vec![1, 2, 6, 5],
                vec![2, 3, 7, 6],
                vec![3, 0, 4, 7],
            ],
        ),
        _ => {
            let n = if name == "triangle" { 3 } else { 5 };
            (
                (0..n)
                    .map(|i| {
                        let t = std::f64::consts::TAU * i as f64 / n as f64;
                        [t.cos(), t.sin(), 0.]
                    })
                    .collect(),
                vec![(0..n as u32).collect()],
            )
        }
    };
    SubD::try_from_polygons(
        points
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect(),
        faces,
    )
    .unwrap()
}

#[test]
fn analysis_subd_density_limit_points_match_native_subdivided_geometry() {
    let native: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/rhino_oracle/observations/analysis_subd_details.json"
        ))
        .unwrap(),
    )
    .unwrap();
    for row in native["results"][0]["value"]["limits"].as_array().unwrap() {
        let name = row["source"].as_str().unwrap();
        let level = row["level"].as_u64().unwrap() as u8;
        // Nonquad fragments require subdivision before making quad analysis meshes.
        if level == 0 && matches!(name, "triangle" | "pentagon") {
            continue;
        }
        let subd = source(name);
        let mesh = subd
            .curvature_mesh_with_density(SubDMeshDensity::new(level, false).unwrap())
            .unwrap();
        assert_eq!(
            mesh.mesh.vertices().len() as u64,
            row["vertices"].as_u64().unwrap()
        );
        assert_eq!(
            mesh.mesh.faces().len() as u64,
            row["faces"].as_u64().unwrap()
        );
        for point in row["samples"].as_array().unwrap() {
            let point = Point3::try_new(
                point[0].as_f64().unwrap(),
                point[1].as_f64().unwrap(),
                point[2].as_f64().unwrap(),
            )
            .unwrap();
            let error = mesh
                .mesh
                .vertices()
                .iter()
                .map(|p| p.distance_to(point).unwrap())
                .fold(f64::INFINITY, f64::min);
            assert!(error < 1e-10, "{name} level {level}: {error}");
        }
    }
}

#[test]
fn analysis_subd_density_zero_nonquad_fragments_subdivide_and_plain_tessellation_rejects_them() {
    for (name, faces) in [("triangle", 3), ("pentagon", 5)] {
        let subd = source(name);
        assert!(subd.tessellate(0).is_err());
        let mesh = subd
            .curvature_mesh_with_density(SubDMeshDensity::new(0, false).unwrap())
            .unwrap();
        assert_eq!(mesh.mesh.faces().len(), faces);
    }
}
