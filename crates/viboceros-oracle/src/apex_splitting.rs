//! Surface apex-splitting geometry and complete command history.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub apex: [f64; 3],
    pub solid: bool,
    pub delete_input: bool,
    #[serde(default = "crease_default")]
    pub crease_splitting: bool,
    pub split_at_tangents: bool,
    #[serde(default = "default_true")]
    pub set_split: bool,
    #[serde(default)]
    pub seed_subd: bool,
}
fn default_true() -> bool {
    true
}
fn crease_default() -> bool {
    true
}
impl Fixture {
    pub fn input(&self) -> String {
        let p = self.apex;
        format!(
            "ExtrudeCrvToPoint {},{},{} Output=Surface Solid={} DeleteInput={} {}",
            p[0],
            p[1],
            p[2],
            if self.solid && !self.seed_subd {
                "Yes"
            } else {
                "No"
            },
            if self.delete_input { "Yes" } else { "No" },
            if self.set_split {
                format!(
                    "SplitAtTangents={}",
                    if self.split_at_tangents { "Yes" } else { "No" }
                )
            } else {
                String::new()
            }
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
            "single owned surface apex workflow required",
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
    let wrapped;
    let brep = match geometry {
        Geometry::Brep(b) => b,
        Geometry::NurbsSurface(s) => {
            wrapped = Brep::try_surface_face_with_native_edge_parameters(s.clone(), tolerance)?;
            &wrapped
        }
        _ => return Err(ProbeError::FixtureInvariant("apex surface output required")),
    };
    let orientation = if !brep.is_solid() {
        "None"
    } else if brep.signed_volume(tolerance)? > 0. {
        "Outward"
    } else {
        "Inward"
    };
    Ok(
        json!({"kind":"brep","solid":brep.is_solid(),"orientation":orientation,"surfaces":brep.faces().iter().map(|f|nurbs_surface_definition_value(f.surface())).collect::<Vec<_>>(),"brep":crate::brep_interchange::definition_record(brep)?}),
    )
}
