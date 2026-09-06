use super::*;

const REGULAR: &str = include_str!("../../../../tools/rhino_oracle/fixtures/distribute.json");
const DIAGNOSTICS: &str =
    include_str!("../../../../tools/rhino_oracle/fixtures/distribute_diagnostics.json");

#[test]
fn permanent_distribution_preserves_all_source_geometry_up_to_one_rigid_translation() {
    for (fixture, count) in [(REGULAR, 188), (DIAGNOSTICS, 6)] {
        let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
        let tolerance = request.tolerance.geometry().unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), count);
        for (operation, result) in request.operations.iter().zip(response.results) {
            let Operation::Distribute { id, fixture: f } = operation else {
                panic!("distribution")
            };
            assert_eq!(
                result.value["succeeded"],
                !matches!(id.as_str(), "all-group" | "two-units"),
                "{id}"
            );
            let records = result.value["objects"].as_array().unwrap();
            assert_eq!(records.len(), f.sources.len());
            let selected = f
                .selected
                .clone()
                .unwrap_or_else(|| (0..f.sources.len()).collect());
            for (i, record) in records.iter().enumerate() {
                assert_eq!(record["source"], i);
                assert_eq!(record["retained"], true);
                assert_eq!(record["current_layer"], true);
                assert_eq!(record["selected"], selected.contains(&i));
                let geometry = f.sources[i].geometry(tolerance).unwrap();
                let (domain, original) = sample(&geometry).unwrap();
                assert_eq!(record["domain"], domain);
                let points = record["points"].as_array().unwrap();
                assert_eq!(points.len(), original.len());
                let offset: [f64; 3] = std::array::from_fn(|axis| {
                    points[0][axis].as_f64().unwrap() - original[0][axis]
                });
                if !selected.contains(&i) || result.value["succeeded"] == false {
                    assert!(offset.iter().all(|x| x.abs() < 1e-12));
                }
                for (p, q) in points.iter().zip(original) {
                    for axis in 0..3 {
                        assert!(
                            (p[axis].as_f64().unwrap() - q[axis] - offset[axis]).abs() < 1e-8,
                            "{id}"
                        );
                    }
                }
            }
            let mut expected = f.groups.clone();
            for group in &mut expected {
                group.sort_unstable();
            }
            expected.sort();
            assert_eq!(result.value["groups"], json!(expected));
        }
    }
}

#[test]
fn curved_diagnostic_translations_match_closed_form_extrema() {
    let request: ProbeRequest = serde_json::from_str(DIAGNOSTICS).unwrap();
    let mut count = 0;
    for operation in &request.operations {
        let Operation::Distribute { id, fixture: f } = operation else {
            panic!("distribution")
        };
        if !id.starts_with("curved-") {
            continue;
        }
        let gap = f.mode == Mode::Gap;
        let expected = if id.starts_with("curved-oblique-disk-world-z") {
            // z = 4 + u/4 + v/2 - 2(u²+v²), u²+v² <= 0.8².
            let minimum = 4. - 2. * 0.8_f64.powi(2) - 0.8 * 5_f64.sqrt() / 4.;
            let maximum = 4. + 5. / 128.;
            [
                0.,
                0.,
                (if gap { 5. } else { 5.5 }) - (minimum + maximum) / 2.,
            ]
        } else {
            // Unnormalized projection direction is (1,2,3).
            let center = if id.starts_with("curved-surface-") {
                // f(u,v)=13u-12u²+26v-24v² on [0,1]².
                (169. / 16.) / 2.
            } else {
                // f(u,v)=u+2v+3(u²+v²), disk radius 0.8.
                (-5. / 12. + 3. * 0.8_f64.powi(2) + 0.8 * 5_f64.sqrt()) / 2.
            };
            let multiplier = ((if gap { 31. } else { 33. }) - center) / 14.;
            [multiplier, 2. * multiplier, 3. * multiplier]
        };
        let (value, _) = run(f, request.tolerance.geometry().unwrap()).unwrap();
        let (_, original) = sample(
            &f.sources[1]
                .geometry(request.tolerance.geometry().unwrap())
                .unwrap(),
        )
        .unwrap();
        for (p, q) in value["objects"][1]["points"]
            .as_array()
            .unwrap()
            .iter()
            .zip(original)
        {
            for axis in 0..3 {
                assert!(
                    (p[axis].as_f64().unwrap() - q[axis] - expected[axis]).abs() < 1e-8,
                    "{id}: {p:?}"
                );
            }
        }
        count += 1;
    }
    assert_eq!(count, 6);
}

#[test]
fn distribution_protocol_rejects_incomplete_preselection_bad_groups_and_bad_modes() {
    let request: Value = serde_json::from_str(REGULAR).unwrap();
    for (key, value) in [
        ("selected", json!([0, 1])),
        ("selected", json!([0, 0, 2])),
        ("groups", json!([[0, 3]])),
        ("sources", json!([])),
    ] {
        let mut f = request["operations"][0].clone();
        f[key] = value;
        let f: DistributeFixture = serde_json::from_value(f).unwrap();
        assert!(run(&f, Tolerance::DEFAULT).is_err());
    }
    let mut f = request["operations"][0].clone();
    f["mode"] = json!("Center _Delete");
    assert!(serde_json::from_value::<DistributeFixture>(f).is_err());
}
