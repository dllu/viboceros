//! Sweep2 command output compared by world-space queries, independent of UV charts.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub rails: [NurbsCurveDefinition; 2],
    pub sections: Vec<NurbsCurveDefinition>,
    pub queries: Vec<[f64; 3]>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.queries.is_empty() || f.queries.len() > 64 {
        return Err(ProbeError::FixtureInvariant(
            "Sweep2 needs 1 to 64 query points",
        ));
    }
    let curves = f
        .rails
        .iter()
        .chain(f.sections.iter())
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let queries = f
        .queries
        .iter()
        .map(|q| point(*q))
        .collect::<Result<Vec<_>, _>>()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let ids = curves
            .iter()
            .map(|c| doc.add_geometry(Geometry::NurbsCurve(c.clone())))
            .collect::<Result<Vec<_>, _>>()?;
        doc.select_objects_direct(ids.clone(), viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, "Sweep2")?;
        let output = doc
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .ok_or(ProbeError::FixtureInvariant("Sweep2 produced no result"))?;
        let Geometry::Brep(brep) = output.geometry() else {
            return Err(ProbeError::FixtureInvariant("Sweep2 output is not a B-rep"));
        };
        let mut closest = vec![];
        for q in &queries {
            let (i, u, v) = brep.closest_face_parameters(*q, tolerance)?.ok_or(
                ProbeError::FixtureInvariant("Sweep2 closest-point query failed"),
            )?;
            closest.push(brep.faces()[i].surface().evaluate(u, v)?.to_array());
        }
        Ok(
            json!({"face_count":brep.faces().len(),"is_solid":brep.is_solid(),
            "valid":viboceros_command::geometry_check::check_geometry(output.geometry()).status==viboceros_command::geometry_check::GeometryCheckStatus::Valid,
            "area":brep.area(tolerance)?,"closest_points":closest}),
        )
    })
}

#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn sweep_two_rail_recipes_hit_independently_constructed_world_space_points() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/two_rail_sweep.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        for (op, row) in request.operations.iter().zip(response.results) {
            let Operation::TwoRailSweep { fixture, .. } = op else {
                panic!("wrong fixture")
            };
            assert_eq!(row.value["valid"], true, "{}", row.id);
            assert_eq!(row.value["face_count"], 1, "{}", row.id);
            assert_eq!(row.value["is_solid"], false, "{}", row.id);
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
                    p.distance_to(point(*q).unwrap()).unwrap() < 2e-6,
                    "{}: {p:?} vs {q:?}",
                    row.id
                );
            }
        }
    }
}
