use super::*;

mod segmented;

const LIMIT: Real = 1e-7;

#[test]
fn sweep1_section_refits_single_profile_miters_match_native_images() {
    compare_miters(true, 4, |actual, native, id| {
        let axis = usize::from(*native.domain_u().start() == 5.);
        let transverse = generator_error(actual, axis) + generator_error(native, axis);
        let bound = rim_bound(actual, native, id) + 2. * transverse;
        assert!(
            bound < LIMIT,
            "{id}: complete miter image bound {bound:.3e}"
        );
        bound
    });
}

#[test]
fn sweep1_section_refits_mixed_profile_miters_keep_native_charts_and_rims() {
    compare_miters(false, 5, |actual, native, id| {
        rim_bound(actual, native, id);
        // The interior U parameterization still has measured differences.
        let limit = 2e-5;
        actual
            .parameter_surface_deviation_bound(native, limit)
            .unwrap()
            .unwrap_or_else(|| panic!("{id}: complete miter correspondence exceeds {limit}"))
    });
}

#[test]
fn sweep1_shared_miter_keeps_both_native_crease_bands() {
    let (input, observed, _) = captures!("sweep1_shared_miter_crease", ());
    let query: ProbeRequest = serde_json::from_str(input).unwrap();
    let observation: Value = serde_json::from_str(observed).unwrap();
    let tolerance = Tolerance::try_new(LIMIT, 1e-12, 1e-6).unwrap();
    let mut compared = 0;
    let mut maximum: Real = 0.;
    for (op, record) in query
        .operations
        .into_iter()
        .zip(observation["results"].as_array().unwrap())
    {
        let Operation::Sweep1 { id, fixture } = op else {
            panic!("Sweep1 fixture expected");
        };
        assert_eq!(record["id"], id);
        let (value, _) = run(&fixture, 1, tolerance).unwrap();
        let actual = sorted_faces(&value);
        let native = sorted_faces(&record["value"]);
        assert_eq!(actual.len(), 4, "{id}");
        assert_eq!(native.len(), 4, "{id}");
        for (i, (actual, native)) in actual.iter().zip(&native).enumerate() {
            // Trimmed faces retain extended supports. Compare each complete
            // patch between its natural end and the common corner.
            let u = if i < 2 { 0. ..=5. } else { 5. ..=10. };
            let actual = actual.try_trimmed_u(u.clone()).unwrap();
            let native = native
                .try_trimmed_u(u.clone())
                .unwrap()
                .try_reparameterized(u, actual.domain_v())
                .unwrap();
            rim_bound(&actual, &native, &id);
            let limit = 2e-5;
            let bound = actual
                .parameter_surface_deviation_bound(&native, limit)
                .unwrap()
                .unwrap_or_else(|| panic!("{id}: natural patch correspondence exceeds {limit}"));
            maximum = maximum.max(bound);
        }
        compared += 1;
    }
    assert_eq!(compared, 4);
    println!("{compared} captures; maximum natural patch bound {maximum:.3e}");
}

fn compare_miters(
    single: bool,
    count: usize,
    compare: impl Fn(&NurbsSurface, &NurbsSurface, &str) -> Real,
) {
    let (input, observed, _) = captures!("sweep1_refit_miters", ());
    let query: ProbeRequest = serde_json::from_str(input).unwrap();
    let observation: Value = serde_json::from_str(observed).unwrap();
    let tolerance = Tolerance::try_new(LIMIT, 1e-12, 1e-10).unwrap();
    let mut compared = 0;
    let mut maximum: Real = 0.;
    for (op, record) in query
        .operations
        .into_iter()
        .zip(observation["results"].as_array().unwrap())
    {
        let Operation::Sweep1 { id, fixture } = op else {
            panic!("Sweep1 fixture expected");
        };
        if (fixture.sections.len() == 1) != single || fixture.untrimmed_miters != Some(true) {
            continue;
        }
        assert_eq!(record["id"], id);
        let (value, _) = run(&fixture, 1, tolerance).unwrap();
        let actual = sorted_faces(&value);
        let native = sorted_faces(&record["value"]);
        assert_eq!(actual.len(), native.len(), "{id}");
        for (actual, native) in actual.iter().zip(&native) {
            assert_same_chart(actual, native, &id);
            maximum = maximum.max(compare(actual, native, &id));
        }
        compared += 1;
    }
    assert_eq!(compared, count);
    println!("{compared} captures; maximum complete miter bound {maximum:.3e}");
}

fn rim_bound(actual: &NurbsSurface, native: &NurbsSurface, id: &str) -> Real {
    let [start, end] = [false, true].map(|last| {
        let edge = |s: &NurbsSurface| {
            let u = s.domain_u();
            s.isocurve_v(*if last { u.end() } else { u.start() })
                .unwrap()
        };
        edge(actual)
            .parameter_curve_deviation_bound(&edge(native), LIMIT / 2.)
            .unwrap()
            .unwrap_or_else(|| panic!("{id}: miter rim exceeds tolerance"))
    });
    start.max(end)
}

fn sorted_faces(records: &Value) -> Vec<NurbsSurface> {
    let mut faces = records
        .as_array()
        .unwrap()
        .iter()
        .map(|f| surface(&f["definition"]))
        .collect::<Vec<_>>();
    faces.sort_by(|a, b| {
        a.domain_u()
            .start()
            .total_cmp(b.domain_u().start())
            .then(a.domain_v().start().total_cmp(b.domain_v().start()))
    });
    faces
}

fn assert_same_chart(actual: &NurbsSurface, native: &NurbsSurface, id: &str) {
    assert_eq!(actual.degree_u(), native.degree_u(), "{id}");
    assert_eq!(actual.degree_v(), native.degree_v(), "{id}");
    for (a, b) in [
        (actual.knots_u(), native.knots_u()),
        (actual.knots_v(), native.knots_v()),
    ] {
        assert_eq!(a.len(), b.len(), "{id}: {a:?} {b:?}");
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < LIMIT),
            "{id}: {a:?} {b:?}"
        );
    }
}

// Positive weights that are constant along U make each fixed-V curve a
// monotone traversal between its two rim points in the rail direction.
// The convex hull bounds transverse drift from that straight segment.
fn generator_error(panel: &NurbsSurface, axis: usize) -> Real {
    let mut maximum: Real = 0.;
    for row in panel
        .control_points()
        .chunks_exact(panel.control_point_count_u())
    {
        let first = row[0].point().to_array();
        let mut previous = first[axis];
        for control in row {
            assert!(control.weight() > 0.);
            assert_eq!(control.weight(), row[0].weight());
            let point = control.point().to_array();
            assert!(point[axis] >= previous);
            previous = point[axis];
            let transverse = (point[1 - axis] - first[1 - axis]).hypot(point[2] - first[2]);
            maximum = maximum.max(transverse);
        }
    }
    maximum
}
