//! FilletSrf preparation, document policy, and scriptable UV picks.
use super::*;
mod options;
mod prepare;
pub use options::{Options, Trim, parse};
pub use prepare::{prepare, source_surface};
#[cfg(test)]
mod tests;
pub(super) struct FilletSurfaceCommand;
const USAGE: &str = "FilletSrf [Sources=id,id] Pick1=u,v Pick2=u,v [Radius=positive] [Extend=Yes|No] [Trim=Yes|No|Split] [Join=Yes|No] [BlendType=Arc] [Preview=Yes|No]";
fn failure(context: &'static str) -> CommandError {
    CommandError::SurfaceFillet(context)
}
#[derive(Clone, Debug)]
struct Output {
    brep: Brep,
    owner: Option<usize>,
    selected: bool,
}
#[derive(Clone, Debug)]
pub struct Prepared {
    outputs: Vec<Output>,
    sources: [viboceros_document::Object; 2],
    replace: [bool; 2],
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
}
impl Prepared {
    pub fn message(&self) -> String {
        format!("FilletSrf: {} result object(s)", self.outputs.len())
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.layer == doc.current_layer_id()
            && self.sources.iter().all(|old| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
    fn apply(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        if !self.is_current(doc) {
            return Err(failure(
                "sources, attributes, groups, current layer or tolerance changed; prepare again",
            ));
        }
        let mut replaced = [false; 2];
        let mut selected = Vec::new();
        for output in &self.outputs {
            let geometry = Geometry::Brep(output.brep.clone());
            let id = if let Some(i) = output.owner {
                let source = &self.sources[i];
                if !replaced[i] {
                    doc.replace_object_geometries([(source.id(), geometry)])?;
                    // Explicit document replacement clears geometry text; fillet
                    // parent edits inherit it, like the additional split pieces.
                    for (key, value) in source.geometry_user_text() {
                        doc.set_object_geometry_user_text([source.id()], key, Some(value))?;
                    }
                    replaced[i] = true;
                    source.id()
                } else {
                    doc.copy_object_pieces_with_metadata_into_source_groups([(
                        source.id(),
                        geometry,
                        source.geometry_user_text().clone(),
                    )])?[0]
                }
            } else {
                doc.add_geometry(geometry)?
            };
            if output.selected {
                selected.push(id);
            }
        }
        for (i, source) in self.sources.iter().enumerate() {
            if self.replace[i] && !replaced[i] {
                doc.delete_object(source.id())?;
            }
        }
        doc.select_objects_direct(selected.iter().copied(), SelectionMode::Replace)?;
        Ok(selected)
    }
    pub fn commit(&self, doc: &mut Document) -> Result<Vec<ObjectId>, CommandError> {
        run_command_transaction(doc, "FilletSrf", |doc| self.apply(doc))
    }
}
impl Command for FilletSurfaceCommand {
    fn name(&self) -> &'static str {
        "FilletSrf"
    }
    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let options = parse(args)?;
        let ids = if let Some(ids) = options.sources {
            ids
        } else {
            selected_ids(document)?
                .try_into()
                .map_err(|_| CommandError::Usage(USAGE))?
        };
        let prepared = prepare(document, ids, &options)?;
        if !options.preview {
            prepared.apply(document)?;
        }
        Ok(format!(
            "{}{}",
            prepared.message(),
            if options.preview {
                "; preview only"
            } else {
                "; accepted"
            }
        ))
    }
}
