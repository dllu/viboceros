//! Component command comparisons from independently built, exactly shared inputs.
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UntrimHolesFixture {
    sources: Vec<Source>,
    all: bool,
    components: Vec<(usize, usize)>,
    maximum_edge_length: f64,
    keep_trim_objects: bool,
    pick: Pick,
    #[serde(default)]
    finish: Finish,
    #[serde(default)]
    source_layer: bool,
    #[serde(default)]
    undo_after: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Source {
    brep: crate::brep_source::BrepSourceFixture,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Pick {
    Preselect,
    Mouse,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
enum Finish {
    #[default]
    Enter,
    Cancel,
}

pub(super) fn run(
    f: &UntrimHolesFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=8).contains(&f.sources.len())
        || f.components.len() > 64
        || f.components
            .iter()
            .any(|(source, _)| *source >= f.sources.len())
        || f.undo_after
            .iter()
            .any(|pick| *pick == 0 || *pick > f.components.len())
        || !f.undo_after.windows(2).all(|pair| pair[0] < pair[1])
        || (!f.undo_after.is_empty() && f.pick != Pick::Mouse)
        || (f.pick == Pick::Preselect && f.components.len() > 1)
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid hole command component sequence",
        ));
    }
    let options = viboceros_command::UntrimHolesOptions {
        all: f.all,
        maximum_edge_length: f.maximum_edge_length,
        keep_trim_objects: f.keep_trim_objects,
    };
    options.validate()?;
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    registry.accept_object_selection_input(&options.command_line())?;
    let layer = if f.source_layer {
        document.add_layer("Sources", ColorRgb::new(0, 0, 0))?
    } else {
        document.current_layer_id()
    };
    let mut ids = Vec::new();
    let mut groups = Vec::new();
    let mut constructed = Vec::new();
    let mut source_geometry = Vec::new();
    for (index, source) in f.sources.iter().enumerate() {
        let geometry = Geometry::Brep(source.brep.build(tolerance)?);
        if let Some(path) = &source.brep.artifact_path {
            crate::brep_source::write_shared_artifact(&geometry, path, tolerance)?;
        }
        constructed.push(untrim::geometry_record(&geometry, tolerance)?);
        source_geometry.push(geometry.clone());
        let id = document.add_geometry_with_attributes(
            geometry,
            ObjectAttributes::on_layer(layer)
                .with_name(format!("source-{index}"))
                .with_object_color(ColorRgb::new(10 + index as u8, 30, 50)),
        )?;
        ids.push(id);
        groups.push(document.add_group(Some(format!("source-{index}")), [id])?);
    }
    // Transient component preselection never selects the parent object.
    let before = untrim::snapshot(&document, &ids, &groups)?;
    document.clear_history()?;
    let mut changed = Vec::new();
    for (pick, &(source, component)) in f.components.iter().enumerate() {
        // Native mouse coordinates are computed on the original source. Until
        // dynamic component mapping is implemented, reject changed topology
        // instead of reinterpreting an original index on compacted geometry.
        if f.pick == Pick::Mouse
            && document
                .object(ids[source])
                .is_none_or(|object| object.geometry() != &source_geometry[source])
        {
            return Err(ProbeError::FixtureInvariant(
                "hole mouse replay needs original component topology",
            ));
        }
        let prior = untrim::snapshot(&document, &ids, &groups)?;
        registry.execute(
            &mut document,
            &format!("UntrimHoles {} {component}", ids[source]),
        )?;
        let after = untrim::snapshot(&document, &ids, &groups)?;
        changed.push(prior != after);
        if f.undo_after.contains(&(pick + 1)) {
            if changed.pop() == Some(true) {
                document.undo()?;
            } else {
                return Err(ProbeError::FixtureInvariant(
                    "internal Undo has no accepted edit",
                ));
            }
        }
    }
    Ok((
        json!({"constructed": constructed, "before": before,
        "after": untrim::snapshot(&document, &ids, &groups)?,
        "succeeded": f.finish == Finish::Enter}),
        0,
    ))
}
