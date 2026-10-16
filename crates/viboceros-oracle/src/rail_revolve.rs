//! Command construction compared in world coordinates and exact rational basis.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub profile: NurbsCurveDefinition,
    pub rail: NurbsCurveDefinition,
    pub axis: [[f64; 3]; 2],
    pub scale_height: bool,
    pub queries: Vec<[f64; 3]>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.queries.is_empty() || f.queries.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "RailRevolve needs 1 to 64 queries",
        ));
    }
    let profile = nurbs_curve_from_definition(&f.profile)?;
    let rail = nurbs_curve_from_definition(&f.rail)?;
    let queries = f
        .queries
        .iter()
        .map(|q| point(*q))
        .collect::<Result<Vec<_>, _>>()?;
    let [a, b] = f.axis;
    let command = format!(
        "RailRevolve {},{},{} {},{},{} ScaleHeight={} SplitAtTangents=No",
        a[0],
        a[1],
        a[2],
        b[0],
        b[1],
        b[2],
        if f.scale_height { "Yes" } else { "No" }
    );
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let ids = [
            doc.add_geometry(Geometry::NurbsCurve(profile.clone()))?,
            doc.add_geometry(Geometry::NurbsCurve(rail.clone()))?,
        ];
        doc.select_objects_direct(ids, viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, &command)?;
        let output =
            doc.objects()
                .find(|o| !ids.contains(&o.id()))
                .ok_or(ProbeError::FixtureInvariant(
                    "RailRevolve produced no result",
                ))?;
        let Geometry::NurbsSurface(surface) = output.geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "RailRevolve output is not a surface",
            ));
        };
        let mut closest = vec![];
        for q in &queries {
            let (u, v) = surface.closest_parameters(*q, tolerance)?;
            closest.push(surface.evaluate(u, v)?.to_array());
        }
        Ok(json!({"created":true,
            "valid":viboceros_command::geometry_check::check_geometry(output.geometry()).status==viboceros_command::geometry_check::GeometryCheckStatus::Valid,
            "area":surface.area(tolerance)?,"closest_points":closest,
            "definition":nurbs_surface_definition_value(surface)}))
    })
}

#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn rail_revolve_fixtures_hit_independent_world_points_through_the_command() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/rail_revolve.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        for (op, row) in request.operations.iter().zip(response.results) {
            let Operation::RailRevolve { fixture, .. } = op else {
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
                    p.distance_to(point(*q).unwrap()).unwrap() < 3e-6,
                    "{}: {p:?} {q:?}",
                    row.id
                );
            }
        }
    }
}
