//! Actual surface-to-apex command diagnostics for independent public SDK recipes.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    #[serde(default)]
    pub face: usize,
    pub apex: [f64; 3],
    #[serde(default = "default_true")]
    pub solid: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let apex = Point3::try_from(f.apex)?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        CommandRegistry::with_builtins().execute(
            &mut doc,
            &format!(
                "ExtrudeSrfToPoint {},{},{} FaceRefs={id}:{} Solid={}",
                apex.x(),
                apex.y(),
                apex.z(),
                f.face,
                if f.solid { "Yes" } else { "No" }
            ),
        )?;
        let mut outputs = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface apex extrusion output is not a B-rep",
                ));
            };
            let volume = b
                .is_solid()
                .then(|| b.signed_volume(tolerance).map(f64::abs))
                .transpose()?;
            #[cfg(feature = "native-smlib")]
            let bounds = viboceros_smlib::Shape::from_brep(b, tolerance)
                .and_then(|s| s.tight_bounds())
                .map_err(CommandError::from)?;
            #[cfg(not(feature = "native-smlib"))]
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
    fn surface_apex_pending_fixtures_match_independent_volumes() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_apex_extrusion.json"
        ))
        .unwrap();
        let r = run_request(&q).unwrap();
        assert_eq!(r.results.len(), 9);
        for row in r.results {
            let b = &row.value["breps"][0];
            if row.id == "apex_ring_walls" {
                assert_eq!(b["is_solid"], false);
                assert!(b["volume"].is_null());
                continue;
            }
            let expected = match row.id.as_str() {
                "apex_warped" | "apex_shifted" => 11.5,
                "apex_warped_down" => 12.5,
                "apex_warped_off_axis" => 65. / 6.,
                "apex_ring_hole" => 16. * std::f64::consts::PI,
                "apex_box_face" => 2. / 3.,
                _ => 12.,
            };
            assert_eq!(b["is_solid"], true);
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-4,
                "{}: {}",
                row.id,
                row.value
            );
        }
    }
}
