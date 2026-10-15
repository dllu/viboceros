//! Shell probes resolve physical opening points independently of face order.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    pub opening_points: Vec<[f64; 3]>,
    pub thickness: f64,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let Geometry::Brep(brep) = &source else {
        return Err(ProbeError::FixtureInvariant("Shell requires a B-rep"));
    };
    if f.opening_points.is_empty() {
        return Err(ProbeError::FixtureInvariant("Shell needs opening points"));
    }
    let mut faces = BTreeSet::new();
    for point in &f.opening_points {
        let point = Point3::try_from(*point)?;
        let mut matches = Vec::new();
        for (i, face) in brep.faces().iter().enumerate() {
            let (u, v) = face.surface().closest_parameters(point, tolerance)?;
            if face.surface().evaluate(u, v)?.distance_to(point)? <= tolerance.absolute() {
                matches.push(i);
            }
        }
        if matches.len() != 1 {
            return Err(ProbeError::FixtureInvariant(
                "opening point must identify one face interior",
            ));
        }
        faces.insert(matches[0]);
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        let refs = faces
            .iter()
            .map(|i| format!("{id}:{i}"))
            .collect::<Vec<_>>()
            .join(",");
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!("Shell Thickness={} FaceRefs={refs}", f.thickness),
        )?;
        let mut outputs = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant("Shell output is not a B-rep"));
            };
            #[cfg(feature = "native-smlib")]
            let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
                .and_then(|s| s.tight_bounds())
                .map_err(CommandError::from)?;
            #[cfg(not(feature = "native-smlib"))]
            let bounds = b.bounds();
            outputs.push(json!({"is_solid":b.is_solid(),"face_count":b.faces().len(),"volume":b.signed_volume(tolerance)?.abs(),"bounds":[bounds.min().to_array(),bounds.max().to_array()]}));
        }
        outputs.sort_by(|a, b| {
            for axis in 0..3 {
                let order = a["bounds"][0][axis]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&b["bounds"][0][axis].as_f64().unwrap());
                if !order.is_eq() {
                    return order;
                }
            }
            std::cmp::Ordering::Equal
        });
        Ok(json!({"breps":outputs}))
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn shell_fixtures_match_independent_wall_volumes() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/shell.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        let expected = [240., 192., 210., 264., 240., 96.];
        assert_eq!(result.results.len(), expected.len());
        for (row, expected) in result.results.iter().zip(expected) {
            let breps = row.value["breps"].as_array().unwrap();
            assert_eq!(
                breps.len(),
                if row.id == "shell_disconnected" { 2 } else { 1 }
            );
            assert!(breps.iter().all(|b| b["is_solid"] == true));
            let volume = breps
                .iter()
                .map(|b| b["volume"].as_f64().unwrap())
                .sum::<f64>();
            assert!(
                (volume - expected).abs() < 1e-4,
                "{}: {} expected {expected}",
                row.id,
                row.value
            );
        }
    }
}
