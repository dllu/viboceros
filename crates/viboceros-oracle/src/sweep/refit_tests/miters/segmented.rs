use super::*;

#[test]
fn sweep1_segmented_miters_match_native_edges_and_face_counts() {
    let (input, observed, _) = captures!("sweep1_segmented_miters", ());
    let query: ProbeRequest = serde_json::from_str(input).unwrap();
    let observation: Value = serde_json::from_str(observed).unwrap();
    let tolerance = Tolerance::try_new(LIMIT, 1e-12, 1e-6).unwrap();
    let mut maximum: Real = 0.;
    assert_eq!(query.operations.len(), 2);
    for (op, record) in query
        .operations
        .into_iter()
        .zip(observation["results"].as_array().unwrap())
    {
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
        let [actual] = actual.as_slice() else {
            panic!("{id}: expected one joined BRep");
        };
        let faces = record["value"].as_array().unwrap();
        let native = faces
            .iter()
            .filter_map(|face| face["edges"].as_array())
            .collect::<Vec<_>>();
        let [native] = native.as_slice() else {
            panic!("{id}: Rhino did not produce one BRep");
        };
        assert_eq!(actual.faces().len(), faces.len(), "{id}");
        assert_eq!(actual.edges().len(), native.len(), "{id}");
        let native_miter = miter_edges(native.iter().filter(|edge| edge["face_count"] == 2).map(
            |edge| {
                let definition = serde_json::from_value(edge["definition"].clone()).unwrap();
                nurbs_curve_from_definition(&definition).unwrap()
            },
        ));
        let actual_miter = miter_edges(actual.edges().iter().map(|edge| edge.curve().clone()));
        assert_eq!(native_miter.len(), 2, "{id}");
        assert_eq!(actual_miter.len(), 2, "{id}");
        for (a, b) in native_miter.iter().zip(&actual_miter) {
            let mut pairs = vec![[0., 0.]];
            for i in 1..64 {
                let fraction = Real::from(i) / 64.;
                pairs.push([fraction, fraction_at_height(b, point_at(a, fraction).z())]);
            }
            pairs.push([1., 1.]);
            let limit = 2e-5;
            let bound = a
                .mapped_curve_deviation_bound(b, &pairs, limit)
                .unwrap()
                .unwrap_or_else(|| panic!("{id}: miter edge image exceeds {limit}"));
            maximum = maximum.max(bound);
        }
    }
    println!("2 captures; maximum complete miter edge bound {maximum:.3e}");
}

fn miter_edges(curves: impl Iterator<Item = NurbsCurve>) -> Vec<NurbsCurve> {
    let mut curves = curves
        .filter_map(|curve| {
            let a = point_at(&curve, 0.).to_array();
            let b = point_at(&curve, 1.).to_array();
            // Only the intersection edges on these right-angle fixtures
            // change all three coordinates between endpoints.
            a.iter().zip(b).all(|(&a, b)| (a - b).abs() > 0.1).then(|| {
                if a[2] < b[2] {
                    curve
                } else {
                    curve.reversed().unwrap()
                }
            })
        })
        .collect::<Vec<_>>();
    curves.sort_by(|a, b| point_at(a, 0.).z().total_cmp(&point_at(b, 0.).z()));
    curves
}

fn point_at(curve: &NurbsCurve, fraction: Real) -> Point3 {
    curve
        .evaluate(curve.parameter_at(fraction).unwrap())
        .unwrap()
}

// Matching elevation proposes a correspondence. The curve certificate
// verifies the intervals between these proposals.
fn fraction_at_height(curve: &NurbsCurve, z: Real) -> Real {
    let [mut lo, mut hi]: [Real; 2] = [0., 1.];
    for _ in 0..48 {
        let t = lo.midpoint(hi);
        if point_at(curve, t).z() < z {
            lo = t;
        } else {
            hi = t;
        }
    }
    lo.midpoint(hi)
}
