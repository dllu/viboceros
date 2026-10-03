use super::*;
use serde_json::Value;
use viboceros_document::SelectionMode;
use viboceros_geometry::{MeshFace, WeightedPoint3};

fn p(v: &Value) -> Point3 {
    Point3::try_from(serde_json::from_value::<[Real; 3]>(v.clone()).unwrap()).unwrap()
}
fn mesh_colors(v: &Value) -> Vec<[u8; 4]> {
    let mut colors: Vec<[u8; 4]> = serde_json::from_value(v.clone()).unwrap();
    // RhinoCommon reports opacity; OpenNURBS/kernel colors store transparency.
    for c in &mut colors {
        c[3] = 255 - c[3];
    }
    colors
}
fn controls(v: &Value) -> Vec<WeightedPoint3> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|c| WeightedPoint3::try_new(p(&c["point"]), c["weight"].as_f64().unwrap()).unwrap())
        .collect()
}
fn curve(v: &Value) -> NurbsCurve {
    NurbsCurve::try_new_rational(
        v["degree"].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots"].clone()).unwrap(),
    )
    .unwrap()
}
fn surface(v: &Value) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        v["degree"][0].as_u64().unwrap() as usize,
        v["degree"][1].as_u64().unwrap() as usize,
        v["control_count"][0].as_u64().unwrap() as usize,
        v["control_count"][1].as_u64().unwrap() as usize,
        controls(&v["control_points"]),
        serde_json::from_value(v["knots_u"].clone()).unwrap(),
        serde_json::from_value(v["knots_v"].clone()).unwrap(),
    )
    .unwrap()
}
fn near(a: Point3, b: Point3, epsilon: Real, label: &str) {
    assert!(
        a.distance_to(b).unwrap() <= epsilon,
        "{label}: {a:?} != {b:?}"
    );
}
fn compare_controls(actual: &[WeightedPoint3], expected: &[WeightedPoint3], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, b) in actual.iter().zip(expected) {
        near(a.point(), b.point(), 1e-11, label);
        assert_eq!(a.weight(), b.weight(), "{label}");
    }
}
fn compare_knots(actual: &[Real], expected: &[Real], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() <= 1e-11, "{label}: knot {a} != {b}");
    }
}
fn compare_preserved_structure(
    actual: &Geometry,
    expected: &Value,
    source: &Geometry,
    label: &str,
) {
    if let Geometry::NurbsSurface(actual) = actual {
        let Geometry::NurbsSurface(source) = source else {
            panic!("expected a source NURBS surface");
        };
        assert_eq!(actual.degree_u(), source.degree_u(), "{label}");
        assert_eq!(actual.degree_v(), source.degree_v(), "{label}");
        assert_eq!(actual.knots_u(), source.knots_u(), "{label}");
        assert_eq!(actual.knots_v(), source.knots_v(), "{label}");
        let expected = surface(&expected["surfaces"][0]);
        assert_eq!(actual.degree_u(), expected.degree_u(), "{label}");
        assert_eq!(actual.degree_v(), expected.degree_v(), "{label}");
        assert_eq!(
            actual.control_point_count_u(),
            expected.control_point_count_u(),
            "{label}"
        );
        assert_eq!(
            actual.control_point_count_v(),
            expected.control_point_count_v(),
            "{label}"
        );
        compare_knots(actual.knots_u(), expected.knots_u(), label);
        compare_knots(actual.knots_v(), expected.knots_v(), label);
        compare_controls(actual.control_points(), expected.control_points(), label);
    } else if !expected["definition"].is_null() {
        let actual = actual.nurbs_curve_representation().unwrap().unwrap();
        let source = source.nurbs_curve_representation().unwrap().unwrap();
        assert_eq!(actual.degree(), source.degree(), "{label}");
        assert_eq!(actual.knots(), source.knots(), "{label}");
        let expected = curve(&expected["definition"]);
        assert_eq!(actual.degree(), expected.degree(), "{label}");
        // Independent line-to-NURBS conversions can round their length-domain
        // endpoints differently; retaining the Rust source's knots above is exact.
        compare_knots(actual.knots(), expected.knots(), label);
        compare_controls(actual.control_points(), expected.control_points(), label);
    }
}
fn axis(op: &Value) -> (Point3, Point3) {
    let (a, b) = match op["axis"].as_str().unwrap_or("Z") {
        "LongZ" => ([0., 0., 0.], [0., 0., 20.]),
        "Reverse" => ([0., 0., 10.], [0., 0., 0.]),
        "Spatial" => ([1., 2., 3.], [5., 6., 11.]),
        _ => ([0., 0., 0.], [0., 0., 10.]),
    };
    (Point3::try_from(a).unwrap(), Point3::try_from(b).unwrap())
}
fn setup(op: &Value, before: &Value) -> (Document, Vec<ObjectId>) {
    let tolerance =
        Tolerance::try_new(op["tolerance"].as_f64().unwrap_or(1e-5), 1e-12, 1e-9).unwrap();
    let mut doc = Document::new(tolerance);
    let mut ids = Vec::new();
    for (i, row) in before["objects"].as_array().unwrap().iter().enumerate() {
        let g = &row["geometry"];
        let geometry = match op["shape"].as_str().unwrap() {
            "Points" => Geometry::Point(p(&g["points"][0])),
            "Line" => Geometry::Line(
                LineSegment::try_new(p(&g["samples"][0]), p(&g["samples"][64]), tolerance).unwrap(),
            ),
            "Curve" => Geometry::NurbsCurve(curve(&g["definition"])),
            "Surface" => Geometry::NurbsSurface(surface(&g["surfaces"][0])),
            "Box" => Geometry::Brep(
                Brep::try_box(
                    CommandContext::default().construction_plane,
                    [[1., 3.], [-1., 1.], [0., 10.]],
                    tolerance,
                )
                .unwrap(),
            ),
            "Mesh" => Geometry::Mesh(
                TriangleMesh::try_new_faces(
                    g["points"].as_array().unwrap().iter().map(p).collect(),
                    vec![MeshFace::Quad([0, 1, 2, 3])],
                    tolerance,
                )
                .unwrap()
                .try_with_vertex_colors(Some(mesh_colors(&g["colors"])))
                .unwrap(),
            ),
            _ => unreachable!(),
        };
        let id = doc.add_geometry(geometry).unwrap();
        doc.set_object_names([(id, Some(format!("source-{i}")))])
            .unwrap();
        doc.set_objects_color(
            [id],
            Some(viboceros_document::ColorRgb::new(10 + i as u8, 30, 50)),
        )
        .unwrap();
        ids.push(id);
    }
    if op["grouped"].as_bool().unwrap() {
        doc.add_group(Some("TwistSource".into()), ids.iter().copied())
            .unwrap();
    }
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)
        .unwrap();
    (doc, ids)
}
fn compare_geometry(
    actual: &Geometry,
    expected: &Value,
    source: &Geometry,
    source_record: &Value,
    epsilon: Real,
    label: &str,
) {
    match actual {
        Geometry::Point(a) => near(*a, p(&expected["points"][0]), epsilon, label),
        Geometry::Mesh(mesh) => {
            assert_eq!(
                mesh.vertices().len(),
                expected["points"].as_array().unwrap().len(),
                "{label}"
            );
            for (a, b) in mesh
                .vertices()
                .iter()
                .zip(expected["points"].as_array().unwrap())
            {
                near(*a, p(b), epsilon.max(1e-6), label);
            }
            assert_eq!(
                mesh.vertex_colors().unwrap(),
                mesh_colors(&expected["colors"])
            );
            assert_eq!(mesh.faces(), [MeshFace::Quad([0, 1, 2, 3])]);
        }
        Geometry::NurbsSurface(s) => {
            let u = s.domain_u();
            let v = s.domain_v();
            for (i, b) in expected["samples"].as_array().unwrap().iter().enumerate() {
                let a = s
                    .evaluate(
                        u.start() + (u.end() - u.start()) * (i % 9) as Real / 8.,
                        v.start() + (v.end() - v.start()) * (i / 9) as Real / 8.,
                    )
                    .unwrap();
                near(a, p(b), epsilon, label);
            }
        }
        Geometry::Brep(brep) => {
            assert!(brep.is_closed());
            assert!(brep.is_solid());
            assert_eq!(
                brep.vertices().len(),
                expected["vertices"].as_array().unwrap().len(),
                "{label}"
            );
            assert_eq!(
                brep.faces().len(),
                expected["surfaces"].as_array().unwrap().len(),
                "{label}"
            );
            let Geometry::Brep(original) = source else {
                panic!();
            };
            for (face_index, native_source) in source_record["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                let native = surface(native_source);
                let sample = |surface: &NurbsSurface, u: Real, v: Real| {
                    let du = surface.domain_u();
                    let dv = surface.domain_v();
                    surface
                        .evaluate(
                            du.start() + (du.end() - du.start()) * u,
                            dv.start() + (dv.end() - dv.start()) * v,
                        )
                        .unwrap()
                };
                let mut correspondence = None;
                for (k, face) in original.faces().iter().enumerate() {
                    for swap in [false, true] {
                        for flip_u in [false, true] {
                            for flip_v in [false, true] {
                                let uv = |mut u: Real, mut v: Real| {
                                    if swap {
                                        std::mem::swap(&mut u, &mut v);
                                    }
                                    (
                                        if flip_u { 1. - u } else { u },
                                        if flip_v { 1. - v } else { v },
                                    )
                                };
                                if [[0., 0.], [1., 0.], [0., 1.], [1., 1.]].into_iter().all(
                                    |[u, v]| {
                                        let (a, b) = uv(u, v);
                                        sample(&native, u, v)
                                            .distance_to(sample(face.surface(), a, b))
                                            .unwrap()
                                            < 1e-10
                                    },
                                ) {
                                    correspondence = Some((k, swap, flip_u, flip_v));
                                }
                            }
                        }
                    }
                }
                let (k, swap, flip_u, flip_v) =
                    correspondence.expect("native box face has a matching source patch");
                for j in 0..9 {
                    for i in 0..9 {
                        let (mut u, mut v) = (i as Real / 8., j as Real / 8.);
                        if swap {
                            std::mem::swap(&mut u, &mut v);
                        }
                        if flip_u {
                            u = 1. - u;
                        }
                        if flip_v {
                            v = 1. - v;
                        }
                        near(
                            sample(brep.faces()[k].surface(), u, v),
                            p(&expected["samples"][face_index * 81 + j * 9 + i]),
                            epsilon,
                            label,
                        );
                    }
                }
            }
            for v in expected["vertices"].as_array().unwrap() {
                assert!(
                    brep.vertices()
                        .iter()
                        .any(|a| a.point().distance_to(p(v)).unwrap() < epsilon),
                    "{label}: vertex {v}"
                );
            }
        }
        _ => {
            let c = actual.nurbs_curve_representation().unwrap().unwrap();
            let domain = c.domain();
            for (i, b) in expected["samples"].as_array().unwrap().iter().enumerate() {
                near(
                    c.evaluate(domain.start() + (domain.end() - domain.start()) * i as Real / 64.)
                        .unwrap(),
                    p(b),
                    epsilon,
                    label,
                );
            }
        }
    }
}
#[test]
fn replay_actual_twist_commands_rigid_placements_and_fitting_tolerances() {
    let registry = CommandRegistry::with_builtins();
    for (fixtures, observations) in [
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_rigid_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_rigid_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_tight_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_tight_command.json"),
        ),
        (
            include_str!("../../../../tools/rhino_oracle/fixtures/twist_fitting_command.json"),
            include_str!("../../../../tools/rhino_oracle/observations/twist_fitting_command.json"),
        ),
    ] {
        let f: Value = serde_json::from_str(fixtures).unwrap();
        let o: Value = serde_json::from_str(observations).unwrap();
        assert_eq!(
            f["operations"].as_array().unwrap().len(),
            o["results"].as_array().unwrap().len()
        );
        for (op, row) in f["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(o["results"].as_array().unwrap())
        {
            let label = op["id"].as_str().unwrap();
            assert_eq!(op["id"], row["id"]);
            assert_eq!(row["value"]["success"], true);
            let (mut doc, ids) = setup(op, &row["value"]["before"]);
            let (start, end) = axis(op);
            let source_geometry = ids
                .iter()
                .map(|id| doc.object(*id).unwrap().geometry().clone())
                .collect::<Vec<_>>();
            let options = TwistOptions {
                copy: op["copy"].as_bool().unwrap(),
                rigid: op["rigid"].as_bool().unwrap(),
                infinite: op["infinite"].as_bool().unwrap(),
                preserve_structure: op["preserve"].as_bool().unwrap(),
            };
            let input = format!(
                "Twist {} {} {} {}",
                format_point(start),
                format_point(end),
                op["degrees"].as_f64().unwrap(),
                options.command_options()
            );
            registry
                .execute(&mut doc, &input)
                .unwrap_or_else(|e| panic!("{label}: {e}"));
            let expected = &row["value"]["after"];
            assert_eq!(
                doc.objects().count(),
                expected["objects"].as_array().unwrap().len(),
                "{label}"
            );
            let epsilon = if options.rigid {
                1e-7
            } else if op["shape"] == "Points"
                || (options.preserve_structure && op["shape"] != "Box")
            {
                1e-11
            } else {
                2. * op["tolerance"].as_f64().unwrap_or(1e-5).max(1e-5)
            };
            for (i, (actual, record)) in doc
                .objects()
                .zip(expected["objects"].as_array().unwrap())
                .enumerate()
            {
                compare_geometry(
                    actual.geometry(),
                    &record["geometry"],
                    &source_geometry[i % ids.len()],
                    &row["value"]["before"]["objects"][i % ids.len()]["geometry"],
                    epsilon,
                    label,
                );
                if options.preserve_structure && !options.rigid && op["shape"] != "Box" {
                    compare_preserved_structure(
                        actual.geometry(),
                        &record["geometry"],
                        &source_geometry[i % ids.len()],
                        label,
                    );
                }
                assert_eq!(
                    doc.is_selected(actual.id()),
                    record["selected"].as_bool().unwrap(),
                    "{label}"
                );
                assert_eq!(
                    actual.attributes().name(),
                    record["name"].as_str(),
                    "{label}"
                );
                assert_eq!(
                    actual.group_ids().len(),
                    record["groups"].as_array().unwrap().len(),
                    "{label}"
                );
            }
            assert_metadata(&doc, expected, label);
            if let Some(expected) = row["value"]["undo"].as_object() {
                registry.execute(&mut doc, "Undo").unwrap();
                assert_eq!(doc.objects().count(), ids.len(), "{label}");
                assert_eq!(
                    doc.groups().count(),
                    expected["groups"].as_array().unwrap().len(),
                    "{label}"
                );
                assert_metadata(&doc, &row["value"]["undo"], label);
                registry.execute(&mut doc, "Redo").unwrap();
                assert_metadata(&doc, &row["value"]["redo"], label);
                assert_eq!(
                    doc.objects().count(),
                    row["value"]["redo"]["objects"].as_array().unwrap().len(),
                    "{label}"
                );
            }
        }
    }
}
#[test]
fn twist_invalid_inputs_leave_geometry_and_history_unchanged() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    let before = doc.object(id).unwrap().geometry().clone();
    let depth = doc.undo_label().map(str::to_owned);
    for input in [
        "Twist 0,0,0 0,0,0 90",
        "Twist 0,0,0 0,0,10 NaN",
        "Twist 0,0,0 0,0,10 90 Copy=Maybe",
        "Twist 0,0,0 0,0,10 90 Rigid=Yes Rigid=No",
        "Twist 0,0,0 0,0,10 0,0,5 1,0,5",
    ] {
        assert!(registry.execute(&mut doc, input).is_err(), "{input}");
        assert_eq!(doc.object(id).unwrap().geometry(), &before);
        assert_eq!(doc.undo_label().map(str::to_owned), depth);
    }
    registry
        .execute(&mut doc, "Twist 0,0,0 0,0,10 1,0,0 0,1,0")
        .unwrap();
    near(
        match doc.object(id).unwrap().geometry() {
            Geometry::Point(p) => *p,
            _ => unreachable!(),
        },
        Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap(),
        1e-12,
        "reference angle",
    );
}

#[test]
fn twist_scripted_preferences_are_shared_across_documents_and_isolated_between_registries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    let id = doc
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
        .unwrap();
    doc.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(
            &mut doc,
            "Twist 0,0,0 0,0,10 0 Rigid=Yes Infinite=Yes PreserveStructure=Yes",
        )
        .unwrap();
    let expected = TwistOptions {
        rigid: true,
        infinite: true,
        preserve_structure: true,
        copy: false,
    };
    assert_eq!(registry.twist_options_default(), expected);
    for input in [
        "Twist 0,0,0 0,0,0 45 Rigid=No Infinite=No PreserveStructure=No",
        "Twist 0,0,0 0,0,10 NaN Rigid=No",
        "Twist 0,0,0 0,0,10 45 Rigid=Maybe",
    ] {
        assert!(registry.execute(&mut doc, input).is_err());
        assert_eq!(registry.twist_options_default(), expected);
        assert_eq!(registry.transform_scalar_default("Twist"), Some(0.));
    }
    registry.execute(&mut doc, "Twist 0,0,0 0,0,10 90").unwrap();
    near(
        match doc.object(id).unwrap().geometry() {
            Geometry::Point(p) => *p,
            _ => unreachable!(),
        },
        Point3::try_new(2_f64.sqrt() / 2., 3. * 2_f64.sqrt() / 2., 5.).unwrap(),
        1e-7,
        "remembered Infinite/Rigid",
    );
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(registry.twist_options_default(), expected);
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(registry.twist_options_default(), expected);
    let mut other = Document::default();
    let other_id = other
        .add_geometry(Geometry::Point(Point3::try_new(2., 1., 2.5).unwrap()))
        .unwrap();
    other
        .select_object(other_id, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(&mut other, "_-Twist 0,0,0 0,0,10 90")
        .unwrap();
    let Geometry::Point(point) = other.object(other_id).unwrap().geometry() else {
        panic!();
    };
    let angle = std::f64::consts::FRAC_PI_8;
    near(
        *point,
        Point3::try_new(
            2. * angle.cos() - angle.sin(),
            2. * angle.sin() + angle.cos(),
            2.5,
        )
        .unwrap(),
        1e-7,
        "new document Infinite",
    );
    assert_eq!(
        CommandRegistry::with_builtins().twist_options_default(),
        TwistOptions::default()
    );
}

#[test]
fn repeated_twist_copies_match_four_native_batches_and_share_one_history_entry() {
    let f: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/fixtures/twist_repeat_command.json"
    ))
    .unwrap();
    let o: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/twist_repeat_command.json"
    ))
    .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (op, row) in f["operations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(o["results"].as_array().unwrap())
    {
        let (mut doc, ids) = setup(op, &row["value"]["before"]);
        let mut group = doc.begin_history_group("Twist").unwrap();
        for degrees in std::iter::once(op["degrees"].as_f64().unwrap()).chain(
            op["angles"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap()),
        ) {
            registry
                .execute_in_history_group(
                    &mut doc,
                    &format!("Twist 0,0,0 0,0,10 {degrees} Copy=Yes"),
                    CommandContext::default(),
                    &mut group,
                )
                .unwrap();
        }
        let expected = &row["value"]["after"];
        assert_eq!(
            doc.objects().count(),
            expected["objects"].as_array().unwrap().len()
        );
        for (obj, record) in doc.objects().zip(expected["objects"].as_array().unwrap()) {
            let Geometry::Point(point) = obj.geometry() else {
                panic!();
            };
            near(
                *point,
                p(&record["geometry"]["points"][0]),
                1e-11,
                "repeated Twist",
            );
        }
        assert_eq!(
            doc.groups().count(),
            expected["groups"].as_array().unwrap().len()
        );
        assert_metadata(&doc, expected, "repeated Twist after");
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().count(), ids.len());
        assert_metadata(&doc, &row["value"]["undo"], "repeated Twist Undo");
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(
            doc.objects().count(),
            expected["objects"].as_array().unwrap().len()
        );
        assert_metadata(&doc, &row["value"]["redo"], "repeated Twist Redo");
    }
}

#[test]
fn failed_mixed_point_and_collapsing_mesh_twist_is_atomic() {
    let registry = CommandRegistry::with_builtins();
    for copy in [false, true] {
        let mut doc = Document::default();
        let point = doc
            .add_geometry(Geometry::Point(Point3::try_new(2., 1., 5.).unwrap()))
            .unwrap();
        let half = 0.5_f64.sqrt();
        let vertices = [[1., 0., 0.], [half, -half, 5.], [0., -1., 10.]]
            .into_iter()
            .map(|p| Point3::try_from(p).unwrap())
            .collect();
        let mesh = doc
            .add_geometry(Geometry::Mesh(
                TriangleMesh::try_new(vertices, vec![[0, 1, 2]], doc.tolerance()).unwrap(),
            ))
            .unwrap();
        let group = doc.add_group(None, [point, mesh]).unwrap();
        doc.select_objects_direct([point, mesh], SelectionMode::Replace)
            .unwrap();
        let before = doc
            .objects()
            .map(|o| {
                (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec(),
                )
            })
            .collect::<Vec<_>>();
        let label = doc.undo_label().map(str::to_owned);
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "Twist 0,0,0 0,0,10 90 Infinite=Yes Copy={}",
                        if copy { "Yes" } else { "No" }
                    )
                )
                .is_err()
        );
        assert_eq!(
            doc.objects()
                .map(|o| (
                    o.id(),
                    o.geometry().clone(),
                    o.attributes().clone(),
                    o.group_ids().to_vec()
                ))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(doc.undo_label(), label.as_deref());
        assert_eq!(
            doc.group(group)
                .unwrap()
                .members()
                .collect::<std::collections::BTreeSet<_>>(),
            [point, mesh]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        );
        assert_eq!(doc.selected_object_count(), 2);
        assert_eq!(registry.twist_options_default(), TwistOptions::default());
        assert_eq!(registry.transform_scalar_default("Twist"), None);
    }
}

fn assert_metadata(doc: &Document, expected: &Value, label: &str) {
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    let groups = doc
        .groups()
        .map(|g| {
            let mut members = g
                .members()
                .map(|id| ids.iter().position(|x| *x == id).unwrap())
                .collect::<Vec<_>>();
            members.sort_unstable();
            members
        })
        .collect::<Vec<_>>();
    let native_groups = expected["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| serde_json::from_value::<Vec<usize>>(g["members"].clone()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(groups, native_groups, "{label}");
    for (o, row) in doc.objects().zip(expected["objects"].as_array().unwrap()) {
        assert_eq!(o.attributes().name(), row["name"].as_str(), "{label}");
        assert_eq!(
            o.group_ids()
                .iter()
                .map(|id| doc.groups().position(|g| g.id() == *id).unwrap())
                .collect::<Vec<_>>(),
            serde_json::from_value::<Vec<usize>>(row["groups"].clone()).unwrap(),
            "{label}"
        );
        assert_eq!(
            doc.is_selected(o.id()),
            row["selected"].as_bool().unwrap(),
            "{label}"
        );
        let c = o.attributes().object_color();
        assert_eq!(
            [c.red, c.green, c.blue],
            serde_json::from_value::<[u8; 3]>(row["color"].clone()).unwrap(),
            "{label}"
        );
    }
}
