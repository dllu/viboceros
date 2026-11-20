use super::*;
use viboceros_geometry::Real;

fn surface(definition: &Value) -> NurbsSurface {
    let dims = |name: &str| serde_json::from_value::<[usize; 2]>(definition[name].clone()).unwrap();
    let [degree_u, degree_v] = dims("degree");
    let [count_u, count_v] = dims("control_count");
    let knots = |name: &str| serde_json::from_value::<Vec<Real>>(definition[name].clone()).unwrap();
    let controls = definition["control_points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|control| {
            let point = serde_json::from_value::<[Real; 3]>(control["point"].clone()).unwrap();
            WeightedPoint3::try_new(
                Point3::try_from(point).unwrap(),
                control["weight"].as_f64().unwrap(),
            )
            .unwrap()
        })
        .collect();
    NurbsSurface::try_new_rational(
        degree_u,
        degree_v,
        count_u,
        count_v,
        controls,
        knots("knots_u"),
        knots("knots_v"),
    )
    .unwrap()
}

macro_rules! captures {
    ($family:literal, $cases:expr) => {
        (
            include_str!(concat!(
                "../../../../tools/rhino_oracle/fixtures/",
                $family,
                ".json"
            )),
            include_str!(concat!(
                "../../../../tools/rhino_oracle/observations/",
                $family,
                ".json"
            )),
            $cases,
        )
    };
}

#[test]
fn sweep1_section_refits_closed_match_native_control_nets_and_complete_surfaces() {
    compare_captures(
        [
            captures!(
                "sweep1_refit_closed",
                &[
                    "circle-tight",
                    "circle-second-pass",
                    "rational-circle-tight",
                    "rotated-circle-tight",
                    "unequal-circles-tight",
                    "cubic-loop-command",
                ][..]
            ),
            captures!(
                "sweep1_refit_sdk_holdouts",
                &["closed-cubic-0.01", "closed-cubic-0.001"][..]
            ),
            captures!("sweep1_refit_commands", &["two-closed-refit-0.001"][..]),
            captures!("sweep1_refit_sdk", &["two-closed-refit-0.001"][..]),
        ],
        10,
    );
}

#[test]
fn sweep1_section_refits_match_native_control_nets_and_complete_surfaces() {
    let families = [
        captures!(
            "sweep1_refit_commands",
            &[
                "open-refit-0.001",
                "open-refit-0.01",
                "open-refit-0.1",
                "closed-refit-0.001",
                "closed-refit-0.01",
                "closed-refit-0.1",
                "polyline-refit-0.001",
                "polyline-refit-0.01",
                "rational-refit-0.001",
                "rational-refit-0.01",
                "two-open-refit-0.01",
                "two-open-refit-0.001",
                "two-closed-refit-0.01",
            ][..]
        ),
        captures!(
            "sweep1_refit_sdk",
            &[
                "open-refit-0.0",
                "open-refit-1e-09",
                "open-refit-0.001",
                "open-refit-0.1",
                "closed-refit-0.0",
                "closed-refit-0.001",
                "rational-refit-0.001",
                "rational-refit-0.01",
                "polyline-refit-0.01",
                "two-open-refit-0.01",
                "two-open-refit-0.001",
                "two-closed-refit-0.01",
            ][..]
        ),
        captures!("sweep1_refit_sdk_holdouts", &["nurbs-circle-0.001"][..]),
        captures!(
            "sweep1_refit_command_holdouts",
            &["open-end", "closed-end", "closed-both", "retained-arc"][..]
        ),
        captures!(
            "sweep1_refit_common_handles",
            &[
                "common-0.9-1-0.01",
                "common-0.9-1-0.001",
                "common-0.97-1-0.001",
                "common-0.97-1-0.01",
                "common-0.99-1-0.001",
                "common-0.99-1-0.01",
                "common-0.99-2-0.01",
                "common-0.99-2-0.001",
                "common-0.9-2-0.01",
                "common-0.995-1-0.001",
                "common-0.995-1-0.01",
                "common-1.0-1-0.001",
                "common-1.0-1-0.01",
                "common-1.01-1-0.001",
                "common-1.01-1-0.01",
            ][..]
        ),
        captures!(
            "sweep1_refit_contexts",
            &[
                "single-scale-0.5-0.001",
                "equal-pair-scale-0.5-0.001",
                "single-scale-0.5-0.01",
                "equal-pair-scale-0.5-0.01",
                "single-scale-2.0-0.01",
                "single-scale-2.0-0.001",
                "equal-pair-scale-2.0-0.01",
                "equal-pair-scale-1.0-0.001",
                "equal-pair-scale-10.0-0.01",
                "single-scale-10.0-0.01",
                "single-scale-10.0-0.001",
                "domain-0.001",
                "elevated-0.001",
                "reversed-0.001",
                "translated-0.001",
            ][..]
        ),
    ];
    compare_captures(families, 60);
}

fn compare_captures<const N: usize>(families: [(&str, &str, &[&str]); N], count: usize) {
    let limit = 1e-7;
    let mut maximum: Real = 0.;
    let mut compared = 0;
    for (input, observed, cases) in families {
        let mut query: Value = serde_json::from_str(input).unwrap();
        let observation: Value = serde_json::from_str(observed).unwrap();
        query["operations"]
            .as_array_mut()
            .unwrap()
            .retain(|op| cases.contains(&op["id"].as_str().unwrap()));
        let query: ProbeRequest = serde_json::from_value(query).unwrap();
        assert_eq!(query.operations.len(), cases.len());
        for op in query.operations {
            let Operation::Sweep1 { id, mut fixture } = op else {
                panic!("Sweep1 fixture expected");
            };
            if fixture.command && id == "closed-refit-0.001" {
                // A separate Rhino command run with RefitRail=No retains
                // the same cubic rail chart for the circular section.
                fixture.refit_rail = false;
            }
            let expected = observation["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == id)
                .unwrap()["value"]
                .as_array()
                .unwrap();
            let actual = run(&fixture, 1, Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap())
                .unwrap_or_else(|error| panic!("{id}: {error:?}"))
                .0;
            let mut actual = actual
                .as_array()
                .unwrap()
                .iter()
                .map(|f| surface(&f["definition"]))
                .collect::<Vec<_>>();
            let mut expected = expected
                .iter()
                .map(|f| {
                    let surface = surface(&f["definition"]);
                    // Rhino runs U from the end pole to the start pole in
                    // this capture. Compare the same complete geometric image.
                    if id == "closed-both" {
                        surface
                            .try_reversed_u()
                            .unwrap()
                            .try_reparameterized(surface.domain_u(), surface.domain_v())
                            .unwrap()
                    } else {
                        surface
                    }
                })
                .collect::<Vec<_>>();
            for faces in [&mut actual, &mut expected] {
                faces.sort_by(|a, b| a.domain_v().start().total_cmp(b.domain_v().start()));
            }
            assert_eq!(actual.len(), expected.len(), "{id}");
            for (actual, expected) in actual.iter().zip(&expected) {
                for (a, b) in [
                    (actual.knots_u(), expected.knots_u()),
                    (actual.knots_v(), expected.knots_v()),
                ] {
                    assert_eq!(a.len(), b.len(), "{id}: {a:?} {b:?}");
                    assert!(
                        a.iter().zip(b).all(|(a, b)| (a - b).abs() < limit),
                        "{id}: {a:?} {b:?}"
                    );
                }
                assert_eq!(
                    actual.control_points().len(),
                    expected.control_points().len(),
                    "{id}"
                );
                for (a, b) in actual
                    .control_points()
                    .iter()
                    .zip(expected.control_points())
                {
                    assert!((a.weight() - b.weight()).abs() < limit, "{id}");
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < limit,
                        "{id}: {a:?} {b:?}"
                    );
                }
                let bound = actual
                    .parameter_surface_deviation_bound(expected, limit)
                    .unwrap()
                    .unwrap_or_else(|| panic!("{id}: full-image comparison exceeded {limit}"));
                maximum = maximum.max(bound);
            }
            compared += 1;
        }
    }
    assert_eq!(compared, count);
    println!("{compared} captures; maximum complete-surface bound {maximum:.3e}");
}
