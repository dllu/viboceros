//! Actual tapered surface commands with independent analytic reference fixtures.
use super::*;
use crate::object_source::ObjectSource;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: ObjectSource,
    #[serde(default)]
    pub face: usize,
    pub displacement: [f64; 3],
    pub draft_angle: f64,
    #[serde(default)]
    pub round_corners: bool,
    #[serde(default = "default_true")]
    pub solid: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let source = f.source.geometry(tolerance)?;
    let d = Vector3::try_from(f.displacement)?;
    let h = d.length()?;
    let [x, y, z] = d.normalized_nonzero()?.as_vector().to_array();
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let id = doc.add_geometry(source.clone())?;
        CommandRegistry::with_builtins().execute(&mut doc,&format!("ExtrudeSrfTapered {h} Direction={x},{y},{z} FaceRefs={id}:{} DraftAngle={} Corners={} Solid={}",f.face,f.draft_angle,if f.round_corners{"Round"}else{"Sharp"},if f.solid{"Yes"}else{"No"}))?;
        let mut outputs = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface taper output is not a B-rep",
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
    fn surface_taper_pending_fixtures_match_independent_volumes() {
        let q: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/surface_tapered_extrusion.json"
        ))
        .unwrap();
        let r = run_request(&q).unwrap();
        assert_eq!(r.results.len(), 11);
        let k = 10_f64.to_radians().tan();
        let planar = 72. - 48. * k + 32. / 3. * k * k;
        for row in r.results {
            let b = &row.value["breps"][0];
            let name = row.id.strip_prefix("surface_taper_").unwrap();
            if name.ends_with("walls") {
                assert_eq!(b["is_solid"], false);
                assert!(b["volume"].is_null());
                continue;
            }
            let expected = match name {
                "bump" | "shifted_bump" => planar - 4.,
                "bump_down" => planar + 4.,
                "ring_inner" => 16. * std::f64::consts::PI * (1. - k),
                "ring_outer" => 16. * std::f64::consts::PI * (1. + k),
                "round_outward" => 72. + 48. * k + 8. * std::f64::consts::PI / 3. * k * k,
                "zero_warped" => 72.,
                _ => planar,
            };
            assert_eq!(b["is_solid"], true);
            assert!(
                (b["volume"].as_f64().unwrap() - expected).abs() < 1e-3,
                "{}: {}",
                row.id,
                row.value
            );
        }
    }
}
