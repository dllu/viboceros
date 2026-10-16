//! Candidate Gordon networks compared with Rhino network fitting; parity is pending.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: Vec<NurbsCurveDefinition>,
    pub queries: Vec<[f64; 3]>,
    pub edge_tolerance: f64,
    pub interior_tolerance: f64,
    /// Optional diagnostic fractions on every complete original source.
    #[serde(default)]
    pub source_sample_fractions: Vec<f64>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.queries.is_empty() || f.queries.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "NetworkSrf needs 1 to 64 queries",
        ));
    }
    if f.source_sample_fractions.len() > 65
        || f.source_sample_fractions
            .iter()
            .any(|s| !s.is_finite() || !(0. ..=1.).contains(s))
        || f.source_sample_fractions.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid network source sample fractions",
        ));
    }
    let curves = f
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let queries = f
        .queries
        .iter()
        .map(|q| point(*q))
        .collect::<Result<Vec<_>, _>>()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let surface = construct(&curves, f, tolerance)?;
        let geometry = Geometry::NurbsSurface(surface.clone());
        let closest = queries
            .iter()
            .map(|q| {
                let (u, v) = surface.closest_parameters(*q, tolerance)?;
                Ok::<_, GeometryError>(surface.evaluate(u, v)?.to_array())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut value = json!({"created":true,"error":0,"valid":viboceros_command::geometry_check::check_geometry(&geometry).status==viboceros_command::geometry_check::GeometryCheckStatus::Valid,
            "area":surface.area(tolerance)?,"closest_points":closest,"definition":nurbs_surface_definition_value(&surface)});
        if !f.source_sample_fractions.is_empty() {
            let mut samples = vec![];
            for (index, curve) in curves.iter().enumerate() {
                for &fraction in &f.source_sample_fractions {
                    let parameter = curve.parameter_at(fraction)?;
                    let source = curve.evaluate(parameter)?;
                    let (u, v) = surface.closest_parameters(source, tolerance)?;
                    let closest = surface.evaluate(u, v)?;
                    samples.push(json!({"source_curve_index":index,"source_fraction":fraction,
                        "source_parameter":parameter,"source_point":source.to_array(),
                        "parameters":[u,v],"normalized_parameters":surface.normalized_parameters(u,v)?,
                        "point":closest.to_array(),"distance":source.distance_to(closest)?}));
                }
            }
            value["source_samples"] = json!(samples);
        }
        Ok(value)
    })
}
fn construct(
    curves: &[NurbsCurve],
    f: &Fixture,
    tolerance: Tolerance,
) -> Result<NurbsSurface, ProbeError> {
    #[cfg(feature = "native-smlib")]
    {
        Ok(viboceros_smlib::gordon_network_surface(
            curves,
            Tolerance::try_new(f.edge_tolerance, tolerance.relative(), tolerance.angular())?,
            Tolerance::try_new(
                f.interior_tolerance,
                tolerance.relative(),
                tolerance.angular(),
            )?,
        )
        .map_err(CommandError::from)?)
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = (curves, f, tolerance);
        Err(ProbeError::FixtureInvariant(
            "Gordon networks require native-smlib",
        ))
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn source_sample_limits_reject_before_native_construction() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/network_surface.json"
        ))
        .unwrap();
        let Operation::NetworkSurface { mut fixture, .. } = request.operations[0].clone() else {
            panic!("network fixture expected")
        };
        for samples in [
            vec![f64::NAN],
            vec![-0.1],
            vec![1.1],
            vec![0.5, 0.5],
            vec![1., 0.],
            (0..66).map(|i| i as f64 / 65.).collect(),
        ] {
            fixture.source_sample_fractions = samples;
            assert!(matches!(
                run(&fixture, 1, Tolerance::DEFAULT),
                Err(ProbeError::FixtureInvariant(
                    "invalid network source sample fractions"
                ))
            ));
        }
    }
}

#[cfg(all(test, not(feature = "native-smlib")))]
mod portable_tests {
    use super::*;
    #[test]
    fn network_surface_without_native_declines_the_probe() {
        let mut request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/network_surface.json"
        ))
        .unwrap();
        request.operations.truncate(1);
        assert!(
            run_request(&request)
                .unwrap_err()
                .to_string()
                .contains("native-smlib")
        );
    }
}

#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn gordon_network_surface_fixtures_hit_independent_world_space_points() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/network_surface.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        for (op, row) in request.operations.iter().zip(response.results) {
            let Operation::NetworkSurface { fixture, .. } = op else {
                panic!("wrong fixture")
            };
            assert_eq!(row.value["valid"], true, "{}", row.id);
            assert!(row.value.get("source_samples").is_none());
            for (q, p) in fixture
                .queries
                .iter()
                .zip(row.value["closest_points"].as_array().unwrap())
            {
                let p = point([
                    p[0].as_f64().unwrap(),
                    p[1].as_f64().unwrap(),
                    p[2].as_f64().unwrap(),
                ])
                .unwrap();
                assert!(
                    p.distance_to(point(*q).unwrap()).unwrap() < 1e-6,
                    "{}: {p:?} vs {q:?}",
                    row.id
                );
            }
        }
    }

    #[test]
    fn source_sample_diagnostics_keep_independent_domains_and_actual_world_distances() {
        let mut request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/network_surface.json"
        ))
        .unwrap();
        request.operations.truncate(1);
        let Operation::NetworkSurface { fixture, .. } = &mut request.operations[0] else {
            panic!("network fixture expected")
        };
        fixture.source_sample_fractions = vec![0., 0.25, 1.];
        for (i, curve) in fixture.curves.iter_mut().enumerate() {
            curve.domain = Some([100. * i as f64, 100. * i as f64 + 10.]);
        }
        let fixture = fixture.clone();
        let before = request.clone();
        let response = run_request(&request).unwrap();
        let rows = response.results[0].value["source_samples"]
            .as_array()
            .unwrap();
        assert_eq!(rows.len(), fixture.curves.len() * 3);
        for (i, rows) in rows.chunks(3).enumerate() {
            for (&fraction, row) in fixture.source_sample_fractions.iter().zip(rows) {
                assert_eq!(row["source_curve_index"], json!(i));
                assert_eq!(row["source_fraction"], json!(fraction));
                assert_eq!(
                    row["source_parameter"],
                    json!(100. * i as f64 + 10. * fraction)
                );
                let a =
                    point(serde_json::from_value(row["source_point"].clone()).unwrap()).unwrap();
                let b = point(serde_json::from_value(row["point"].clone()).unwrap()).unwrap();
                assert_eq!(row["distance"].as_f64().unwrap(), a.distance_to(b).unwrap());
                assert!(a.distance_to(b).unwrap() < 1e-9);
            }
        }
        assert_eq!(request, before);
    }
}
