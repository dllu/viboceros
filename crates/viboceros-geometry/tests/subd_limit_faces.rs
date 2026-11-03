use viboceros_geometry::{Point3, SubD};
#[path = "support/subd_to_nurbs.rs"]
mod native;

fn check_native(scene: &str) {
    let value = native::case(scene);
    let source = native::source(&value["before"]);
    let archive = source.archive().to_vec();
    let grids = source.limit_face_grids(2).unwrap();
    let expected = value["before"]["surface_mesh"]["vertices"]
        .as_array()
        .unwrap();
    assert_eq!(
        grids.iter().map(|g| g.points.len()).sum::<usize>(),
        expected.len()
    );
    // Native n-gon fragments start at the face center, diagonally opposite the
    // source corner. A 180-degree rotation maps them to our corner-first chart.
    let error = grids
        .iter()
        .flat_map(|g| {
            (0..g.points.len()).map(move |i| {
                &g.points[if g.corner.is_some() {
                    g.points.len() - 1 - i
                } else {
                    i
                }]
            })
        })
        .zip(expected)
        .map(|(&p, q)| p.distance_to(native::point(q)).unwrap())
        .fold(0_f64, f64::max);
    assert!(error < 1e-11, "{scene}: {error}");
    let edges = source.edge_polylines(2).unwrap();
    for grid in &grids {
        let face = &source.faces()[grid.face];
        let side = grid.side;
        let stride = side + 1;
        let corner = grid.corner.unwrap_or(0);
        assert!(
            grid.points[0]
                .distance_to(source.surface_points()[face[corner] as usize])
                .unwrap()
                < 1e-11
        );
        let boundary = if grid.corner.is_none() {
            vec![
                (0, (0..=side).collect::<Vec<_>>()),
                (1, (0..=side).map(|v| v * stride + side).collect()),
                (2, (0..=side).map(|u| side * stride + side - u).collect()),
                (3, (0..=side).map(|v| (side - v) * stride).collect()),
            ]
        } else {
            vec![
                (corner, (0..=side).collect()),
                (
                    (corner + face.len() - 1) % face.len(),
                    (0..=side).map(|v| (side - v) * stride).collect(),
                ),
            ]
        };
        for (i, stations) in boundary {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            let (e, reversed) = source
                .edges()
                .iter()
                .enumerate()
                .find_map(|(e, &v)| {
                    if v == [a, b] {
                        Some((e, false))
                    } else if v == [b, a] {
                        Some((e, true))
                    } else {
                        None
                    }
                })
                .unwrap();
            for (j, index) in stations.into_iter().enumerate() {
                let j = if grid.corner.is_some() && i != corner {
                    side + j
                } else {
                    j
                };
                let j = if reversed { edges[e].len() - 1 - j } else { j };
                assert!(grid.points[index].distance_to(edges[e][j]).unwrap() < 1e-11);
            }
        }
    }
    assert_eq!(source.archive(), archive);
}

#[test]
fn hard_corners_match_native_limit_charts() {
    check_native("quad_corners");
}
#[test]
fn smooth_boundary_matches_native_limit_charts() {
    check_native("quad_smooth");
}
#[test]
fn grid_matches_native_limit_charts() {
    check_native("grid");
}
#[test]
fn extraordinary_vertices_match_native_limit_charts() {
    check_native("box");
}
#[test]
fn ngon_sectors_match_native_limit_charts() {
    check_native("pentagon");
}
#[test]
fn crease_sectors_match_native_limit_charts() {
    check_native("creased_box");
}
#[test]
fn periodic_torus_matches_native_limit_charts() {
    check_native("torus");
}

#[test]
fn chart_lineage_survives_refined_levels_soft_creases_and_coincident_components() {
    let source = native::source(&native::case("box")["before"]);
    let (soft, _) = source
        .try_with_soft_crease_weights(&[(0, [24.75, 75.25])])
        .unwrap();
    for source in [
        source.clone(),
        source.subdivided(1).unwrap(),
        source.subdivided_faces(&[0]).unwrap(),
        soft,
    ] {
        let archive = source.archive().to_vec();
        let coarse = source.limit_face_grids(1).unwrap();
        let fine = source.limit_face_grids(3).unwrap();
        assert_eq!(coarse.len(), fine.len());
        for (a, b) in coarse.iter().zip(&fine) {
            assert_eq!((a.face, a.corner), (b.face, b.corner));
            for v in 0..=a.side {
                for u in 0..=a.side {
                    assert!(
                        a.points[v * (a.side + 1) + u]
                            .distance_to(b.points[4 * v * (b.side + 1) + 4 * u])
                            .unwrap()
                            < 1e-11
                    );
                }
            }
        }
        assert_eq!(source.archive(), archive);
    }
    let mut points = source.control_points().to_vec();
    points.extend_from_within(..);
    let mut faces = source.faces().to_vec();
    faces.extend(
        source
            .faces()
            .iter()
            .map(|f| f.iter().map(|v| v + 8).collect()),
    );
    let doubled = SubD::try_from_polygons(points, faces).unwrap();
    let charts = doubled.limit_face_grids(2).unwrap();
    assert_eq!(charts.len(), 12);
    for i in 0..6 {
        assert_eq!(charts[i].points, charts[i + 6].points);
        assert_eq!((charts[i].face, charts[i + 6].face), (i, i + 6));
    }
}

#[test]
fn density_archive_and_preallocation_limits_are_checked() {
    let source = native::source(&native::case("box")["before"]);
    for density in [0, 7, u32::MAX] {
        assert!(source.limit_face_grids(density).is_err());
    }
    assert!(viboceros_opennurbs::subd_face_grids(b"invalid archive", 2).is_err());
    assert_eq!(source.limit_face_grids(6).unwrap()[0].points.len(), 65 * 65);
    let points = (0..=16)
        .flat_map(|y| (0..=16).map(move |x| Point3::try_new(x as f64, y as f64, 0.).unwrap()))
        .collect();
    let faces = (0..16)
        .flat_map(|y| {
            (0..16).map(move |x| {
                let i = y * 17 + x;
                vec![i, i + 1, i + 18, i + 17]
            })
        })
        .collect();
    let source = SubD::try_from_polygons(points, faces).unwrap();
    let archive = source.archive().to_vec();
    // 256 * 4096 refined quads exceed the one-million-face budget.
    assert!(
        source
            .limit_face_grids(6)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert_eq!(source.archive(), archive);
    assert_eq!(source.limit_face_grids(1).unwrap().len(), 256);
}
