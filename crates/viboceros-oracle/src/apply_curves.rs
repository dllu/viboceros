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
    #[serde(default)]
    groups: Vec<Vec<usize>>,
    limit: Real,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Input {
    Curve {
        name: String,
        definition: NurbsCurveDefinition,
        #[serde(default)]
        subcurves: Vec<[Real; 2]>,
        #[serde(default)]
        subcurve_lengths: Vec<[Real; 2]>,
        #[serde(default = "selected_input")]
        selected: bool,
    },
    Point {
        name: String,
        point: [Real; 3],
    },
}

fn selected_input() -> bool {
    true
}

pub(super) fn run(
    f: &ApplyCurvesFixture,
    tolerance: Tolerance,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    run_command(f, tolerance, iterations, "ApplyCrv")
}

pub(super) fn create(
    f: &ApplyCurvesFixture,
    tolerance: Tolerance,
    iterations: u32,
) -> Result<(Value, u64), ProbeError> {
    run_command(f, tolerance, iterations, "CreateUVCrv")
}

fn run_command(
    f: &ApplyCurvesFixture,
    tolerance: Tolerance,
    iterations: u32,
    command: &str,
) -> Result<(Value, u64), ProbeError> {
    if f.inputs.len() > 64 || (command == "ApplyCrv" && f.inputs.is_empty()) {
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
        let mut selected = Vec::new();
        let mut input_options = String::new();
        for input in &f.inputs {
            let (name, geometry) = match input {
                Input::Curve {
                    name, definition, ..
                } => (
                    name,
                    Geometry::NurbsCurve(nurbs_curve_from_definition(definition)?),
                ),
                Input::Point { name, point } => (name, Geometry::Point(Point3::try_from(*point)?)),
            };
            let id = doc.add_geometry_with_attributes(
                geometry,
                ObjectAttributes::on_layer(input_layer)
                    .with_name(name.clone())
                    .try_with_user_text("viboceros-source", name.clone())?,
            )?;
            sources.push(id);
            if let Input::Curve {
                subcurves,
                subcurve_lengths,
                selected,
                ..
            } = input
            {
                if subcurves.len() + subcurve_lengths.len() > 64 {
                    return Err(ProbeError::FixtureInvariant(
                        "at most 64 temporary subcurves per source",
                    ));
                }
                for [start, end] in subcurves {
                    input_options.push_str(&format!(" SubCrv={id},{start},{end}"));
                }
                for [start, length] in subcurve_lengths {
                    input_options.push_str(&format!(" SubCrvLength={id},{start},{length}"));
                }
                if !subcurves.is_empty() || !subcurve_lengths.is_empty() || !selected {
                    continue;
                }
            }
            selected.push(id);
        }
        if f.grouped {
            doc.add_group(Some("source".into()), sources.iter().copied())?;
        }
        if f.groups.len() > 64 {
            return Err(ProbeError::FixtureInvariant("at most 64 input groups"));
        }
        for (i, group) in f.groups.iter().enumerate() {
            if group.is_empty()
                || group.len() > 65
                || group.iter().any(|index| *index >= sources.len())
            {
                return Err(ProbeError::FixtureInvariant(
                    "group source index must reference an input object",
                ));
            }
            doc.add_group(
                Some(format!("source_group_{i}")),
                group.iter().map(|index| sources[*index]),
            )?;
        }
        let output_layer = doc.add_layer("output", ColorRgb::BLACK)?;
        doc.set_current_layer(output_layer)?;
        doc.clear_history()?;
        doc.select_objects_direct(selected, SelectionMode::Replace)?;
        let before = snapshot(&doc, &sources)?;
        let start = Instant::now();
        registry.execute(
            &mut doc,
            &format!("{command} Surface={target}{input_options}"),
        )?;
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
