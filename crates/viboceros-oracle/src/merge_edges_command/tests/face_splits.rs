use super::*;

#[test]
fn command_replays_kinked_surface_replacement_with_one_exact_cutoff_difference() {
    replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/merge_edges_kinky_surfaces.json"),
        include_str!(
            "../../../../../tools/rhino_oracle/observations/merge_edges_kinky_surfaces.json"
        ),
        64,
        &["kink-2-doc-0.0872665"],
    );
}

#[test]
fn command_replays_surface_cutoff_layout_selection_and_history() {
    replay(
        include_str!("../../../../../tools/rhino_oracle/fixtures/merge_edges_face_splits.json"),
        include_str!("../../../../../tools/rhino_oracle/observations/merge_edges_face_splits.json"),
        107,
        &["crease-2-doc0.0872665", "upper-2-doc2.5", "upper-2-doc10"],
    );
}

fn replay(fixture: &str, observations: &str, count: usize, cutoffs: &[&str]) {
    let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
    let expected: Value = serde_json::from_str(observations).unwrap();
    let actual = run_request(&request).unwrap();
    assert_eq!(actual.results.len(), count);
    assert_eq!(expected["results"].as_array().unwrap().len(), count);
    let mut exceptions = 0;
    for ((a, b), op) in actual
        .results
        .iter()
        .zip(expected["results"].as_array().unwrap())
        .zip(&request.operations)
    {
        assert_eq!(a.id, b["id"]);
        if !cutoffs.contains(&a.id.as_str()) {
            close(&a.value, &b["value"], &a.id);
            continue;
        }
        exceptions += 1;
        let Operation::MergeEdgesCommand { fixture, .. } = op else {
            panic!()
        };
        let Source::Brep { brep } = &fixture.sources[0] else {
            panic!()
        };
        let tolerance =
            Tolerance::try_new(1e-9, 1e-12, fixture.angular_tolerance.unwrap()).unwrap();
        let source = brep.build(Tolerance::DEFAULT).unwrap();
        let cleaned = source
            .try_cleanup_edges(1_f64.to_radians(), tolerance)
            .unwrap();
        let expected_native =
            crate::brep_join::geometry_record(&cleaned, Tolerance::DEFAULT).unwrap();
        assert_eq!(cleaned.faces().len(), 1);
        assert_eq!(cleaned.edges().len(), 5);
        let observed = &b["value"]["after"][0]["geometry"]["brep"];
        assert_eq!(observed["faces"].as_array().unwrap().len(), 2);
        assert_eq!(observed["edges"].as_array().unwrap().len(), 7);
        close(
            &a.value["after"][0]["geometry"]["brep"],
            &expected_native,
            &a.id,
        );
        // Exclude only the asserted cutoff geometry, not selection, attributes,
        // history, success, or any untouched source data. Raw records stay intact.
        let mut observed = b["value"].clone();
        observed["after"][0]["geometry"]["brep"] = expected_native;
        close(&a.value, &observed, &a.id);
    }
    assert_eq!(exceptions, cutoffs.len());
}

#[test]
fn partition_kernel_replays_retained_face_split_geometry() {
    let request: ProbeRequest = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/fixtures/merge_edges_face_splits.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/merge_edges_face_splits.json"
    ))
    .unwrap();
    assert_eq!(request.operations.len(), 107);
    assert_eq!(expected["results"].as_array().unwrap().len(), 107);
    let mut cutoff_differences = 0;
    for (op, row) in request
        .operations
        .iter()
        .zip(expected["results"].as_array().unwrap())
    {
        let Operation::MergeEdgesCommand { fixture, .. } = op else {
            panic!()
        };
        let Source::Brep { brep } = &fixture.sources[0] else {
            panic!()
        };
        let tolerance =
            Tolerance::try_new(1e-9, 1e-12, fixture.angular_tolerance.unwrap()).unwrap();
        let source = brep.build(Tolerance::DEFAULT).unwrap();
        let cleaned = source
            .try_cleanup_edges(
                tolerance
                    .angular()
                    .clamp(0.1_f64.to_radians(), 1_f64.to_radians()),
                tolerance,
            )
            .unwrap();
        let partitioned = cleaned
            .try_split_kinky_faces(
                tolerance
                    .angular()
                    .clamp(0.1_f64.to_radians(), 2_f64.to_radians()),
                tolerance,
            )
            .unwrap_or_else(|error| panic!("{}: {error}", row["id"]))
            .unwrap_or(cleaned);
        let actual = crate::brep_join::geometry_record(&partitioned, Tolerance::DEFAULT).unwrap();
        let observed = row["value"]["after"][0]["geometry"]["brep"].clone();
        if ["crease-2-doc0.0872665", "upper-2-doc2.5", "upper-2-doc10"]
            .contains(&row["id"].as_str().unwrap())
        {
            cutoff_differences += 1;
            assert_eq!(partitioned.faces().len(), 1);
            assert_eq!(partitioned.edges().len(), 5);
            assert_eq!(observed["faces"].as_array().unwrap().len(), 2);
            assert_eq!(observed["edges"].as_array().unwrap().len(), 7);
            let surface = source.faces()[0].surface();
            let profile = surface.isocurve_u(*surface.domain_v().start()).unwrap();
            let a = profile
                .tangent_at_on_side(1., viboceros_geometry::ParameterSide::Left)
                .unwrap();
            let b = profile
                .tangent_at_on_side(1., viboceros_geometry::ParameterSide::Right)
                .unwrap();
            let angle = a
                .as_vector()
                .cross(b.as_vector())
                .unwrap()
                .length()
                .unwrap()
                .atan2(a.as_vector().dot(b.as_vector()).unwrap());
            let cutoff = 2_f64.to_radians();
            assert!(angle <= cutoff && cutoff - angle < 1e-14);
            // Stable atan2 does not exceed the requested cutoff here; Rhino's
            // cosine comparison splits. Keep the raw disagreement explicit.
            continue;
        }
        // Compare the complete raw geometry record, including face/trim order
        // and all native parameter intervals; no representation normalization.
        close(&actual, &observed, row["id"].as_str().unwrap());
    }
    assert_eq!(cutoff_differences, 3);
}
