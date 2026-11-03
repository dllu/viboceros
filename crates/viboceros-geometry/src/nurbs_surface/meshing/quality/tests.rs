use super::*;
use crate::{Frame3, Vector3};

fn polynomial(degree: usize, heights: &[Real], rational: bool) -> NurbsSurface {
    let controls = (0..=1)
        .flat_map(|v| {
            (0..=degree).map(move |u| {
                WeightedPoint3::try_new(
                    Point3::try_new(3. * u as Real / degree as Real, 2. * v as Real, heights[u])
                        .unwrap(),
                    if rational {
                        0.4 + (u + v) as Real / degree as Real
                    } else {
                        1.
                    },
                )
                .unwrap()
            })
        })
        .collect();
    NurbsSurface::try_new_rational(
        degree,
        1,
        degree + 1,
        2,
        controls,
        [vec![0.; degree + 1], vec![1.; degree + 1]].concat(),
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn sample_error(surface: &NurbsSurface, resolution: [usize; 2]) -> Real {
    let (mesh, uv) = surface
        .tessellate_grid_with_resolution(resolution, true, Tolerance::DEFAULT, true)
        .unwrap();
    let mut error: Real = 0.;
    for triangle in mesh.triangles() {
        let p = triangle.map(|i| mesh.vertices()[i as usize].to_array());
        let uv = triangle.map(|i| uv[i as usize]);
        for a in 0..=4 {
            for b in 0..=4 - a {
                let weights = [a as Real / 4., b as Real / 4., (4 - a - b) as Real / 4.];
                let u = (0..3).map(|i| weights[i] * uv[i].x()).sum();
                let v = (0..3).map(|i| weights[i] * uv[i].y()).sum();
                let linear = Point3::try_from(std::array::from_fn(|k| {
                    (0..3).map(|i| weights[i] * p[i][k]).sum::<Real>()
                }))
                .unwrap();
                error = error.max(surface.evaluate(u, v).unwrap().distance_to(linear).unwrap());
            }
        }
    }
    error
}

#[test]
fn density_chord_rule_matches_public_opennurbs_values() {
    for (density, ratio) in [
        (0., 0.),
        (0.25, 10_f64.powf(-2.25)),
        (0.5, 0.001),
        (0.8, 10_f64.powf(-3.6)),
        (1., 0.0001),
    ] {
        assert!((chord_height(density, 7.) - 7. * ratio).abs() < 1e-15);
    }
}

#[test]
fn rational_bezier_bound_includes_the_bilinear_diagonal_error_and_missed_midpoint_waves() {
    let saddle = NurbsSurface::try_bilinear(
        [[0., 0., 0.], [1., 0., 0.], [1., 1., 1.], [0., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap()),
    )
    .unwrap();
    let error = Net::new(&saddle).unwrap().chord_bound().unwrap();
    assert!((error - 0.25).abs() < 1e-12);
    assert!((sample_error(&saddle, [1, 1]) - 0.25).abs() < 1e-12);
    let wave = polynomial(5, &[0., 2., -2., 2., -2., 0.], false);
    assert!(wave.evaluate(0.5, 0.5).unwrap().z().abs() < 1e-15);
    assert!(Net::new(&wave).unwrap().chord_bound().unwrap() > 0.1);
    let counts = counts(&wave, 0.8).unwrap();
    assert!(counts[0] > 4);
    assert_eq!(counts[1], 1);
    let box3 = wave.control_point_bounds();
    assert!(
        sample_error(&wave, counts)
            <= chord_height(0.8, box3.min().distance_to(box3.max()).unwrap())
    );
}

#[test]
fn curved_meshes_meet_sampled_chord_targets_without_refining_straight_directions() {
    let frame = Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let sphere = NurbsSurface::try_sphere(frame, 2.).unwrap();
    let rational = polynomial(3, &[0., 1., -1., 0.], true);
    for surface in [&sphere, &rational] {
        let bbox = surface.control_point_bounds();
        let size = bbox.min().distance_to(bbox.max()).unwrap();
        let mut previous = [1, 1];
        for density in [0.25, 0.5, 0.8, 1.] {
            let resolution = counts(surface, density).unwrap();
            assert!(resolution[0] * resolution[1] >= previous[0] * previous[1]);
            assert!(sample_error(surface, resolution) <= chord_height(density, size) * 1.00000001);
            previous = resolution;
        }
    }
    let extrusion = polynomial(3, &[0., 1., -1., 0.], false);
    assert_eq!(counts(&extrusion, 1.).unwrap()[1], 1);
}

#[test]
fn signed_weights_resolve_when_possible_and_poles_fail_with_a_bounded_error() {
    let make = |middle| {
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            (0..2)
                .flat_map(|v| {
                    (0..3).map(move |u| {
                        WeightedPoint3::try_new(
                            Point3::try_new(u as Real, v as Real, if u == 1 { 1. } else { 0. })
                                .unwrap(),
                            if u == 1 { middle } else { 1. },
                        )
                        .unwrap()
                    })
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
            vec![0., 0., 1., 1.],
        )
        .unwrap()
    };
    let regular = make(-0.1);
    let resolution = counts(&regular, 0.5).unwrap();
    assert!(sample_error(&regular, resolution) < 0.01);
    assert_eq!(
        counts(&make(-1.), 1.),
        Err(GeometryError::MeshSubdivisionLimit)
    );
}

#[test]
fn common_weight_scaling_and_world_translation_do_not_change_the_selected_grid() {
    let source = polynomial(3, &[0., 1., -1., 0.], true);
    let base = counts(&source, 0.8).unwrap();
    for factor in [1e-150, -1e-150, 1e150, -1e150] {
        let controls = source
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * factor).unwrap())
            .collect();
        let copy = NurbsSurface::try_new_rational(
            3,
            1,
            4,
            2,
            controls,
            source.knots_u().to_vec(),
            source.knots_v().to_vec(),
        )
        .unwrap();
        assert_eq!(counts(&copy, 0.8).unwrap(), base);
    }
    let moved = source
        .transformed(crate::AffineTransform3::from_translation(
            Vector3::try_new(1e9, -1e9, 1e9).unwrap(),
        ))
        .unwrap();
    assert_eq!(counts(&moved, 0.8).unwrap(), base);
}

#[test]
fn public_native_mesh_vertices_agree_with_the_rust_surface_geometry() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/rhino_oracle/observations/nurbs_meshing.json"
    )))
    .unwrap();
    let degree = 4;
    let bump = NurbsSurface::try_new(
        4,
        4,
        5,
        5,
        (0..=degree)
            .flat_map(|v| {
                (0..=degree).map(move |u| {
                    Point3::try_new(
                        3. * u as Real / degree as Real,
                        2. * v as Real / degree as Real,
                        if u == 2 && v == 2 { 4. } else { 0. },
                    )
                    .unwrap()
                })
            })
            .collect(),
        [vec![0.; 5], vec![1.; 5]].concat(),
        [vec![0.; 5], vec![1.; 5]].concat(),
    )
    .unwrap();
    let wave = polynomial(5, &[0., 2., -2., 2., -2., 0.], false);
    for row in capture["results"].as_array().unwrap() {
        let name = row["id"].as_str().unwrap().split('-').next().unwrap();
        for mesh in row["value"]["meshes"].as_array().unwrap() {
            for point in mesh["samples"].as_array().unwrap() {
                let p = Point3::try_new(
                    point[0].as_f64().unwrap(),
                    point[1].as_f64().unwrap(),
                    point[2].as_f64().unwrap(),
                )
                .unwrap();
                let error = match name {
                    "sphere" => (p.x().hypot(p.y()).hypot(p.z()) - 2.).abs(),
                    "cylinder" => {
                        assert!(p.z() >= -1e-6 && p.z() <= 3. + 1e-6);
                        (p.x().hypot(p.y()) - 2.).abs()
                    }
                    "plane" => p.z().abs(),
                    "saddle" => {
                        let (u, v) = (p.x() / 3., p.y() / 2.);
                        (p.z() - (2. * v + u - 3. * u * v)).abs()
                    }
                    _ => {
                        let s = if name == "bump" { &bump } else { &wave };
                        let u = (p.x() / 3.).clamp(0., 1.);
                        let v = (p.y() / 2.).clamp(0., 1.);
                        p.distance_to(s.evaluate(u, v).unwrap()).unwrap()
                    }
                };
                assert!(error < 1e-6, "{} {p:?}: {error}", row["id"]);
            }
        }
    }
}

#[test]
fn rectangular_grids_stitch_periodic_profile_seams_without_adding_straight_rows() {
    let frame = Frame3::try_from_x_and_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cylinder = NurbsSurface::try_cylinder(frame, 2., 0., 3.).unwrap();
    for density in [0., 0.5, 1.] {
        let resolution = counts(&cylinder, density).unwrap();
        assert_eq!(resolution[1], 1);
        let mesh = cylinder
            .polygon_mesh(density, false, Tolerance::DEFAULT)
            .unwrap();
        let topology = mesh.topology();
        assert!(topology.is_manifold());
        assert_eq!(topology.boundary_edge_count(), 8 * resolution[0]);
        assert_eq!(mesh.faces().len(), 4 * resolution[0]);
        let swapped = cylinder.try_swapped_uv().unwrap();
        let mesh = swapped
            .polygon_mesh(density, false, Tolerance::DEFAULT)
            .unwrap();
        assert!(mesh.topology().is_manifold());
        assert_eq!(mesh.topology().boundary_edge_count(), 8 * resolution[0]);
    }
}
