//! SubD apex-extrusion geometry and complete command history.
use super::*;
pub use crate::revolve_subd::subd_record;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub apex: [f64; 3],
    pub solid: bool,
    pub delete_input: bool,
    #[serde(default = "crease_default")]
    pub crease_splitting: bool,
    #[serde(default)]
    pub record_brep: bool,
}
fn crease_default() -> bool {
    true
}
impl Fixture {
    pub fn input(&self) -> String {
        let p = self.apex;
        format!(
            "ExtrudeCrvToPoint {},{},{} Output=SubD Solid={} DeleteInput={}",
            p[0],
            p[1],
            p[2],
            if self.solid { "Yes" } else { "No" },
            if self.delete_input { "Yes" } else { "No" }
        )
    }
}
pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, ObjectId), ProbeError> {
    let mut doc = Document::new(tolerance);
    let source =
        doc.add_geometry(curve_native::CutSource::Nurbs(f.curve.clone()).geometry(tolerance)?)?;
    doc.select_objects_direct([source], SelectionMode::Replace)?;
    doc.clear_history()?;
    Ok((doc, source))
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned SubD extrusion workflow required",
        ));
    }
    let (mut doc, source) = setup(f, tolerance)?;
    let commands = CommandRegistry::with_builtins();
    commands.execute(
        &mut doc,
        if f.crease_splitting {
            "CreaseSplitting Enable"
        } else {
            "CreaseSplitting Disable"
        },
    )?;
    let before = snapshot(&doc, source, f.record_brep)?;
    commands.execute(&mut doc, &f.input())?;
    let after = snapshot(&doc, source, f.record_brep)?;
    commands.execute(&mut doc, "Undo")?;
    let undo = snapshot(&doc, source, f.record_brep)?;
    commands.execute(&mut doc, "Redo")?;
    let redo = snapshot(&doc, source, f.record_brep)?;
    Ok((
        json!({"success":true,"before":before,"after":after,"undo":undo,"redo":redo}),
        0,
    ))
}

pub fn snapshot(doc: &Document, source: ObjectId, record_brep: bool) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for object in doc.objects() {
        let mut row = json!({"source":object.id()==source,"selected":if doc.is_selected(object.id()){2}else{0}});
        if let Geometry::NurbsCurve(curve) = object.geometry() {
            row["kind"] = json!("curve");
            row["curve"] = nurbs_curve_definition_value(curve);
            let knots = row["curve"]["knots"].as_array_mut().unwrap();
            let last = knots.len() - 1;
            knots[0] = knots[1].clone();
            knots[last] = knots[last - 1].clone();
        } else {
            row.as_object_mut().unwrap().extend(
                geometry_record(object.geometry(), record_brep)?
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
pub fn geometry_record(geometry: &Geometry, record_brep: bool) -> Result<Value, ProbeError> {
    match geometry {
        Geometry::SubD(subd) => Ok(json!({"kind":"subd","subd":subd_record(subd)?})),
        Geometry::Brep(brep) => {
            let mut row = json!({"kind":"brep","solid":brep.is_solid(),"surfaces":brep.faces().iter().map(|face|nurbs_surface_definition_value(face.surface())).collect::<Vec<_>>()});
            if record_brep {
                row["brep"] = crate::brep_interchange::definition_record(brep)?;
            }
            Ok(row)
        }
        _ => Err(ProbeError::FixtureInvariant(
            "apex SubD or fallback B-rep required",
        )),
    }
}
