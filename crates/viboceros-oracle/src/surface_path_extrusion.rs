//! Surface path extrusion through the actual command, for public SDK comparison.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    #[serde(default)]
    pub face: usize,
    pub path: NurbsCurveDefinition,
    #[serde(default = "default_true")]
    pub solid: bool,
    #[serde(default)]
    pub reverse: bool,
    pub path_length: Option<f64>,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let path = nurbs_curve_from_definition(&f.path)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        let path = doc.add_geometry(Geometry::NurbsCurve(path.clone()))?;
        let options = viboceros_command::extrude_surface_path::SurfacePathOptions {
            solid: f.solid,
            delete_input: false,
            subcurve: false,
            split_at_tangents: false,
            reverse: Some(f.reverse),
            path_length: f.path_length,
            path_range: None,
        };
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "ExtrudeSrfAlongCrv FaceRefs={id}:{} Path={path} {}",
                f.face,
                options.command_options()
            ),
        )?;
        let mut outputs = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface path extrusion output is not a B-rep",
                ));
            };
            let volume = b
                .is_solid()
                .then(|| b.signed_volume(tolerance).map(f64::abs))
                .transpose()?;
            let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
                .and_then(|s| s.tight_bounds())
                .map_err(CommandError::from)?;
            outputs.push(json!({"is_solid":b.is_solid(),"face_count":b.faces().len(),"volume":volume,"bounds":[bounds.min().to_array(),bounds.max().to_array()]}));
        }
        Ok(json!({"breps":outputs}))
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_path_tangent_walls_match_rhino_face_and_edge_counts() {
        let request: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_apex_tangents.json"
        ))
        .unwrap();
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let points = vec![[10., 20., 0.], [10., 21., 2.], [10., 20., 4.]]
            .into_iter()
            .map(|[x, y, z]| Point3::try_new(x, y, z).unwrap())
            .collect::<Vec<_>>();
        let curved = NurbsCurve::try_new(2, points.clone(), vec![0., 0., 0., 1., 1., 1.]).unwrap();
        let corner = NurbsCurve::try_new(1, points, vec![2., 2., 3., 5., 5.]).unwrap();

        for (profile, rail, solid, off, on) in [
            ("capsule", &curved, true, &[(3, 3)][..], &[(6, 12)][..]),
            ("capsule_c1", &curved, true, &[(3, 3)], &[(6, 12)]),
            ("capsule_seam", &curved, true, &[(3, 3)], &[(7, 15)]),
            ("circle_speed", &curved, true, &[(3, 3)], &[(3, 3)]),
            ("corners", &curved, true, &[(6, 12)], &[(6, 12)]),
            ("tilted", &curved, true, &[(3, 3)], &[(6, 12)]),
            (
                "hole",
                &curved,
                false,
                &[(1, 3), (1, 3)],
                &[(1, 3), (4, 12)],
            ),
            ("capsule", &corner, true, &[(4, 5)], &[(10, 20)]),
        ] {
            let id = format!("apex-tangent-{profile}-0000");
            let value = request["operations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|operation| operation["id"] == id)
                .unwrap();
            let fixture: crate::SurfaceApexExtrusionFixture =
                serde_json::from_value(value.clone()).unwrap();
            let original = fixture.source_face(tolerance).unwrap();
            for (split, expected) in [("No", off), ("Yes", on)] {
                let mut doc = Document::new(tolerance);
                let source = doc.add_geometry(original.clone()).unwrap();
                let path = doc
                    .add_geometry(Geometry::NurbsCurve(rail.clone()))
                    .unwrap();
                let solid_value = if solid { "Yes" } else { "No" };
                CommandRegistry::with_builtins()
                    .execute(
                        &mut doc,
                        &format!(
                            "ExtrudeSrfAlongCrv FaceRefs={source}:0 Path={path} Solid={solid_value} SplitAtTangents={split}"
                        ),
                    )
                    .unwrap_or_else(|e| panic!("{profile} with SplitAtTangents={split}: {e}"));
                let mut topology = Vec::new();
                for object in doc.selected_objects() {
                    let Geometry::Brep(result) = object.geometry() else {
                        panic!("expected a B-rep result");
                    };
                    assert_eq!(result.is_solid(), solid);
                    topology.push((result.faces().len(), result.edges().len()));
                    if profile.starts_with("capsule") {
                        let expected_volume = 4. * (8. + std::f64::consts::PI);
                        let volume = result.signed_volume(tolerance).unwrap().abs();
                        assert!((volume - expected_volume).abs() < 1e-5, "{volume}");
                    }
                }
                topology.sort_unstable();
                assert_eq!(topology, expected, "{profile} with SplitAtTangents={split}");
                assert_eq!(doc.object(source).unwrap().geometry(), &original);
                assert_eq!(
                    doc.object(path).unwrap().geometry(),
                    &Geometry::NurbsCurve(rail.clone())
                );
            }
        }
    }

    #[test]
    fn surface_path_fixtures_match_volumes_and_open_ring_wall_bounds() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_path_extrusion.json"
        ))
        .unwrap();
        let r = run_request(&q).unwrap();
        assert_eq!(r.results.len(), 7);
        for row in r.results {
            let breps = row.value["breps"].as_array().unwrap();
            if row.id == "path_ring_walls" {
                // The Rhino command emits one wall per connected loop. The public
                // SDK returns a single B-rep for the same input.
                assert_eq!(breps.len(), 2);
                let mut walls = breps.iter().collect::<Vec<_>>();
                let min_x = |wall: &Value| wall["bounds"][0][0].as_f64().unwrap();
                walls.sort_by(|a, b| min_x(a).total_cmp(&min_x(b)));
                for (wall, expected) in walls.iter().zip([
                    [[-3., -3., 0.], [3., 3.5, 2.]],
                    [[-1., -1., 0.], [1., 1.5, 2.]],
                ]) {
                    assert_eq!(wall["is_solid"], false);
                    assert!(wall["volume"].is_null());
                    assert_eq!(wall["face_count"], 1);
                    for (i, corner) in expected.into_iter().enumerate() {
                        for (j, coordinate) in corner.into_iter().enumerate() {
                            let actual = wall["bounds"][i][j].as_f64().unwrap();
                            assert!((actual - coordinate).abs() < 1e-8, "{wall}");
                        }
                    }
                }
                continue;
            }
            assert_eq!(breps.len(), 1, "{}", row.id);
            let b = &breps[0];
            if row.id == "path_planar" {
                // The quadratic rail's control hull reaches y=1, but its
                // locus reaches y=0.5. Compare geometric bounds with the SDK.
                assert!((b["bounds"][1][1].as_f64().unwrap() - 3.5).abs() < 1e-5);
            }
            assert_eq!(b["is_solid"], true);
            let expected = match row.id.as_str() {
                "path_ring_hole" => 16. * std::f64::consts::PI,
                "path_half_line" => 6.,
                _ => 12.,
            };
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-4,
                "{}: {}",
                row.id,
                row.value
            );
        }
    }
}
