use super::*;
use crate::{MeshFace, NurbsCurve, NurbsSurface, Tolerance, TriangleMesh, Vector3, WeightedPoint3};
use serde_json::Value;

fn point(value: &Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| value[i].as_f64().unwrap())).unwrap()
}

fn reals(value: &Value) -> Vec<Real> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}

fn controls(value: &Value) -> Vec<WeightedPoint3> {
    value["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            WeightedPoint3::try_new(point(&p["point"]), p["weight"].as_f64().unwrap()).unwrap()
        })
        .collect()
}

fn frame(value: Option<&Value>) -> Frame3 {
    let (origin, x, y) = value.map_or(([0.; 3], [1., 0., 0.], [0., 1., 0.]), |v| {
        (
            point(&v["origin"]).to_array(),
            point(&v["x_axis"]).to_array(),
            point(&v["y_axis"]).to_array(),
        )
    });
    Frame3::try_from_directions(
        Point3::try_from(origin).unwrap(),
        Vector3::try_from(x).unwrap(),
        Vector3::try_from(y).unwrap(),
        Tolerance::default(),
    )
    .unwrap()
}

fn settings(mode: &str) -> SmoothingOptions {
    SmoothingOptions {
        factor: match mode {
            "steps" => 0.25,
            "negative" => -0.3,
            "overshoot" => 1.2,
            "zero" => 0.,
            _ => 0.2,
        },
        steps: if mode == "steps" { 3 } else { 1 },
        axes: match mode {
            "cplane" | "x" => [true, false, false],
            "none" => [false; 3],
            _ => [true; 3],
        },
        fix_boundaries: matches!(mode, "defaults" | "zero"),
    }
}

fn check_points(
    actual: impl IntoIterator<Item = Point3>,
    expected: &Value,
    id: &str,
    weighted: bool,
) -> Real {
    let expected = expected.as_array().unwrap();
    let actual = actual.into_iter().collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len(), "{id}");
    let mut largest: Real = 0.;
    for (i, (a, b)) in actual.into_iter().zip(expected).enumerate() {
        let b = point(if weighted { &b["point"] } else { b });
        for (a, b) in a.to_array().into_iter().zip(b.to_array()) {
            let error = (a - b).abs();
            largest = largest.max(error);
            assert!(error <= 2e-12, "{id} control {i}: {a} != {b} ({error:e})");
        }
    }
    largest
}

fn replay(q: &Value, r: &Value) {
    assert_eq!(r["engine"], "rhino");
    assert_eq!(r["engine_version"], "8.32.26160.13001");
    let mut largest: Real = 0.;
    assert_eq!(
        q["operations"].as_array().unwrap().len(),
        r["results"].as_array().unwrap().len()
    );
    for (op, row) in q["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(r["results"].as_array().unwrap())
    {
        assert_eq!(op["id"], row["id"]);
        let id = op["id"].as_str().unwrap();
        let value = &row["value"];
        assert_eq!(value["success"], true, "{id}");
        let before = &value["before"][0];
        let after = &value["after"][0];
        let options = settings(op["mode"].as_str().unwrap());
        let frame = frame(if op["mode"] == "cplane" {
            Some(&value["plane"])
        } else {
            None
        });
        let picks: Option<BTreeSet<usize>> = matches!(
            op["selection"].as_str().unwrap(),
            "grips" | "parent" | "allgrips"
        )
        .then(|| {
            before["grips"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|g| g["selected"] == true)
                .map(|g| g["index"].as_u64().unwrap() as usize)
                .collect()
        });
        if let Some(b) = before.get("curve") {
            let source = NurbsCurve::try_new_rational(
                b["degree"].as_u64().unwrap() as usize,
                controls(b),
                reals(&b["knots"]),
            )
            .unwrap();
            let result = source.try_smoothed(options, frame, picks.as_ref()).unwrap();
            let a = &after["curve"];
            assert_eq!(result.degree(), source.degree(), "{id}");
            assert_eq!(result.knots(), reals(&a["knots"]), "{id}");
            assert_eq!(
                [*result.domain().start(), *result.domain().end()],
                point_domain(&a["domain"]),
                "{id}"
            );
            assert_eq!(
                result
                    .control_points()
                    .iter()
                    .map(|p| p.weight())
                    .collect::<Vec<_>>(),
                controls(a).iter().map(|p| p.weight()).collect::<Vec<_>>(),
                "{id}"
            );
            largest = largest.max(check_points(
                result.control_points().iter().map(|p| p.point()),
                &a["control_points"],
                id,
                true,
            ));
        } else if let Some(b) = before.get("surface") {
            let source = NurbsSurface::try_new_rational(
                b["degree"][0].as_u64().unwrap() as usize,
                b["degree"][1].as_u64().unwrap() as usize,
                b["control_count"][0].as_u64().unwrap() as usize,
                b["control_count"][1].as_u64().unwrap() as usize,
                controls(b),
                reals(&b["knots_u"]),
                reals(&b["knots_v"]),
            )
            .unwrap();
            let result = source.try_smoothed(options, frame, picks.as_ref()).unwrap();
            let a = &after["surface"];
            assert_eq!(result.degree_u(), source.degree_u());
            assert_eq!(result.degree_v(), source.degree_v());
            assert_eq!(
                result.control_point_count_u(),
                source.control_point_count_u()
            );
            assert_eq!(
                result.control_point_count_v(),
                source.control_point_count_v()
            );
            assert_eq!(result.knots_u(), reals(&a["knots_u"]), "{id}");
            assert_eq!(result.knots_v(), reals(&a["knots_v"]), "{id}");
            assert_eq!(
                [*result.domain_u().start(), *result.domain_u().end()],
                point_domain(&a["domain_u"])
            );
            assert_eq!(
                [*result.domain_v().start(), *result.domain_v().end()],
                point_domain(&a["domain_v"])
            );
            assert_eq!(
                result
                    .control_points()
                    .iter()
                    .map(|p| p.weight())
                    .collect::<Vec<_>>(),
                controls(a).iter().map(|p| p.weight()).collect::<Vec<_>>(),
                "{id}"
            );
            largest = largest.max(check_points(
                result.control_points().iter().map(|p| p.point()),
                &a["control_points"],
                id,
                true,
            ));
        } else {
            let b = &before["mesh"];
            let faces = b["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|face| {
                    let indices = face
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| i.as_u64().unwrap() as u32)
                        .collect::<Vec<_>>();
                    match *indices.as_slice() {
                        [a, b, c] => MeshFace::Triangle([a, b, c]),
                        [a, b, c, d] => MeshFace::Quad([a, b, c, d]),
                        _ => panic!("face"),
                    }
                })
                .collect();
            let source = TriangleMesh::try_from_face_records(
                b["vertices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(point)
                    .collect(),
                faces,
            )
            .unwrap();
            let result = source.try_smoothed(options, frame, picks.as_ref()).unwrap();
            assert_eq!(result.faces(), source.faces(), "{id}");
            assert_eq!(b["faces"], after["mesh"]["faces"], "{id}");
            largest = largest.max(check_points(
                result.vertices().iter().copied(),
                &after["mesh"]["vertices"],
                id,
                false,
            ));
        }
    }
    eprintln!("Smooth fixed-coordinate largest control error: {largest:e}");
}

fn point_domain(value: &Value) -> [Real; 2] {
    [value[0].as_f64().unwrap(), value[1].as_f64().unwrap()]
}

#[test]
fn smooth_kernel_replays_native_fixed_coordinates() {
    let q = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/smooth_fixed.json"
    ))
    .unwrap();
    let r = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/smooth_fixed.json"
    ))
    .unwrap();
    replay(&q, &r);
}

#[test]
fn smooth_kernel_replays_native_selected_boundaries_and_iterations() {
    let q = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/smooth_selected.json"
    ))
    .unwrap();
    let r = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/smooth_selected.json"
    ))
    .unwrap();
    replay(&q, &r);
}

#[test]
fn smoothing_retains_collapsed_mesh_records_colors_ngons_and_unused_vertices() {
    let source = TriangleMesh::try_from_face_records(
        [
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
            [9., 9., 9.],
        ]
        .map(|p| Point3::try_from(p).unwrap())
        .to_vec(),
        vec![MeshFace::Quad([0, 1, 2, 3])],
    )
    .unwrap()
    .try_with_vertex_colors(Some(vec![[1, 2, 3, 0]; 5]))
    .unwrap()
    .try_with_ngons(vec![crate::MeshNgon::from_parts(vec![0, 1, 2, 3], vec![0])])
    .unwrap();
    let options = SmoothingOptions {
        factor: 1.,
        fix_boundaries: false,
        ..Default::default()
    };
    let collapsed = source.try_smoothed(options, frame(None), None).unwrap();
    assert!(
        collapsed.vertices()[..4]
            .iter()
            .all(|p| p.to_array() == [1., 1., 0.])
    );
    assert_eq!(collapsed.vertices()[4], source.vertices()[4]);
    assert_eq!(collapsed.faces(), source.faces());
    assert_eq!(collapsed.triangles(), source.triangles());
    assert_eq!(collapsed.vertex_colors(), source.vertex_colors());
    assert_eq!(collapsed.ngons(), source.ngons());
    assert!(
        collapsed
            .validate_face_geometry(Tolerance::default())
            .is_err()
    );
}

#[test]
fn invalid_smoothing_is_atomic_and_validates_empty_selections() {
    let source = NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(-Real::MAX, 0., 0.).unwrap(),
            Point3::try_new(Real::MAX, 0., 0.).unwrap(),
        ],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let original = source.clone();
    let options = SmoothingOptions {
        factor: Real::MAX,
        fix_boundaries: false,
        ..Default::default()
    };
    assert!(source.try_smoothed(options, frame(None), None).is_err());
    assert_eq!(source, original);
    for options in [
        SmoothingOptions {
            factor: Real::NAN,
            ..Default::default()
        },
        SmoothingOptions {
            steps: 0,
            ..Default::default()
        },
    ] {
        assert!(
            source
                .try_smoothed(options, frame(None), Some(&BTreeSet::new()))
                .is_err()
        );
    }
    assert_eq!(
        source.try_smoothed(
            SmoothingOptions::default(),
            frame(None),
            Some(&BTreeSet::from([2]))
        ),
        Err(GeometryError::InvalidControlPointIndex { index: 2, count: 2 })
    );
}

#[test]
fn means_preserve_extreme_ranges_subnormal_rounding_and_cancellation() {
    let tiny = Real::from_bits(1);
    for (values, expected) in [
        (vec![Real::MAX; 4], Real::MAX),
        (vec![Real::MAX, Real::MAX, -Real::MAX], Real::MAX / 3.),
        (vec![Real::MAX, tiny, -Real::MAX, tiny], tiny * 0.5),
        (vec![tiny, tiny, 0.], tiny),
        (vec![1., Real::EPSILON / 2., -1.], Real::EPSILON / 6.),
    ] {
        let points = values
            .iter()
            .map(|&v| Point3::try_new(v, 0., 0.).unwrap())
            .collect::<Vec<_>>();
        let indices = (0..points.len()).collect::<Vec<_>>();
        assert_eq!(
            mean(&points, &indices).unwrap().x().to_bits(),
            expected.to_bits(),
            "{values:?}"
        );
    }
    assert_eq!(mean(&[], &[]), Err(GeometryError::EmptyPointSet));
}

#[test]
fn projection_avoids_displacement_overflow_and_premature_underflow() {
    let world = frame(None);
    let tiny = Real::from_bits(1);
    let oblique = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 1., 0.).unwrap(),
        Vector3::try_new(-1., 1., 2.).unwrap(),
        Tolerance::default(),
    )
    .unwrap();
    for (p, t, f, axes, frame) in [
        (
            [-Real::MAX, 0., 0.],
            [Real::MAX, 0., 0.],
            0.5,
            [true; 3],
            world,
        ),
        (
            [Real::MAX, 0., 0.],
            [-Real::MAX, 0., 0.],
            0.25,
            [true; 3],
            world,
        ),
        ([Real::MAX, 0., 0.], [tiny, 0., 0.], 1., [true; 3], world),
        ([0.; 3], [tiny, 0., 0.], 4., [true, false, false], oblique),
        (
            [1., 2., 3.],
            [4., 5., 6.],
            -0.3,
            [false, true, true],
            oblique,
        ),
    ] {
        let directions = frame.axes().map(|a| a.as_vector().to_array());
        let matrix: [[Real; 3]; 3] = if axes == [true; 3] {
            std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1. } else { 0. }))
        } else {
            std::array::from_fn(|r| {
                std::array::from_fn(|c| {
                    (0..3)
                        .filter(|&i| axes[i])
                        .map(|i| directions[i][r] * directions[i][c])
                        .sum()
                })
            })
        };
        let expected = std::array::from_fn::<_, 3, _>(|r| {
            let displacement = (0..3)
                .map(|c| {
                    crate::exact_scalar::rational(matrix[r][c])
                        * (crate::exact_scalar::rational(t[c])
                            - crate::exact_scalar::rational(p[c]))
                })
                .sum::<crate::exact_scalar::Rational>();
            crate::exact_scalar::scalar(
                &(crate::exact_scalar::rational(p[r])
                    + crate::exact_scalar::rational(f) * displacement),
            )
            .unwrap()
        });
        let actual = projected_step(
            Point3::try_from(p).unwrap(),
            Point3::try_from(t).unwrap(),
            f,
            matrix,
        )
        .unwrap();
        for (a, b) in actual.to_array().into_iter().zip(expected) {
            assert!(
                (a - b).abs() <= b.abs() * Real::EPSILON * 4. || a.to_bits() == b.to_bits(),
                "{p:?} {t:?} {f}: {a} != {b}"
            );
        }
    }
}
