//! Direct rolling-ball producer comparisons, independent of command policy.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surfaces: [NurbsSurfaceDefinition; 2],
    pub picks: [[f64; 2]; 2],
    pub radius: f64,
    pub extend: bool,
    /// Optional diagnostic override; ordinary requests use the SDK UV picks.
    #[serde(default, deserialize_with = "explicit_sides")]
    pub sides: Option<[i32; 2]>,
    pub queries: Vec<[f64; 3]>,
}
fn explicit_sides<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<[i32; 2]>, D::Error> {
    <[i32; 2]>::deserialize(deserializer).map(Some)
}
#[cfg(feature = "native-smlib")]
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    use viboceros_smlib::{SurfaceFilletOptions, SurfaceFilletSide};
    if f.queries.is_empty()
        || f.queries.len() > 64
        || f.sides
            .is_some_and(|sides| sides.iter().any(|s| ![-1, 1].contains(s)))
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid surface fillet sides/queries",
        ));
    }
    let parents = f.surfaces.each_ref().map(nurbs_surface_from_definition);
    let [a, b] = parents;
    let parents = [a?, b?];
    let before = parents.clone();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let options = if let Some(sides) = f.sides {
            SurfaceFilletOptions {
                radius: f.radius,
                sides: sides.map(|s| {
                    if s == 1 {
                        SurfaceFilletSide::Positive
                    } else {
                        SurfaceFilletSide::Negative
                    }
                }),
                extend: f.extend,
            }
        } else {
            SurfaceFilletOptions::try_from_picks(
                &parents[0],
                &parents[1],
                f.picks,
                f.radius,
                f.extend,
                tolerance,
            )?
        };
        let output =
            viboceros_smlib::fillet_surfaces(&parents[0], &parents[1], options, tolerance)?;
        let mut records = Vec::new();
        if let Some(brep) = output {
            let native = viboceros_smlib::Shape::from_brep(&brep, tolerance)?;
            let bounds = native.tight_bounds()?;
            let mut closest = Vec::new();
            for q in &f.queries {
                let (face, u, v) = brep.closest_face_parameters(point(*q)?, tolerance)?.ok_or(
                    ProbeError::FixtureInvariant("surface fillet closest point failed"),
                )?;
                closest.push(brep.faces()[face].surface().evaluate(u, v)?.to_array());
            }
            records.push(json!({"valid":viboceros_command::geometry_check::check_geometry(&Geometry::Brep(brep.clone())).status == viboceros_command::geometry_check::GeometryCheckStatus::Valid,"is_solid":brep.is_solid(),"face_count":brep.faces().len(),"edge_count":brep.edges().len(),
                "area":brep.area(tolerance)?,"bounds":[bounds.min().to_array(),bounds.max().to_array()],"closest_points":closest,
                "geometry":crate::brep_interchange::geometry_record(&brep)?}));
        }
        Ok(json!({"outputs":records,"source_unchanged":parents==before}))
    })
}
#[cfg(not(feature = "native-smlib"))]
pub(super) fn run(_: &Fixture, _: u32, _: Tolerance) -> Result<(Value, u64), ProbeError> {
    Err(ProbeError::FixtureInvariant(
        "surface fillet producer requires native-smlib",
    ))
}

#[cfg(all(test, feature = "native-smlib"))]
#[path = "surface_fillet_images.rs"]
mod images;

#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn qualified_surface_fillet_recipes_match_saved_sdk_closest_points() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_fillet.json"
        ))
        .unwrap();
        let reference: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_fillet.json"
        ))
        .unwrap();
        let mut compared = 0;
        for op in &request.operations {
            // Curved support continuation has unqualified endpoint extents;
            // the full audit retains its complete result separately.
            if op.id() == "cylinder-True" {
                continue;
            }
            let mut one = request.clone();
            one.operations = vec![op.clone()];
            let response = run_request(&one).unwrap();
            let actual = &response.results[0].value;
            let expected = &reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == op.id())
                .unwrap()["value"];
            assert_eq!(actual["source_unchanged"], true);
            let a = &actual["outputs"][0];
            let e = &expected["outputs"][0];
            assert_eq!(a["valid"], true);
            assert_eq!(a["is_solid"], false);
            for (p, q) in a["closest_points"]
                .as_array()
                .unwrap()
                .iter()
                .zip(e["closest_points"].as_array().unwrap())
            {
                for i in 0..3 {
                    assert!(
                        (p[i].as_f64().unwrap() - q[i].as_f64().unwrap()).abs() < 1e-8,
                        "{}",
                        op.id()
                    );
                }
            }
            compared += 1;
        }
        assert_eq!(compared, 5);
    }

    #[test]
    fn uv_picks_choose_the_sdk_fillet_sides_across_quadrants_and_parameter_senses() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_fillet_picks.json"
        ))
        .unwrap();
        // Keep complete definitions on disk without embedding the large SDK
        // capture in every native test executable.
        let reference: Value = serde_json::from_str(
            &std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tools/rhino_oracle/observations/surface_fillet_picks.json"
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(request.operations.len(), 16);
        for op in &request.operations {
            let Operation::SurfaceFillet { fixture, .. } = op else {
                panic!("wrong operation")
            };
            assert!(fixture.sides.is_none());
            let mut one = request.clone();
            one.operations = vec![op.clone()];
            let response = run_request(&one).unwrap_or_else(|e| panic!("{}: {e}", op.id()));
            let actual = &response.results[0].value;
            let expected = &reference["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == op.id())
                .unwrap()["value"];
            assert_eq!(actual["source_unchanged"], true, "{}", op.id());
            assert_eq!(expected["source_unchanged"], true, "{}", op.id());
            assert_eq!(
                actual["outputs"].as_array().unwrap().len(),
                1,
                "{}",
                op.id()
            );
            assert_eq!(
                expected["outputs"].as_array().unwrap().len(),
                1,
                "{}",
                op.id()
            );
            let a = &actual["outputs"][0];
            let e = &expected["outputs"][0];
            assert_eq!(a["valid"], true, "{}", op.id());
            assert_eq!(e["valid"], true, "{}", op.id());
            assert_eq!(a["is_solid"], false, "{}", op.id());
            assert_eq!(e["is_solid"], false, "{}", op.id());
            {
                let actual = a["bounds"].as_array().unwrap();
                let expected = e["bounds"].as_array().unwrap();
                assert_eq!(actual.len(), expected.len());
                for (p, q) in actual.iter().zip(expected) {
                    for axis in 0..3 {
                        let gap = (p[axis].as_f64().unwrap() - q[axis].as_f64().unwrap()).abs();
                        assert!(gap <= 1e-8, "{} bounds: {gap}", op.id());
                    }
                }
            }
            // Closest-point answers can differ tangentially above the model
            // epsilon. Check both sampled surface images at the same
            // epsilon; retain raw closest-point differences in the full audit.
            images::assert_agreement(
                a,
                e,
                &fixture.queries,
                request.tolerance.geometry().unwrap(),
            );
        }
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    #[test]
    fn surface_fillet_request_uses_sdk_picks_by_default() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_fillet.json"
        ))
        .unwrap();
        assert_eq!(request.operations.len(), 6);
        for op in request.operations {
            let Operation::SurfaceFillet { fixture, .. } = op else {
                panic!("wrong operation")
            };
            assert_eq!(fixture.sides, None);
            assert_eq!(fixture.queries.len(), 4);
            assert!(fixture.picks.iter().flatten().all(|x| x.is_finite()));
        }
    }

    #[test]
    fn explicit_normal_sides_remain_a_diagnostic_override() {
        let mut fixture: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_fillet.json"
        ))
        .unwrap();
        fixture["operations"][0]["sides"] = json!([-1, 1]);
        let request: ProbeRequest = serde_json::from_value(fixture).unwrap();
        let Operation::SurfaceFillet { fixture, .. } = &request.operations[0] else {
            panic!("wrong operation")
        };
        assert_eq!(fixture.sides, Some([-1, 1]));
        let mut invalid: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_fillet.json"
        ))
        .unwrap();
        invalid["operations"][0]["sides"] = Value::Null;
        assert!(serde_json::from_value::<ProbeRequest>(invalid).is_err());
    }
}
