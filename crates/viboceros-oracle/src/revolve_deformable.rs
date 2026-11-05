//! Public-command lifecycle and full cubic revolution control-net comparisons.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub origin: [f64; 3],
    pub axis_end: [f64; 3],
    pub start_degrees: f64,
    pub sweep_degrees: f64,
    pub point_count: usize,
    pub delete_input: bool,
    #[serde(default = "idle_cancel_default")]
    pub cancel_before_history: bool,
}
fn idle_cancel_default() -> bool {
    true
}
impl Fixture {
    pub fn input(&self) -> String {
        let p = |v: [f64; 3]| format!("{},{},{}", v[0], v[1], v[2]);
        format!(
            "Revolve {} {} {} StartAngle={} Deformable=Yes PointCount={} DeleteInput={} Output=Surface SplitAtTangents=No",
            p(self.origin),
            p(self.axis_end),
            self.sweep_degrees,
            self.start_degrees,
            self.point_count,
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
pub fn snapshot(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for obj in doc.objects() {
        let mut row =
            json!({"source":obj.id()==source,"selected":if doc.is_selected(obj.id()){2}else{0}});
        match obj.geometry() {
            Geometry::NurbsCurve(curve) => {
                row["kind"] = json!("curve");
                row["curve"] = nurbs_curve_definition_value(curve);
                // The native observer reconstructs two unused full-vector
                // padding knots from OpenNURBS's short knot vector.
                let knots = row["curve"]["knots"].as_array_mut().unwrap();
                let last = knots.len() - 1;
                knots[0] = knots[1].clone();
                knots[last] = knots[last - 1].clone();
            }
            Geometry::NurbsSurface(surface) => {
                row["kind"] = json!("surface");
                row["surfaces"] = json!([nurbs_surface_definition_value(surface)]);
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "deformable revolution surface required",
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
            "single owned revolution workflow required",
        ));
    }
    let (mut doc, source) = setup(f, tolerance)?;
    let commands = CommandRegistry::with_builtins();
    let before = snapshot(&doc, source)?;
    commands.execute(&mut doc, &f.input())?;
    let after = snapshot(&doc, source)?;
    // The native workflow sends two idle Cancels before history commands.
    // These clear whole-object selection without creating model history.
    if f.cancel_before_history {
        doc.clear_selection();
    }
    commands.execute(&mut doc, "Undo")?;
    let undo = snapshot(&doc, source)?;
    commands.execute(&mut doc, "Redo")?;
    let redo = snapshot(&doc, source)?;
    Ok((
        json!({"success":true,"before":before,"after":after,"undo":undo,"redo":redo}),
        0,
    ))
}
