use super::*;

#[test]
fn sweep1_staggered_miters_keep_the_native_extended_pieces() {
    let (input, observed, _) = captures!("sweep1_staggered_miters", ());
    let query: ProbeRequest = serde_json::from_str(input).unwrap();
    let observation: Value = serde_json::from_str(observed).unwrap();
    let tolerance = Tolerance::try_new(LIMIT, 1e-12, 1e-6).unwrap();
    let mut maximum: Real = 0.;
    let results = observation["results"].as_array().unwrap();
    assert_eq!(query.operations.len(), 2);
    assert_eq!(results.len(), 2);
    for (op, record) in query.operations.into_iter().zip(results) {
        let Operation::Sweep1 { id, fixture } = op else {
            panic!("Sweep1 fixture expected");
        };
        assert_eq!(record["id"], id);
        let rail = fixture.rail.geometry().unwrap();
        let sections = fixture
            .sections
            .iter()
            .zip(&fixture.parameters)
            .map(|(input, &parameter)| SweepSection {
                parameter,
                curve: input.geometry().unwrap().as_ref().to_nurbs().unwrap(),
            })
            .collect::<Vec<_>>();
        let actual = command_results(&rail, &sections, &fixture, tolerance, Clone::clone).unwrap();
        let faces = record["value"].as_array().unwrap();
        let edge_counts = faces
            .iter()
            .filter_map(|face| face["edges"].as_array().map(Vec::len));
        assert_eq!(edge_counts.collect::<Vec<_>>(), [12, 12], "{id}");
        assert_eq!(actual.len(), 2, "{id}");
        for piece in &actual {
            assert_eq!((piece.faces().len(), piece.edges().len()), (4, 12), "{id}");
        }
        let mut actual = actual
            .iter()
            .flat_map(|b| b.faces().iter().map(|f| f.surface().clone()))
            .collect::<Vec<_>>();
        let mut native = faces
            .iter()
            .map(|f| surface(&f["definition"]))
            .collect::<Vec<_>>();
        for patches in [&mut actual, &mut native] {
            patches.sort_by(|a, b| {
                let au = a.domain_u();
                let bu = b.domain_u();
                au.start()
                    .total_cmp(bu.start())
                    .then(au.end().total_cmp(bu.end()))
                    .then(a.domain_v().start().total_cmp(b.domain_v().start()))
            });
        }
        assert_eq!(actual.len(), native.len(), "{id}");
        for (a, b) in actual.iter().zip(&native) {
            let limit = 5e-5;
            let bound = a
                .parameter_surface_deviation_bound(b, limit)
                .unwrap()
                .unwrap_or_else(|| panic!("{id}: extended patch correspondence exceeds {limit}"));
            maximum = maximum.max(bound);
        }
    }
    println!("2 captures; maximum extended patch bound {maximum:.3e}");
}
