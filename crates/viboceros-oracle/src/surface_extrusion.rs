//! Face extrusion diagnostics for public BrepFace.CreateExtrusion comparison.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    #[serde(default)]
    pub face: usize,
    pub displacement: [f64; 3],
    #[serde(default = "default_true")]
    pub solid: bool,
    #[serde(default)]
    pub both_sides: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let vector = Vector3::try_from(f.displacement)?;
    let height = vector.length()?;
    let [x, y, z] = vector.normalized_nonzero()?.as_vector().to_array();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "ExtrudeSrf {height} Direction={x},{y},{z} FaceRefs={id}:{} Solid={} BothSides={}",
                f.face,
                if f.solid { "Yes" } else { "No" },
                if f.both_sides { "Yes" } else { "No" }
            ),
        )?;
        let mut outputs = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface extrusion output is not a B-rep",
                ));
            };
            let volume = b
                .is_solid()
                .then(|| b.signed_volume(tolerance).map(f64::abs))
                .transpose()?;
            let bounds = b.bounds();
            outputs.push(json!({"is_solid":b.is_solid(),"face_count":b.faces().len(),"volume":volume,"bounds":[bounds.min().to_array(),bounds.max().to_array()]}));
        }
        Ok(json!({"breps":outputs}))
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn surface_extrusion_pending_protocol_fixtures_match_independent_volumes() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_extrusion.json"
        ))
        .unwrap();
        let r = run_request(&q).unwrap();
        assert_eq!(r.results.len(), 8);
        for row in r.results {
            let b = &row.value["breps"][0];
            if row.id == "surface_ring_walls" {
                assert_eq!(b["is_solid"], false);
                assert_eq!(b["face_count"], 2);
                assert!(b["volume"].is_null());
                continue;
            }
            assert_eq!(b["is_solid"], true);
            let expected = match row.id.as_str() {
                "surface_warped_oblique" => 10.5,
                "surface_warped_both" => 24.,
                "surface_ring_hole" => 16. * std::f64::consts::PI,
                "surface_box_face" => 2.,
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
