use super::*;

#[test]
fn options_reject_duplicate_values_and_resolve_model_units() {
    use viboceros_geometry::LengthUnitSystem;
    let input = parse(
        &["Distance=2cm", "w1,2,3", "Corner=Smooth", "BothSides"],
        Options::default(),
        &LengthUnitSystem::Millimeters,
        CommandContext::default(),
    )
    .unwrap();
    assert_eq!(input.options.distance, 20.);
    assert!(input.options.both_sides);
    assert!(!input.options.remembered().both_sides);
    assert_eq!(input.side.unwrap(), Point3::try_new(1., 2., 3.).unwrap());
    for args in [
        vec!["0"],
        vec!["-1"],
        vec!["nan"],
        vec!["Distance=1", "2"],
        vec!["Loose=Yes", "Loose=No"],
        vec!["Trim=Yes", "Trim=No"],
        vec!["OutputLayer=Target"],
        vec!["w0,0,0", "w1,2,3"],
        vec!["ThroughPoint=w0,2,0", "ThroughPoint"],
        vec!["ThroughPoint=w0,2,0", "w1,2,3"],
    ] {
        assert!(
            parse(
                &args,
                Options::default(),
                &LengthUnitSystem::Millimeters,
                CommandContext::default()
            )
            .is_err(),
            "{args:?}"
        );
    }
}

#[test]
fn ribbon_trim_preserves_native_strip_geometry_and_incidence() {
    let document: serde_json::Value =
        serde_json::from_str(include_str!("../../../../docs/ribbon-trim-reference.json")).unwrap();
    for case in document["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let closure = case["source"].as_str().unwrap();
        for (trim, field) in [(false, "untrimmed"), (true, "trimmed")] {
            // Global self-intersection trimming is separate from the inside
            // corner policy used in these regular Trim=Yes cases.
            if trim && matches!(name, "bow" | "dumbbell" | "u" | "lobed") {
                continue;
            }
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry.execute(&mut doc, closure).unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            let prefix = case["ribbon"].as_str().unwrap();
            let setting = if trim { "Yes" } else { "No" };
            let args = format!("{prefix} Trim={setting}");
            registry
                .execute(&mut doc, &args)
                .unwrap_or_else(|err| panic!("{name} {args}: {err}"));
            let b = doc
                .objects()
                .find_map(|object| match object.geometry() {
                    Geometry::Brep(brep) => Some(brep),
                    _ => None,
                })
                .unwrap();
            let expected = &case[field];
            for (actual, dimension) in [
                (b.faces().len(), "faces"),
                (b.edges().len(), "edges"),
                (b.vertices().len(), "vertices"),
            ] {
                assert_eq!(
                    actual,
                    expected[dimension].as_u64().unwrap() as usize,
                    "{name} {args} {dimension}"
                );
            }
            let area = b.area(doc.tolerance()).unwrap();
            assert!(
                (area - expected["area"].as_f64().unwrap()).abs() < 1e-9,
                "{name} {args} area={area}"
            );
            for (point, key) in [(b.bounds().min(), "min"), (b.bounds().max(), "max")] {
                let values = expected[key].as_array().unwrap();
                let expected =
                    Point3::try_from(std::array::from_fn(|i| values[i].as_f64().unwrap())).unwrap();
                assert!(
                    point.distance_to(expected).unwrap() < 1e-9,
                    "{name} {args} {key}={point:?}"
                );
            }
            assert_eq!(registry.ribbon_options().trim, trim);
        }
    }
}

#[test]
fn ribbon_trims_closed_sharp_offsets_and_preserves_native_ruled_sections() {
    verify_native_ruled_sections(include_str!(
        "../../../../docs/ribbon-global-trim-reference.json"
    ));
}

#[test]
fn ribbon_trims_open_inside_crossings_and_resolves_coincident_rails_by_pick() {
    verify_native_ruled_sections(include_str!(
        "../../../../docs/ribbon-open-trim-reference.json"
    ));
}

fn verify_native_ruled_sections(fixture: &str) {
    let reference: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let model = Tolerance::try_new(
        reference["model_absolute_tolerance"].as_f64().unwrap(),
        Tolerance::DEFAULT.relative(),
        Tolerance::DEFAULT.angular(),
    )
    .unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut doc = Document::new(model);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(&mut doc, case["source"].as_str().unwrap())
            .unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        doc.clear_history().unwrap();
        let result = registry.execute(&mut doc, case["ribbon"].as_str().unwrap());
        let expected = &case["expected"];
        if expected.is_null() {
            assert!(result.is_err(), "{name}");
            assert_eq!(doc.objects().len(), 1, "{name}");
            assert!(!doc.can_undo(), "{name}");
            continue;
        }
        result.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(doc.objects().len(), 2, "{name}");
        let brep = doc
            .objects()
            .find_map(|object| match object.geometry() {
                Geometry::Brep(brep) => Some(brep),
                _ => None,
            })
            .unwrap();
        let faces = expected["faces"].as_array().unwrap();
        let count = |name: &str| expected[name].as_u64().unwrap() as usize;
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (faces.len(), count("edges"), count("vertices")),
            "{name}"
        );
        let area = brep.area(model).unwrap();
        assert!(
            (area - expected["area"].as_f64().unwrap()).abs() < 1e-9,
            "{name} area={area}"
        );
        let mut surfaces = brep
            .faces()
            .iter()
            .map(|face| face.surface())
            .collect::<Vec<_>>();
        for face in faces {
            let value = |axis: usize, end: usize| face[0][axis][end].as_f64().unwrap();
            let expected_corners = face[1]
                .as_array()
                .unwrap()
                .iter()
                .map(json_point)
                .collect::<Vec<_>>();
            let matched = surfaces.iter().position(|surface| {
                let (u, v) = (surface.domain_u(), surface.domain_v());
                [*u.start(), *u.end(), *v.start(), *v.end()]
                    .into_iter()
                    .zip([value(0, 0), value(0, 1), value(1, 0), value(1, 1)])
                    .all(|(a, b)| (a - b).abs() < 1e-9)
                    && [
                        [*u.start(), *v.start()],
                        [*u.end(), *v.start()],
                        [*u.end(), *v.end()],
                        [*u.start(), *v.end()],
                    ]
                    .into_iter()
                    .zip(&expected_corners)
                    .all(|([u, v], &expected)| {
                        surface
                            .evaluate(u, v)
                            .and_then(|point| point.distance_to(expected))
                            .is_ok_and(|error| error < 1e-9)
                    })
            });
            let index =
                matched.unwrap_or_else(|| panic!("{name} missing native ruled section {face}"));
            surfaces.swap_remove(index);
        }
    }
}

#[test]
fn ribbon_corner_side_and_width_match_native_picks() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../../../../docs/ribbon-side-reference.json")).unwrap();
    let tolerance = Tolerance::try_new(
        reference["model_absolute_tolerance"].as_f64().unwrap(),
        Tolerance::DEFAULT.relative(),
        Tolerance::DEFAULT.angular(),
    )
    .unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        let [x, y, z] = json_point(&case["point"]).to_array();
        for (mode, args) in [
            ("fixed", format!("Ribbon .25 Trim=No w{x},{y},{z}")),
            (
                "through",
                format!("Ribbon Trim=No ThroughPoint=w{x},{y},{z}"),
            ),
        ] {
            let mut doc = Document::new(tolerance);
            let registry = CommandRegistry::with_builtins();
            registry.execute(&mut doc, source).unwrap();
            registry.execute(&mut doc, "SelAll").unwrap();
            doc.clear_history().unwrap();
            let output = registry.execute(&mut doc, &args);
            let expected = &case[mode];
            if expected.is_null() {
                assert!(output.is_err(), "{name} {args}");
                assert_eq!(doc.objects().len(), 1);
                assert!(!doc.can_undo());
                continue;
            }
            output.unwrap_or_else(|err| panic!("{name} {args}: {err}"));
            assert_eq!(doc.objects().len(), 2, "{name} {args}");
            let result = doc
                .objects()
                .find_map(|object| match object.geometry() {
                    Geometry::Brep(brep) => Some(brep),
                    _ => None,
                })
                .unwrap();
            let bounds = result.bounds();
            let width = expected["width"].as_f64().unwrap();
            let coordinate_tolerance = 1e-9_f64.min(width * 1e-7);
            assert_eq!(
                result.faces().len(),
                expected["faces"].as_u64().unwrap() as usize,
                "{name} {args}"
            );
            for (actual, key) in [(bounds.min(), "min"), (bounds.max(), "max")] {
                assert!(
                    actual.distance_to(json_point(&expected[key])).unwrap() < coordinate_tolerance,
                    "{name} {args} {key}={actual:?}"
                );
            }
            let area = result.area(doc.tolerance()).unwrap();
            let expected_area = expected["area"].as_f64().unwrap();
            assert!(
                (area - expected_area).abs() < 1e-9_f64.min(expected_area * 1e-7),
                "{name} {args} area={area}"
            );
            let remembered = registry.ribbon_options().distance;
            assert!(
                (remembered - width).abs() < coordinate_tolerance,
                "{name} {args} width={remembered}"
            );
        }
    }
}

fn json_point(values: &serde_json::Value) -> Point3 {
    Point3::try_from(std::array::from_fn(|i| values[i].as_f64().unwrap())).unwrap()
}

#[test]
fn ribbon_through_point_matches_rhino_signed_width_and_remembers_it() {
    let cases = [
        ("Line 0,0,0 6,0,0", "ThroughPoint=w9,-2,5", 2., -2., 0.),
        (
            "Line 0,0,0 6,0,0",
            "BothSides ThroughPoint=w2,-3,0",
            3.,
            -3.,
            3.,
        ),
        (
            "Line 0,0,0 6,0,6",
            "InCPlane=Yes ThroughPoint=w2,3,10",
            3.,
            0.,
            3.,
        ),
        (
            "Polyline 0,0,0 6,0,0 6,6,0",
            "Corner=Round ThroughPoint=w9,-1,0",
            3.,
            -3.,
            6.,
        ),
        (
            "Polyline 0,0,0 6,0,0 6,6,0",
            "Corner=Round ThroughPoint=w7,-3,0",
            3.,
            -3.,
            6.,
        ),
    ];
    for (source, args, width, min_y, max_y) in cases {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, source).unwrap();
        registry.execute(&mut doc, "SelAll").unwrap();
        registry
            .execute(&mut doc, &format!("Ribbon {args}"))
            .unwrap();
        let result = doc
            .objects()
            .find_map(|o| match o.geometry() {
                Geometry::Brep(brep) => Some(brep),
                _ => None,
            })
            .unwrap();
        assert!((result.bounds().min().y() - min_y).abs() < 1e-9, "{args}");
        assert!((result.bounds().max().y() - max_y).abs() < 1e-9, "{args}");
        assert!(
            (registry.ribbon_options().distance - width).abs() < 1e-9,
            "{args}"
        );
        assert!(!registry.ribbon_options().both_sides);
    }
}

#[test]
fn command_keeps_preselection_and_creates_unselected_output_in_one_undo_step() {
    let mut doc = Document::new(Tolerance::DEFAULT);
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 5,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let source = doc.selected_objects().next().unwrap().clone();
    registry.execute(&mut doc, "Ribbon .5 w0,2,0").unwrap();
    assert_eq!(doc.objects().len(), 2);
    assert_eq!(
        doc.selected_object_ids().collect::<Vec<_>>(),
        vec![source.id()]
    );
    let result = doc
        .objects()
        .find(|o| o.id() != source.id())
        .unwrap()
        .clone();
    assert!(matches!(result.geometry(), Geometry::Brep(_)));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 1);
    assert_eq!(
        doc.object(source.id()).unwrap().geometry(),
        source.geometry()
    );
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(
        doc.object(result.id()).unwrap().geometry(),
        result.geometry()
    );
}

#[test]
fn remembered_options_are_registry_local_and_failures_preserve_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut doc, "Line 0,0,0 5,0,0").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    registry
        .execute(
            &mut doc,
            "Ribbon 2 w0,5,0 BothSides=Yes OutputLayer=Input Corner=Round",
        )
        .unwrap();
    let accepted = registry.ribbon_options();
    assert_eq!(accepted.distance, 2.);
    assert!(accepted.input_layer);
    assert!(!accepted.both_sides);
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    for text in [
        "Ribbon 0 w0,5,0",
        "Ribbon nan w0,5,0",
        "Ribbon Distance=1 Distance=2 w0,5,0",
    ] {
        assert!(registry.execute(&mut doc, text).is_err());
        assert_eq!(format!("{doc:?}"), before);
        assert_eq!(registry.ribbon_options(), accepted);
    }
    assert!(doc.can_redo());
    assert_eq!(
        CommandRegistry::with_builtins().ribbon_options(),
        Options::default()
    );
}

#[test]
fn coarse_fit_tolerance_does_not_collapse_or_sew_distinct_boundaries() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Circle 0,0,0 3").unwrap();
    registry.execute(&mut doc, "SelAll").unwrap();
    let mut coarse = doc.clone();
    registry
        .execute(&mut doc, "Ribbon 1 w5,0,0 BothSides=Yes")
        .unwrap();
    registry
        .execute(&mut coarse, "Ribbon 1 w5,0,0 BothSides=Yes Tolerance=10")
        .unwrap();
    let result = |d: &Document| {
        d.objects()
            .find_map(|o| {
                if let Geometry::Brep(b) = o.geometry() {
                    Some(b.clone())
                } else {
                    None
                }
            })
            .unwrap()
    };
    let (a, b) = (result(&doc), result(&coarse));
    assert_eq!(a.faces().len(), 2);
    assert_eq!(a, b);
}
