//! SubD revolution control topology, limit samples and source/history policy.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curve: NurbsCurveDefinition,
    pub origin: [f64; 3],
    pub axis_end: [f64; 3],
    pub start_degrees: f64,
    pub sweep_degrees: f64,
    pub segment_count: usize,
    pub delete_input: bool,
    #[serde(default)]
    pub crease_splitting: bool,
    #[serde(default)]
    pub seed_surface: bool,
}
impl Fixture {
    pub fn configure(
        &self,
        commands: &CommandRegistry,
        doc: &mut Document,
    ) -> Result<(), ProbeError> {
        commands.execute(
            doc,
            if self.crease_splitting {
                "CreaseSplitting Enable"
            } else {
                "CreaseSplitting Disable"
            },
        )?;
        Ok(())
    }
    pub fn input(&self) -> String {
        let p = |p: [f64; 3]| format!("{},{},{}", p[0], p[1], p[2]);
        format!(
            "Revolve {} {} {} StartAngle={} Output=SubD SegmentCount={} DeleteInput={} {}",
            p(self.origin),
            p(self.axis_end),
            self.sweep_degrees,
            self.start_degrees,
            self.segment_count,
            if self.delete_input { "Yes" } else { "No" },
            if self.seed_surface {
                "Deformable=Yes PointCount=16 SplitAtTangents=Yes"
            } else {
                ""
            }
        )
    }
}
pub fn setup(f: &Fixture, tolerance: Tolerance) -> Result<(Document, ObjectId), ProbeError> {
    revolve_deformable::setup(
        &revolve_deformable::Fixture {
            curve: f.curve.clone(),
            origin: f.origin,
            axis_end: f.axis_end,
            start_degrees: f.start_degrees,
            sweep_degrees: f.sweep_degrees,
            point_count: 10,
            delete_input: f.delete_input,
            cancel_before_history: false,
        },
        tolerance,
    )
}
pub fn snapshot(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for object in doc.objects() {
        let mut row = json!({"source":object.id()==source,"selected":if doc.is_selected(object.id()){2}else{0}});
        match object.geometry() {
            Geometry::NurbsCurve(c) => {
                row["kind"] = json!("curve");
                row["curve"] = nurbs_curve_definition_value(c);
                let knots = row["curve"]["knots"].as_array_mut().unwrap();
                let last = knots.len() - 1;
                knots[0] = knots[1].clone();
                knots[last] = knots[last - 1].clone();
            }
            Geometry::SubD(s) => {
                row["kind"] = json!("subd");
                row["subd"] = subd_record(s)?;
            }
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "SubD revolution object required",
                ));
            }
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
pub fn subd_record(s: &viboceros_geometry::SubD) -> Result<Value, ProbeError> {
    Ok(
        json!({"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"surface_points":s.surface_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":s.faces(),"edges":s.edges(),"vertex_tags":s.vertex_tags(),"edge_tags":s.edge_tags(),"valid":true,"surface_mesh":polygon_mesh_value(&s.tessellate(2)?)}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned SubD revolution workflow required",
        ));
    }
    let (mut doc, source) = setup(f, tolerance)?;
    let commands = CommandRegistry::with_builtins();
    f.configure(&commands, &mut doc)?;
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
pub(super) fn profile(curve: &NurbsCurveDefinition) -> Result<(Value, u64), ProbeError> {
    let g = curve_native::CutSource::Nurbs(curve.clone()).geometry(Tolerance::DEFAULT)?;
    let Geometry::NurbsCurve(curve) = g else {
        return Err(ProbeError::FixtureInvariant("profile curve required"));
    };
    Ok((
        json!({"friendly":nurbs_curve_definition_value(&curve.try_subd_friendly(false)?)}),
        0,
    ))
}
