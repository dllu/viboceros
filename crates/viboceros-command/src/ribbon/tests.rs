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
            if name == "bow" || (trim && matches!(name, "dumbbell" | "u" | "lobed")) {
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
