//! Planar-region source membership, exact input preservation and command history.
use super::*;
mod boundary;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: Vec<NurbsCurveDefinition>,
}
impl Fixture {
    pub fn setup(&self, tolerance: Tolerance) -> Result<(Document, Vec<ObjectId>), ProbeError> {
        let mut doc = Document::new(tolerance);
        let ids = self
            .curves
            .iter()
            .map(|c| Ok(doc.add_geometry(Geometry::NurbsCurve(nurbs_curve_from_definition(c)?))?))
            .collect::<Result<Vec<_>, ProbeError>>()?;
        doc.select_objects_direct(
            ids.iter().copied(),
            viboceros_document::SelectionMode::Replace,
        )?;
        doc.clear_history()?;
        Ok((doc, ids))
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_with(f, iterations, tolerance, false)
}
pub(super) fn run_chains(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    run_with(f, iterations, tolerance, true)
}
fn run_with(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    subset: bool,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned planar-region workflow required",
        ));
    }
    let (mut doc, ids) = f.setup(tolerance)?;
    let sources = f
        .curves
        .iter()
        .map(nurbs_curve_from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    let commands = CommandRegistry::with_builtins();
    let originals = doc.objects().cloned().collect::<Vec<_>>();
    let capture = |doc: &Document| snapshot_with(doc, &ids, &sources, subset);
    let before = capture(&doc)?;
    let success = match commands.execute(&mut doc, "PlanarSrf") {
        Ok(_) => true,
        Err(_) if subset => false,
        Err(e) => return Err(e.into()),
    };
    let after = capture(&doc)?;
    if doc.undo_label().is_some() {
        commands.execute(&mut doc, "Undo")?;
    }
    let undo = capture(&doc)?;
    if doc.redo_label().is_some() {
        commands.execute(&mut doc, "Redo")?;
    }
    let redo = capture(&doc)?;
    for original in &originals {
        if doc.object(original.id()) != Some(original) {
            return Err(ProbeError::FixtureInvariant(
                "PlanarSrf changed a retained source object",
            ));
        }
    }
    Ok((
        json!({"success":success,"before":before,"after":after,"undo":undo,"redo":redo}),
        0,
    ))
}
pub fn snapshot(
    doc: &Document,
    ids: &[ObjectId],
    sources: &[NurbsCurve],
) -> Result<Value, ProbeError> {
    snapshot_with(doc, ids, sources, false)
}
pub fn snapshot_chains(
    doc: &Document,
    ids: &[ObjectId],
    sources: &[NurbsCurve],
) -> Result<Value, ProbeError> {
    snapshot_with(doc, ids, sources, true)
}
fn snapshot_with(
    doc: &Document,
    ids: &[ObjectId],
    sources: &[NurbsCurve],
    subset: bool,
) -> Result<Value, ProbeError> {
    let rows=doc.objects().map(|obj| {
        let mut row=json!({"source":ids.iter().position(|id| *id==obj.id()),"selected":if doc.is_selected(obj.id()){2}else{0}});
        match obj.geometry() {
            Geometry::NurbsCurve(c)=>{
                row["kind"]=json!("curve");row["curve"]=nurbs_curve_definition_value(c);
                // OpenNURBS control getters dehomogenize their stored XYZW.
                // Observe that same coefficient chart without editing sources.
                for (value,p) in row["curve"]["control_points"].as_array_mut().unwrap().iter_mut().zip(c.control_points()) {
                    value["point"]=json!(p.point().to_array().map(|x|(x*p.weight())*(1.0/p.weight())));
                }
            }
            Geometry::Brep(b)=>{
                row["kind"]=json!("brep");row["faces"]=json!(b.faces().len());row["area"]=json!(b.area(doc.tolerance())?);
                let mut loops=Vec::new();
                for face in b.faces(){for boundary in face.loops(){
                    let edges=boundary.trims().iter().filter_map(|t|t.edge()).map(|i|b.edges()[i].curve()).collect::<Vec<_>>();
                    let matches=sources.iter().enumerate().filter_map(|(i,c)|match boundary::matches(c,&edges,doc.tolerance(),subset){
                        Ok(true)=>Some(Ok(i)),Ok(false)=>None,Err(e)=>Some(Err(e)),
                    }).collect::<Result<Vec<_>,_>>()?;
                    loops.push(json!({"kind":format!("{:?}",boundary.loop_type()),"sources":matches}));
                }}
                row["loops"]=json!(loops);
            }
            _=>return Err(ProbeError::FixtureInvariant("unexpected planar-region geometry")),
        }
        Ok(row)
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok(json!(rows))
}
