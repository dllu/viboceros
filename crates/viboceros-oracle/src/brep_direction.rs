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
    if doc.objects().count() != 1 {
        return Err(ProbeError::FixtureInvariant("Dir changed object count"));
    }
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
        doc.select_objects_direct([id], SelectionMode::Replace)?;
        registry.execute(&mut doc, "Group DirectionSource")?;
        if !f.preselect {
            doc.clear_selection();
        }
        doc.clear_history()?;
        let before = snapshot(&doc, id)?;
        if f.sdk {
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
        let after = snapshot(&doc, id)?;
        let changed = doc.undo_label().is_some();
        if changed {
            registry.execute(&mut doc, "Undo")?;
        }
        let undo = snapshot(&doc, id)?;
        if changed {
            registry.execute(&mut doc, "Redo")?;
        }
        let redo = snapshot(&doc, id)?;
        Ok(json!({"before":before,"after":after,"undo":undo,"redo":redo}))
    })
}
