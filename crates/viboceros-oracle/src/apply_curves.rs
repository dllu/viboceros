//! ApplyCrv debugging fixtures contain independent inputs, never expected output.
use super::*;
use viboceros_geometry::Real;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ApplyCurvesFixture {
    surface: NurbsSurfaceDefinition,
    inputs: Vec<Input>,
    #[serde(default)]
    grouped: bool,
    limit: Real,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Input {
    Curve {
        name: String,
        definition: NurbsCurveDefinition,
    },
    Point {
        name: String,
        point: [Real; 3],
    },
}

pub(super) fn run(
    f: &ApplyCurvesFixture,
    tolerance: Tolerance,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=64).contains(&f.inputs.len()) {
        return Err(ProbeError::FixtureInvariant(
            "ApplyCrv requires 1 to 64 input curves/points",
        ));
    }
    let tolerance = Tolerance::try_new(f.limit, tolerance.relative(), tolerance.angular())?;
    let registry = CommandRegistry::with_builtins();
    let mut result = Value::Null;
    let mut elapsed = 0_u128;
    for _ in 0..iterations {
        let mut doc = Document::new(tolerance);
        let input_layer = doc.current_layer_id();
        let target = doc.add_geometry(Geometry::NurbsSurface(nurbs_surface_from_definition(
            &f.surface,
        )?))?;
        let mut sources = vec![target];
        for input in &f.inputs {
            let (name, geometry) = match input {
                Input::Curve { name, definition } => (
                    name,
                    Geometry::NurbsCurve(nurbs_curve_from_definition(definition)?),
                ),
                Input::Point { name, point } => (name, Geometry::Point(Point3::try_from(*point)?)),
            };
            sources.push(
                doc.add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(input_layer)
                        .with_name(name.clone())
                        .try_with_user_text("viboceros-source", name.clone())?,
                )?,
            );
        }
        if f.grouped {
            doc.add_group(Some("source".into()), sources.iter().copied())?;
        }
        let output_layer = doc.add_layer("output", ColorRgb::BLACK)?;
        doc.set_current_layer(output_layer)?;
        doc.clear_history()?;
        doc.select_objects_direct(sources[1..].iter().copied(), SelectionMode::Replace)?;
        let before = snapshot(&doc, &sources)?;
        let start = Instant::now();
        registry.execute(&mut doc, &format!("ApplyCrv Surface={target}"))?;
        elapsed += start.elapsed().as_nanos();
        let after = snapshot(&doc, &sources)?;
        let group_count = doc.groups().len();
        let undoable = doc.can_undo();
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &sources)?;
        let groups_after_undo = doc.groups().len();
        registry.execute(&mut doc, "Redo")?;
        result = json!({"before":before,"after":after,"undo":undo,"redo":snapshot(&doc,&sources)?,
            "undoable":undoable,"groups_after":group_count,"groups_after_undo":groups_after_undo,
            "timing_scope":"command execution including proof and document edits; excludes setup and snapshots"});
    }
    Ok((
        result,
        u64::try_from(elapsed).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

fn snapshot(doc: &Document, sources: &[ObjectId]) -> Result<Vec<Value>, ProbeError> {
    doc.objects().map(|o| {
        let mut row = json!({"source":sources.iter().position(|id|*id==o.id()),"name":o.attributes().name(),
            "selected":doc.is_selected(o.id()),"layer":doc.layer(o.attributes().layer_id()).map(|l|l.name()),
            "group_count":o.group_ids().len(),"user_text":o.attributes().user_text()});
        if let Geometry::Point(p) = o.geometry() { row["kind"]=json!("point");row["point"]=json!(p.to_array()); }
        else if let Some(curve) = o.geometry().curve_ref() {
            let curve = curve.to_nurbs()?;
            let sampler = curve.parameter_sampler()?;
            row["kind"]=json!("curve");row["definition"]=nurbs_curve_definition_value(&curve);
            row["samples"]=json!((0..=32).map(|i|sampler.evaluate(i as Real/32.).map(Point3::to_array))
                .collect::<Result<Vec<_>,_>>()?);
        } else { row["kind"]=json!("surface"); }
        Ok(row)
    }).collect()
}
