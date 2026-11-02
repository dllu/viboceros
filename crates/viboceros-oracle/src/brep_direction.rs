//! Shared-input Dir command snapshots; no native trace fields are discarded here.
use super::*;
use std::path::Path;
use viboceros_io::{ThreeDmGeometry, read_3dm_file};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source: String,
    pub edit: String,
    pub preselect: bool,
    #[serde(default)]
    pub faces: Option<Vec<usize>>,
    #[serde(default)]
    pub sdk: bool,
    #[serde(default)]
    pub sequence: Option<Vec<String>>,
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub pair: bool,
}

impl Fixture {
    pub fn source(&self, tolerance: Tolerance) -> Result<Brep, ProbeError> {
        if ![
            "holed",
            "holed_reversed",
            "warped",
            "cylinder",
            "sphere",
            "cone",
            "torus",
            "box",
        ]
        .contains(&self.source.as_str())
        {
            return Err(ProbeError::FixtureInvariant("unknown shared Dir source"));
        }
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/rhino_oracle/fixtures/brep_direction")
            .join(format!("{}.3dm", self.source));
        let mut model = read_3dm_file(path, tolerance)?;
        if model.objects.len() != 1 || model.unsupported_object_count() != 0 {
            return Err(ProbeError::FixtureInvariant("Dir input requires one B-rep"));
        }
        match model.objects.remove(0).geometry {
            ThreeDmGeometry::Brep(b) => Ok(b),
            _ => Err(ProbeError::FixtureInvariant("Dir input is not a B-rep")),
        }
    }
    pub fn command(&self) -> Result<String, ProbeError> {
        let command = match self.edit.as_str() {
            "u_reverse" => "Dir UReverse",
            "v_reverse" => "Dir VReverse",
            "swap_uv" => "Dir SwapUV",
            "flip" => "Dir Flip",
            _ => return Err(ProbeError::FixtureInvariant("unknown Dir edit")),
        };
        Ok(match &self.faces {
            None => command.into(),
            Some(indices) => format!(
                "{command} Faces={}",
                indices
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        })
    }
}

pub fn snapshot(doc: &Document, id: ObjectId) -> Result<Value, ProbeError> {
    snapshots(doc, &[id], false)
}

pub fn snapshots(doc: &Document, ids: &[ObjectId], pair: bool) -> Result<Value, ProbeError> {
    if doc.objects().count() != ids.len() || ids.len() != if pair { 2 } else { 1 } {
        return Err(ProbeError::FixtureInvariant("Dir changed object count"));
    }
    let rows = ids
        .iter()
        .map(|id| object_snapshot(doc, *id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(if pair {
        json!(rows)
    } else {
        rows.into_iter().next().unwrap()
    })
}

fn object_snapshot(doc: &Document, id: ObjectId) -> Result<Value, ProbeError> {
    let obj = doc
        .object(id)
        .ok_or(ProbeError::FixtureInvariant("Dir changed object identity"))?;
    let Geometry::Brep(b) = obj.geometry() else {
        return Err(ProbeError::FixtureInvariant(
            "Dir output must remain a B-rep",
        ));
    };
    Ok(
        json!({"geometry":brep_interchange::geometry_record(b)?, "selected":doc.is_selected(id),
        "name":obj.attributes().name(), "groups":doc.groups().filter(|g|g.members().any(|member|member==id)).count()}),
    )
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "Dir replay requires one iteration",
        ));
    }
    if (f.pair || f.cancel) && f.sequence.is_none()
        || f.sequence
            .as_ref()
            .is_some_and(|steps| steps.len() > 8 || f.sdk || f.faces.is_some() || f.source == "box")
    {
        return Err(ProbeError::FixtureInvariant("invalid Dir session recipe"));
    }
    let source = f.source(tolerance)?;
    let command = f.command()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let id = doc.add_geometry_with_attributes(
            Geometry::Brep(source.clone()),
            viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id())
                .with_name("Direction source"),
        )?;
        let mut ids = vec![id];
        if f.pair {
            ids.push(
                doc.add_geometry_with_attributes(
                    Geometry::Brep(source.clone()),
                    viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id())
                        .with_name("Direction source"),
                )?,
            );
        }
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        registry.execute(&mut doc, "Group DirectionSource")?;
        if !f.preselect {
            doc.clear_selection();
        }
        doc.clear_history()?;
        let snap = |doc: &Document| snapshots(doc, &ids, f.pair);
        let before = snap(&doc)?;
        if let Some(steps) = &f.sequence {
            if f.sdk {
                return Err(ProbeError::FixtureInvariant(
                    "Dir session cannot use SDK replacement",
                ));
            }
            if !f.preselect {
                doc.set_transient_selection(ids.iter().copied())?;
            }
            let mut session =
                viboceros_command::direction::DirectionSession::new(&doc, !f.preselect)?;
            let mut mode = viboceros_command::direction::DirectionEdit::Flip;
            for step in steps {
                use viboceros_command::direction::DirectionEdit;
                let option = step.strip_prefix("Mode=").unwrap_or(step);
                if step == "NextMode" {
                    mode = mode.next_mode();
                    continue;
                }
                let edit = match option {
                    "UReverse" | "FlipU" => DirectionEdit::ReverseU,
                    "VReverse" | "FlipV" => DirectionEdit::ReverseV,
                    "SwapUV" => DirectionEdit::SwapUv,
                    "Flip" | "FlipNormal" => DirectionEdit::Flip,
                    "FlipAll" => mode,
                    _ => return Err(ProbeError::FixtureInvariant("unknown Dir session option")),
                };
                if step.starts_with("Mode=") {
                    mode = edit;
                    continue;
                }
                session.apply(&doc, edit, None)?;
            }
            if f.cancel {
                session.cancel(&mut doc)?;
            } else {
                session.accept(&mut doc)?;
            }
        } else if f.sdk {
            let mut edited = source.clone();
            let indices = f
                .faces
                .clone()
                .unwrap_or_else(|| (0..edited.faces().len()).collect());
            match f.edit.as_str() {
                "u_reverse" => edited.try_reverse_face_u(&indices)?,
                "v_reverse" => edited.try_reverse_face_v(&indices)?,
                "swap_uv" => edited.try_swap_face_uv(&indices)?,
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "SDK capture only edits face charts",
                    ));
                }
            }
            doc.replace_object_geometries([(id, Geometry::Brep(edited))])?;
        } else if f.preselect {
            registry.execute(&mut doc, &command)?;
        } else {
            doc.select_objects_direct([id], SelectionMode::Replace)?;
            registry.execute_postselected(
                &mut doc,
                &command,
                viboceros_command::CommandContext::default(),
            )?;
        }
        let after = snap(&doc)?;
        let changed = doc.undo_label().is_some();
        if changed {
            registry.execute(&mut doc, "Undo")?;
        }
        let undo = snap(&doc)?;
        if changed {
            registry.execute(&mut doc, "Redo")?;
        }
        let redo = snap(&doc)?;
        Ok(json!({"before":before,"after":after,"undo":undo,"redo":redo}))
    })
}
