//! Planar-region source membership, exact input preservation and command history.
use super::*;

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
    let before = snapshot(&doc, &ids, &sources)?;
    commands.execute(&mut doc, "PlanarSrf")?;
    let success = true;
    let after = snapshot(&doc, &ids, &sources)?;
    if doc.undo_label().is_some() {
        commands.execute(&mut doc, "Undo")?;
    }
    let undo = snapshot(&doc, &ids, &sources)?;
    if doc.redo_label().is_some() {
        commands.execute(&mut doc, "Redo")?;
    }
    let redo = snapshot(&doc, &ids, &sources)?;
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
                    let matches=sources.iter().enumerate().filter_map(|(i,c)|match same_boundary(c,&edges,doc.tolerance()){
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
// Membership witnesses compare complete original curves with output loop edges
// in both directions. The portable producer additionally retains those original
// 3D curve definitions exactly; native output can split them into several edges.
fn same_boundary(
    source: &NurbsCurve,
    edges: &[&NurbsCurve],
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if edges.len() == 1 && *edges[0] == *source {
        return Ok(true);
    }
    let on = |point: Point3, candidates: &[&NurbsCurve]| -> Result<bool, GeometryError> {
        for c in candidates {
            let parameter = c.closest_parameter(point, tolerance)?;
            if c.evaluate(parameter)?.distance_to(point)? < 1e-7 {
                return Ok(true);
            }
        }
        Ok(false)
    };
    for i in 0..=32 {
        if !on(
            source.evaluate(source.parameter_at(i as f64 / 32.)?)?,
            edges,
        )? {
            return Ok(false);
        }
    }
    for edge in edges {
        for i in 0..=16 {
            if !on(
                edge.evaluate(edge.parameter_at(i as f64 / 16.)?)?,
                &[source],
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
