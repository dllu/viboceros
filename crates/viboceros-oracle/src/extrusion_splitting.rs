//! Straight-extrusion storage, splitting and complete command history.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub direction: [f64; 3],
    pub solid: bool,
    pub both_sides: bool,
    pub delete_input: bool,
    pub crease_splitting: bool,
    pub split_at_tangents: Option<bool>,
    pub use_extrusions: bool,
    #[serde(default)]
    pub seed_split: Option<bool>,
    #[serde(default)]
    pub seed_subd: bool,
    #[serde(default)]
    pub seed_cancel: bool,
}
impl Fixture {
    pub fn input(&self) -> String {
        let d = self.direction;
        let yes = |b| if b { "Yes" } else { "No" };
        let split = if self.use_extrusions {
            String::new()
        } else if self.seed_subd {
            "SplitAtTangents=No".into()
        } else {
            self.split_at_tangents
                .map(|b| format!("SplitAtTangents={}", yes(b)))
                .unwrap_or_default()
        };
        format!(
            "ExtrudeCrv 0,0,0 {},{},{} Output=Surface Solid={} BothSides={} DeleteInput={} {}",
            d[0],
            d[1],
            d[2],
            yes(self.solid && !self.seed_subd),
            yes(self.both_sides),
            yes(self.delete_input),
            split
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
            "single owned straight extrusion workflow required",
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
    commands.execute(
        &mut doc,
        if f.use_extrusions {
            "UseExtrusions Extrusion"
        } else {
            "UseExtrusions Polysurface"
        },
    )?;
    commands.set_curve_extrusion_split_at_tangents(if f.seed_subd {
        false
    } else {
        if f.seed_cancel {
            true
        } else {
            f.seed_split.unwrap_or(true)
        }
    });
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

pub fn snapshot(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
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
                geometry_record(object.geometry(), doc.tolerance())?
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
pub fn geometry_record(geometry: &Geometry, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let typed = matches!(geometry,Geometry::Brep(b)if b.is_extrusion());
    let runtime;
    let geometry = if let Geometry::Brep(b) = geometry
        && typed
    {
        runtime = Geometry::Brep(b.try_extrusion_brep_form(tolerance)?);
        &runtime
    } else {
        geometry
    };
    let mut record = crate::apex_splitting::geometry_record(geometry, tolerance)?;
    if typed {
        record["kind"] = json!("extrusion");
    }
    Ok(record)
}
