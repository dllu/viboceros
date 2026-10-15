//! Actual command observations for independent planar-region recipes.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    if matches!(&source, Geometry::Mesh(_)) {
        return crate::coplanar_mesh::run(&source, iterations, tolerance);
    }
    let Geometry::Brep(before) = &source else {
        return Err(ProbeError::FixtureInvariant(
            "Coplanar merge requires a B-rep",
        ));
    };
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)?;
        CommandRegistry::with_builtins().execute(&mut doc, "MergeAllCoplanarFaces")?;
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "Coplanar merge output must be a B-rep",
            ));
        };
        #[cfg(feature = "native-smlib")]
        let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
            .and_then(|shape| shape.tight_bounds())
            .map_err(CommandError::from)?;
        #[cfg(not(feature = "native-smlib"))]
        let bounds = b.tight_bounds(tolerance)?;
        let mut loops = b
            .faces()
            .iter()
            .map(|f| f.loops().len())
            .collect::<Vec<_>>();
        loops.sort_unstable();
        let volume = if b.is_solid() {
            Some(b.signed_volume(tolerance)?.abs())
        } else {
            None
        };
        Ok(
            json!({"face_count":b.faces().len(),"removed_faces":before.faces().len()-b.faces().len(),
                  "is_solid":b.is_solid(),"area":b.area(tolerance)?,"volume":volume,"loop_counts":loops,
                  "bounds":[bounds.min().to_array(),bounds.max().to_array()]}),
        )
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn coplanar_fixtures_match_independent_regions_and_integrals() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/coplanar_faces.json"
        ))
        .unwrap();
        let result = run_request(&q).unwrap();
        for row in result.results {
            let (faces, removed, area, loops) = match row.id.as_str() {
                "rectangle" | "shifted" => (1, 1, 12., vec![1]),
                "annulus" => (1, 7, 8., vec![2]),
                "box" => (6, 6, 52., vec![1; 6]),
                "disconnected" => (2, 2, 2., vec![1, 1]),
                "kink" => (2, 0, 0.5 + 0.5 * (1.0002_f64).sqrt(), vec![1, 1]),
                _ => panic!(),
            };
            assert_eq!(row.value["face_count"], faces, "{}", row.id);
            assert_eq!(row.value["removed_faces"], removed, "{}", row.id);
            assert_eq!(row.value["loop_counts"], json!(loops), "{}", row.id);
            assert!(
                (row.value["area"].as_f64().unwrap() - area).abs() < 1e-7,
                "{}: {}",
                row.id,
                row.value
            );
            let expected_bounds = match row.id.as_str() {
                "rectangle" => [[0., 0., 0.], [4., 3., 0.]],
                "shifted" => [[1e6, -2e6, 3e6], [1e6 + 4., -2e6 + 3., 3e6]],
                "annulus" => [[0., 0., 0.], [3., 3., 0.]],
                "box" => [[0., 0., 0.], [2., 3., 4.]],
                "disconnected" => [[0., 0., 0.], [4., 1., 0.]],
                "kink" => [[0., 0., 0.], [1., 1., 0.01]],
                _ => unreachable!(),
            };
            for (i, corner) in expected_bounds.iter().enumerate() {
                for (j, value) in corner.iter().enumerate() {
                    assert!(
                        (row.value["bounds"][i][j].as_f64().unwrap() - value).abs() < 1e-7,
                        "{}: {}",
                        row.id,
                        row.value
                    );
                }
            }
            if row.id == "box" {
                assert!((row.value["volume"].as_f64().unwrap() - 24.).abs() < 1e-8);
            }
        }
    }
}
