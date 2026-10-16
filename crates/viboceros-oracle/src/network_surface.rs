//! Candidate Gordon networks compared with Rhino network fitting; parity is pending.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: Vec<NurbsCurveDefinition>,
    pub queries: Vec<[f64; 3]>,
    pub edge_tolerance: f64,
    pub interior_tolerance: f64,
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
        Ok(
            json!({"created":true,"error":0,"valid":viboceros_command::geometry_check::check_geometry(&geometry).status==viboceros_command::geometry_check::GeometryCheckStatus::Valid,
            "area":surface.area(tolerance)?,"closest_points":closest,"definition":nurbs_surface_definition_value(&surface)}),
        )
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
}
