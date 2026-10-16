#![cfg(feature = "native")]
use serde_json::Value;
use viboceros_geometry::{
    AffineTransform3, NurbsCurve, Point3, Tolerance, Vector3, WeightedPoint3,
};
use viboceros_smlib::{FittedGordonNetwork, GordonCurveFitClock, fitted_gordon_network_surface};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn fixture(id: &str) -> (Vec<NurbsCurve>, Vec<Point3>) {
    let request: Value = serde_json::from_str(include_str!(
        "../../../tools/rhino_oracle/fixtures/network_surface.json"
    ))
    .unwrap();
    let op = request["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == id)
        .unwrap();
    let curves = op["curves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let points = c["control_points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|q| {
                    WeightedPoint3::try_new(
                        Point3::try_from(
                            serde_json::from_value::<[f64; 3]>(q["point"].clone()).unwrap(),
                        )
                        .unwrap(),
                        q["weight"].as_f64().unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            let source = NurbsCurve::try_new_rational(
                c["degree"].as_u64().unwrap() as usize,
                points,
                serde_json::from_value(c["knots"].clone()).unwrap(),
            )
            .unwrap();
            if let Some(domain) = c.get("domain") {
                source
                    .try_reparameterized(domain[0].as_f64().unwrap()..=domain[1].as_f64().unwrap())
                    .unwrap()
            } else {
                source
            }
        })
        .collect();
    let queries = op["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point3::try_from(serde_json::from_value::<[f64; 3]>(p.clone()).unwrap()).unwrap())
        .collect();
    (curves, queries)
}
fn tolerance() -> Tolerance {
    Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap()
}
fn local() -> GordonCurveFitClock {
    GordonCurveFitClock::Local {
        transition_fraction: 0.5,
    }
}
fn verify_report(result: &FittedGordonNetwork, sources: &[NurbsCurve]) {
    assert_eq!(result.source_images.len(), sources.len());
    assert!(result.maximum_sampled_point_distance <= 1e-7);
    for (i, image) in result.source_images.iter().enumerate() {
        assert_eq!(image.source_index, i);
        assert_eq!(image.uv.domain(), sources[i].domain());
        assert!(image.distance_bound <= 1e-6);
        let axis = usize::from(!result.families[0].contains(&i));
        let row = result.families[axis].iter().position(|j| *j == i).unwrap();
        let domain = if axis == 0 {
            result.surface.domain_v()
        } else {
            result.surface.domain_u()
        };
        let fixed =
            result.stations[1 - axis][row].mul_add(domain.end() - domain.start(), *domain.start());
        assert!(image.uv.control_points().iter().all(|p| if axis == 0 {
            p.point().y() == fixed
        } else {
            p.point().x() == fixed
        }));
        for j in 0..=16 {
            let t = sources[i].parameter_at(j as f64 / 16.).unwrap();
            let uv = image.uv.evaluate(t).unwrap();
            assert!(
                result
                    .surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .distance_to(sources[i].evaluate(t).unwrap())
                    .unwrap()
                    <= 1e-6
            );
        }
    }
}

#[test]
fn automatic_plane_keeps_small_grids_reversed_directions_and_remote_original_domains() {
    let (mut sources, queries) = fixture("rectangle");
    let transform = AffineTransform3::try_new(
        [[0., 0., -2.], [-2., 0., 0.], [0., 2., 0.]],
        Vector3::try_new(100_000., -200_000., 300_000.).unwrap(),
    )
    .unwrap();
    for (i, source) in sources.iter_mut().enumerate() {
        *source = source
            .transformed(transform)
            .unwrap()
            .try_reparameterized((10. * i as f64 - 3.)..=(10. * i as f64 + 7.))
            .unwrap();
        if i % 2 == 0 {
            *source = source.reversed().unwrap();
        }
    }
    sources.rotate_left(1);
    let before = sources.clone();
    let result = fitted_gordon_network_surface(
        &sources,
        tolerance(),
        tolerance(),
        Tolerance::DEFAULT,
        local(),
    )
    .unwrap();
    assert_eq!(result.family_fit_attempts, [1, 1]);
    assert_eq!(result.spans_per_interval, [vec![4], vec![4]]);
    assert_eq!(
        [
            result.surface.control_point_count_u(),
            result.surface.control_point_count_v()
        ],
        [7, 7]
    );
    verify_report(&result, &sources);
    for query in queries {
        let query = transform.transform_point(query).unwrap();
        let (u, v) = result
            .surface
            .closest_parameters(query, Tolerance::DEFAULT)
            .unwrap();
        assert!(
            result
                .surface
                .evaluate(u, v)
                .unwrap()
                .distance_to(query)
                .unwrap()
                < 1e-6
        );
    }
    assert_eq!(sources, before);
}

#[test]
fn automatic_curved_network_refines_and_certifies_complete_original_isocurves() {
    let (sources, _) = fixture("biquadratic_network");
    let before = sources.clone();
    let result = fitted_gordon_network_surface(
        &sources,
        tolerance(),
        tolerance(),
        Tolerance::DEFAULT,
        local(),
    )
    .unwrap();
    assert!(result.family_fit_attempts.iter().any(|n| *n > 1));
    assert!(result.family_fit_attempts.iter().all(|n| *n <= 7));
    assert!(
        result
            .spans_per_interval
            .iter()
            .flatten()
            .all(|n| *n >= 4 && *n <= 62)
    );
    assert_eq!(
        [result.surface.degree_u(), result.surface.degree_v()],
        [3, 3]
    );
    for knots in [result.surface.knots_u(), result.surface.knots_v()] {
        assert!(
            knots
                .chunk_by(|a, b| a == b)
                .filter(|k| k[0] > 0. && k[0] < 1.)
                .all(|k| k.len() == 1)
        );
    }
    verify_report(&result, &sources);
    let first = &result.source_images[0];
    let mut certificate = result
        .surface
        .prepare_surface_curve_bounds()
        .unwrap()
        .unwrap();
    assert!(
        certificate
            .bound(&first.uv, &sources[0], first.distance_bound)
            .unwrap()
            .is_some()
    );
    assert_eq!(sources, before);
}

#[test]
fn automatic_fit_declines_invalid_topology_and_excessive_shared_control_budgets() {
    let (mut sources, _) = fixture("rectangle");
    sources.push(sources[0].clone());
    let before = sources.clone();
    assert!(
        fitted_gordon_network_surface(
            &sources,
            tolerance(),
            tolerance(),
            Tolerance::DEFAULT,
            local()
        )
        .is_err()
    );
    assert_eq!(sources, before);
    let mut grid = vec![];
    for axis in [true, false] {
        for i in 0..=16 {
            let t = i as f64 / 16.;
            let points = if axis {
                vec![p(0., t, 0.), p(1., t, 0.)]
            } else {
                vec![p(t, 0., 0.), p(t, 1., 0.)]
            };
            grid.push(NurbsCurve::try_new(1, points, vec![0., 0., 1., 1.]).unwrap());
        }
    }
    let error =
        fitted_gordon_network_surface(&grid, tolerance(), tolerance(), Tolerance::DEFAULT, local())
            .err()
            .unwrap();
    assert!(
        error.to_string().contains("prepared control budget"),
        "{error}"
    );
    assert!(
        fitted_gordon_network_surface(
            &[],
            tolerance(),
            tolerance(),
            Tolerance::DEFAULT,
            GordonCurveFitClock::Local {
                transition_fraction: f64::NAN
            }
        )
        .is_err()
    );
}

#[test]
fn automatic_rational_network_retains_negative_gauges_and_complete_images() {
    let (mut sources, _) = fixture("rational_cylinder");
    for (i, source) in sources.iter_mut().enumerate() {
        *source = NurbsCurve::try_new_rational(
            source.degree(),
            source
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), -p.weight() * 0.125).unwrap())
                .collect(),
            source.knots().to_vec(),
        )
        .unwrap()
        .try_reparameterized((i as f64 * 20. - 7.)..=(i as f64 * 20. + 3.))
        .unwrap();
    }
    let before = sources.clone();
    let result = fitted_gordon_network_surface(
        &sources,
        tolerance(),
        tolerance(),
        Tolerance::DEFAULT,
        local(),
    )
    .unwrap();
    verify_report(&result, &sources);
    assert_eq!(sources, before);
}

#[test]
fn automatic_uneven_families_share_the_total_budget_without_a_uniform_per_source_cap() {
    let mut sources = vec![];
    for t in [0., 1.] {
        sources.push(
            NurbsCurve::try_new(1, vec![p(0., t, 0.), p(1., t, 0.)], vec![0., 0., 1., 1.]).unwrap(),
        );
    }
    for i in 0..32 {
        let t = i as f64 / 31.;
        sources.push(
            NurbsCurve::try_new(1, vec![p(t, 0., 0.), p(t, 1., 0.)], vec![0., 0., 1., 1.]).unwrap(),
        );
    }
    let result = fitted_gordon_network_surface(
        &sources,
        tolerance(),
        tolerance(),
        Tolerance::DEFAULT,
        local(),
    )
    .unwrap();
    assert_eq!(
        result.families.each_ref().map(|family| family.len()),
        [2, 32]
    );
    assert_eq!(result.spans_per_interval[0], vec![4; 31]);
    assert_eq!(result.spans_per_interval[1], vec![4]);
    verify_report(&result, &sources);
}

#[test]
fn automatic_small_interior_contact_gaps_keep_complete_natural_endpoints() {
    let mut sources = vec![];
    for t in [0., 0.5, 1.] {
        let source = if t == 0.5 {
            NurbsCurve::try_new(
                2,
                vec![p(0., t, 0.), p(0.5, t, 1e-7), p(1., t, 0.)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap()
        } else {
            NurbsCurve::try_new(1, vec![p(0., t, 0.), p(1., t, 0.)], vec![0., 0., 1., 1.]).unwrap()
        };
        sources.push(source);
    }
    for t in [0., 0.5, 1.] {
        sources.push(
            NurbsCurve::try_new(1, vec![p(t, 0., 0.), p(t, 1., 0.)], vec![0., 0., 1., 1.]).unwrap(),
        );
    }
    let before = sources.clone();
    let result = fitted_gordon_network_surface(
        &sources,
        tolerance(),
        tolerance(),
        Tolerance::DEFAULT,
        local(),
    )
    .unwrap();
    verify_report(&result, &sources);
    assert_eq!(sources, before);
}
