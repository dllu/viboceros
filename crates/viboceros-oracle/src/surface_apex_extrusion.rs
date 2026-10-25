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

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CommandFixture {
    #[serde(flatten)]
    pub extrusion: Fixture,
    pub delete_input: bool,
    pub preselect: bool,
    #[serde(default)]
    pub split_at_tangents: bool,
    #[serde(default)]
    pub surface_details: bool,
}

pub(super) fn run_command(
    f: &CommandFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 || f.surface_details {
        return Err(ProbeError::FixtureInvariant(
            "surface apex command replay requires one iteration and portable geometry diagnostics",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let original = f.extrusion.source_face(tolerance)?;
        let mut doc = Document::new(tolerance);
        let source = doc.add_geometry(original.clone())?;
        if f.preselect {
            doc.select_objects_direct([source], SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        let [x, y, z] = f.extrusion.apex;
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc,&format!(
            "ExtrudeSrfToPoint {x},{y},{z} FaceRefs={source}:0 Solid={} DeleteInput={} SplitAtTangents={}",
            if f.extrusion.solid {"Yes"} else {"No"},
            if f.delete_input {"Yes"} else {"No"},
            if f.split_at_tangents {"Yes"} else {"No"},
        ))?;
        let after = snapshot(&doc, source, &original)?;
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, source, &original)?;
        registry.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, source, &original)?;
        Ok(
            json!({"after":after,"undo":undo,"redo":redo,"document_tolerance":{
            "absolute":tolerance.absolute(),"relative":tolerance.relative(),"angular":tolerance.angular()}}),
        )
    })
}
impl Fixture {
    /// The selected face as an independent source, shared by command/UI replays.
    pub fn source_face(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(Geometry::Brep(match self.source.geometry(tolerance)? {
            Geometry::Brep(b) => b.duplicate_faces(&[self.face], tolerance)?,
            Geometry::NurbsSurface(s) if self.face == 0 => Brep::try_surface_face(s, tolerance)?,
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "invalid surface apex source face",
                ));
            }
        }))
    }
}

/// Product geometry and retained source identity; selection/attributes are not measured.
pub fn snapshot(
    doc: &Document,
    source: ObjectId,
    original: &Geometry,
) -> Result<Value, ProbeError> {
    let mut outputs = Vec::new();
    for o in doc.objects().filter(|o| o.id() != source) {
        let Geometry::Brep(b) = o.geometry() else {
            return Err(ProbeError::FixtureInvariant(
                "surface apex product is not a B-rep",
            ));
        };
        outputs.push(brep_value(b, doc.tolerance())?);
    }
    Ok(json!({"source_present":doc.object(source).is_some(),
        "source_unchanged":doc.object(source).is_none_or(|o| o.geometry() == original),
        "breps":outputs}))
}

fn brep_value(b: &Brep, tolerance: Tolerance) -> Result<Value, ProbeError> {
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
    Ok(
        json!({"is_solid":b.is_solid(),"face_count":b.faces().len(),"volume":volume,
        "bounds":[bounds.min().to_array(),bounds.max().to_array()]}),
    )
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
        let mut products = Vec::new();
        for o in doc.selected_objects() {
            let Geometry::Brep(b) = o.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "surface apex extrusion output is not a B-rep",
                ));
            };
            products.push(b.clone());
        }
        // This legacy operation measures the public SDK recipe, which appends
        // disconnected walls to one B-rep. Actual command object counts are
        // measured separately by the surface_apex_command corpus.
        let combined = Brep::try_disjoint_union(products, tolerance)?;
        Ok(json!({"breps":[brep_value(&combined, tolerance)?]}))
    })
}
#[cfg(all(test, feature = "native-smlib"))]
mod tests {
    use super::*;
    #[test]
    fn surface_apex_fixtures_match_independent_volumes() {
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
