//! Component command comparisons from independently built, exactly shared inputs.
use super::*;
pub(super) mod window;

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
    #[serde(default)]
    undo_redo: bool,
    preselect_kind: Option<Kind>,
    #[serde(default)]
    trace_components: bool,
    window: Option<[[f64; 3]; 2]>,
    window_subobjects: Option<bool>,
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
    Window,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Edge,
    Face,
}
impl Kind {
    fn component(self, index: usize) -> viboceros_command::UntrimHolesComponent {
        match self {
            Self::Edge => viboceros_command::UntrimHolesComponent::Edge(index),
            Self::Face => viboceros_command::UntrimHolesComponent::Face(index),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Edge => "edge",
            Self::Face => "face",
        }
    }
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
        || (f.preselect_kind.is_some() && f.pick != Pick::Preselect)
        || (f.pick == Pick::Window && (f.window.is_none() || !f.components.is_empty()))
        || (f.pick != Pick::Window && (f.window.is_some() || f.window_subobjects.is_some()))
        || f.window.is_some_and(|corners| {
            corners
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
        })
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
    let mut group = document.begin_history_group("UntrimHoles")?;
    let kind = f
        .preselect_kind
        .unwrap_or(if f.all { Kind::Face } else { Kind::Edge });
    let mut rejected_preselection = false;
    if f.pick != Pick::Mouse {
        let picks = if f.pick == Pick::Preselect {
            f.components
                .iter()
                .map(|&(source, index)| (ids[source], kind.component(index)))
                .collect()
        } else {
            window::picks(&document, &ids, f.window.unwrap(), f.all)?
        };
        match viboceros_command::UntrimHolesSelection::prepare_preselected(
            &document, picks, options,
        ) {
            Ok(Some(selection)) => {
                selection.commit_in_group(&mut document, &mut group)?;
            }
            Ok(None) => {}
            Err(CommandError::UntrimHolesMultipleComponents) => {
                rejected_preselection = f.pick == Pick::Preselect;
            }
            Err(error) => return Err(error.into()),
        }
    }
    for (pick, &(source, component)) in f.components.iter().enumerate() {
        let Geometry::Brep(original) = &source_geometry[source] else {
            unreachable!()
        };
        let count = if kind == Kind::Face {
            original.faces().len()
        } else {
            original.edges().len()
        };
        if component >= count {
            return Err(ProbeError::FixtureInvariant(
                "hole component outside source",
            ));
        }
        if f.pick != Pick::Mouse {
            continue;
        }
        let Geometry::Brep(current) = document.object(ids[source]).unwrap().geometry() else {
            unreachable!()
        };
        // Mouse fixtures refer to original spatial components. Match the exact
        // surviving source curve/surface after compaction, never reuse its index.
        let candidates = if current == original {
            vec![component]
        } else if f.all {
            current
                .faces()
                .iter()
                .enumerate()
                .filter_map(|(index, face)| {
                    (face.surface() == original.faces()[component].surface()
                        && face.is_reversed() == original.faces()[component].is_reversed())
                    .then_some(index)
                })
                .collect::<Vec<_>>()
        } else {
            current
                .edges()
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| {
                    (edge.curve() == original.edges()[component].curve()).then_some(index)
                })
                .collect::<Vec<_>>()
        };
        let [index] = candidates.as_slice() else {
            return Err(ProbeError::FixtureInvariant(
                "hole mouse component has no unique surviving match",
            ));
        };
        let component = if f.all {
            viboceros_command::UntrimHolesComponent::Face(*index)
        } else {
            viboceros_command::UntrimHolesComponent::Edge(*index)
        };
        viboceros_command::UntrimHolesSelection::prepare(
            &document,
            ids[source],
            component,
            options,
        )?
        .commit_in_group(&mut document, &mut group)?;
        if f.undo_after.contains(&(pick + 1)) && !document.undo_history_group(&mut group)? {
            return Err(ProbeError::FixtureInvariant(
                "internal Undo has no accepted edit",
            ));
        }
    }
    let after = untrim::snapshot(&document, &ids, &groups)?;
    let mut result = json!({"constructed": constructed, "before": before,
        "after": after, "succeeded": !rejected_preselection && f.finish == Finish::Enter});
    if f.trace_components {
        let selected = if f.pick == Pick::Preselect {
            f.components
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|(source, index)| json!([source, kind.name(), index]))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        result["component_selection"] = json!({"before":selected,"after":[]});
    }
    if f.undo_redo {
        let tested = before != after;
        result["history_tested"] = json!(tested);
        if tested {
            document.undo()?;
            result["undo"] = Value::Array(untrim::snapshot(&document, &ids, &groups)?);
            document.redo()?;
            result["redo"] = Value::Array(untrim::snapshot(&document, &ids, &groups)?);
            if f.trace_components {
                result["component_selection"]["undo"] = json!([]);
                result["component_selection"]["redo"] = json!([]);
            }
        }
    }
    Ok((result, 0))
}
