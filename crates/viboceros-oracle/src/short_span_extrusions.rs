//! Full native NURBS views, source selection and history for microscopic spans.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub height: f64,
    pub both_sides: bool,
}
impl Fixture {
    pub fn input(&self) -> String {
        format!(
            "ExtrudeCrv {} Output=Surface Solid=No DeleteInput=No BothSides={}",
            self.height,
            if self.both_sides { "Yes" } else { "No" }
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
pub fn snapshot(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for obj in doc.objects() {
        let mut row =
            json!({"source":obj.id()==source,"selected":if doc.is_selected(obj.id()){2}else{0}});
        match obj.geometry() {
            Geometry::NurbsCurve(curve) => {
                row["kind"] = json!("curve");
                row["curve"] = nurbs_curve_definition_value(curve);
            }
            Geometry::Brep(brep) if brep.is_extrusion() => {
                let form = brep.extrusion().unwrap().nurbs_form()?;
                row["kind"] = json!("extrusion");
                row["surface"] = nurbs_surface_definition_value(&form.surface);
                row["faces"] = json!(brep.faces().len());
                row["caps"] = json!(form.caps);
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "native short-span extrusion required",
                ));
            }
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned extrusion workflow required",
        ));
    }
    let (mut doc, source) = setup(f, tolerance)?;
    let commands = CommandRegistry::with_builtins();
    let before = snapshot(&doc, source)?;
    commands.execute(&mut doc, &f.input())?;
    let after = snapshot(&doc, source)?;
    commands.execute(&mut doc, "Undo")?;
    let undo = snapshot(&doc, source)?;
    commands.execute(&mut doc, "Redo")?;
    let redo = snapshot(&doc, source)?;
    Ok((
        json!({"success":true,"before":before,"after":after,"undo":undo,"redo":redo}),
        0,
    ))
}
