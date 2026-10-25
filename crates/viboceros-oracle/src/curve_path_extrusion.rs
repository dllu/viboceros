//! Actual fixed-orientation extrusion commands and their model undo states.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub profile: NurbsCurveDefinition,
    pub path: NurbsCurveDefinition,
    pub crease_splitting: bool,
    pub split_at_tangents: bool,
    pub solid: bool,
    pub delete_input: bool,
    pub preselect: bool,
}

impl Fixture {
    pub fn sources(&self) -> Result<[Geometry; 2], ProbeError> {
        Ok([
            Geometry::NurbsCurve(nurbs_curve_from_definition(&self.profile)?),
            Geometry::NurbsCurve(nurbs_curve_from_definition(&self.path)?),
        ])
    }
    pub fn command(&self) -> String {
        format!(
            "ExtrudeCrvAlongCrv PathName=OraclePath Output=Surface Solid={} DeleteInput={} SplitAtTangents={}",
            yes_no(self.solid),
            yes_no(self.delete_input),
            yes_no(self.split_at_tangents)
        )
    }
    pub fn crease_command(&self) -> &'static str {
        if self.crease_splitting {
            "CreaseSplitting Enable"
        } else {
            "CreaseSplitting Disable"
        }
    }
}
fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

/// Measures source identity and product geometry. Native helper selection is
/// retained in the capture, but selection and attributes are not compared here.
pub fn snapshot(
    doc: &Document,
    ids: [ObjectId; 2],
    original: &[Geometry; 2],
) -> Result<Value, ProbeError> {
    let mut records = Vec::new();
    for obj in doc.objects() {
        if let Some(index) = ids.iter().position(|id| *id == obj.id()) {
            records.push(json!({"source":index,"unchanged":obj.geometry() == &original[index]}));
            continue;
        }
        let owned;
        let brep = match obj.geometry() {
            Geometry::Brep(b) => b,
            Geometry::NurbsSurface(s) => {
                owned = Brep::try_surface_face(s.clone(), doc.tolerance())?;
                &owned
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "unexpected curve path extrusion output",
                ));
            }
        };
        let bounds = obj.geometry().tight_bounds(doc.tolerance())?;
        let volume = brep
            .is_solid()
            .then(|| brep.signed_volume(doc.tolerance()).map(f64::abs))
            .transpose()?;
        records.push(
            json!({"source":null,"is_solid":brep.is_solid(),"face_count":brep.faces().len(),
            "volume":volume,"bounds":[bounds.min().to_array(),bounds.max().to_array()]}),
        );
    }
    Ok(json!(records))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "curve path command replay requires one iteration",
        ));
    }
    let originals = f.sources()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let profile = doc.add_geometry(originals[0].clone())?;
        let path = doc.add_geometry_with_attributes(
            originals[1].clone(),
            ObjectAttributes::on_layer(doc.current_layer_id()).with_name("OraclePath"),
        )?;
        let ids = [profile, path];
        doc.select_objects_direct([profile], SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, f.crease_command())?;
        let succeeded = registry.execute(&mut doc, &f.command()).is_ok();
        let after = snapshot(&doc, ids, &originals)?;
        let (undo, redo) = if succeeded {
            registry.execute(&mut doc, "Undo")?;
            let undo = snapshot(&doc, ids, &originals)?;
            registry.execute(&mut doc, "Redo")?;
            (undo, snapshot(&doc, ids, &originals)?)
        } else {
            (Value::Null, Value::Null)
        };
        Ok(json!({"succeeded":succeeded,"after":after,"undo":undo,"redo":redo}))
    })
}
