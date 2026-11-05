//! SubD straight-extrusion geometry and complete command history.
use super::*;
pub use crate::revolve_subd::{snapshot, subd_record};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub direction: [f64; 3],
    pub both_sides: bool,
    pub solid: bool,
    pub delete_input: bool,
    #[serde(default)]
    pub distance: Option<f64>,
}
impl Fixture {
    pub fn input(&self) -> String {
        let p = self.direction;
        let path = self.distance.map_or_else(
            || format!("0,0,0 {},{},{}", p[0], p[1], p[2]),
            |d| d.to_string(),
        );
        format!(
            "ExtrudeCrv {path} Output=SubD Solid={} BothSides={} DeleteInput={}",
            if self.solid { "Yes" } else { "No" },
            if self.both_sides { "Yes" } else { "No" },
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
