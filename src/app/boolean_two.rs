//! Selection, prepared cyclic preview, and one transaction on acceptance.
use super::*;
use viboceros_command::boolean_two::{Candidates, Mode};
use viboceros_document::{GeometrySnapshot, ObjectId};
use viboceros_geometry::Tolerance;

#[derive(Debug)]
pub(super) struct Prompt {
    pub(super) sources: Option<[ObjectId; 2]>,
    snapshots: Vec<GeometrySnapshot>,
    tolerance: Tolerance,
    pub(super) mode: Mode,
    delete_input: bool,
    candidates: Option<Candidates>,
    scenes: [Option<Document>; 5],
}
impl Prompt {
    pub(super) fn cycling(&self) -> bool {
        self.sources.is_some()
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.sources.is_none_or(|ids| {
                ids.iter().zip(&self.snapshots).all(|(id, snapshot)| {
                    doc.is_object_selectable(*id)
                        && doc
                            .object(*id)
                            .is_some_and(|o| snapshot.shares_storage_with(o.geometry_snapshot()))
                })
            })
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scenes[Mode::ALL.iter().position(|&m| m == self.mode)?].as_ref()
    }
}
impl VibocerosApp {
    pub(super) fn start_boolean_two(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if !words.first().is_some_and(|w| {
            w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Boolean2Objects")
        }) {
            return false;
        }
        if words.iter().skip(1).any(|w| {
            w.split('=').next().is_some_and(|n| {
                n.eq_ignore_ascii_case("Mode") || n.eq_ignore_ascii_case("Sources")
            })
        }) {
            return false;
        }
        let description = match self.commands.object_selection_prompt(input) {
            Ok(Some(p)) => p,
            _ => return false,
        };
        self.cancel_interactive_command(false);
        if let Err(error) = self.commands.accept_object_selection_input(input) {
            self.push_log(format!("Error: {error}"));
            return true;
        }
        self.boolean_two_prompt = Some(Prompt {
            sources: None,
            snapshots: vec![],
            tolerance: self.document.tolerance(),
            mode: Mode::Union,
            delete_input: description.options[0].value,
            candidates: None,
            scenes: std::array::from_fn(|_| None),
        });
        self.push_log(format!("> {input}"));
        if self
            .document
            .selected_objects()
            .filter(|o| {
                viboceros_command::ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
            })
            .count()
            == 2
        {
            self.prepare_boolean_two();
        } else {
            self.push_log(
                "Boolean2Objects: select two surfaces or polysurfaces; Enter previews, Esc cancels"
                    .into(),
            );
        }
        true
    }
    fn prepare_boolean_two(&mut self) {
        let ids = self
            .document
            .selected_objects()
            .filter(|o| {
                viboceros_command::ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let Ok(ids) = <[ObjectId; 2]>::try_from(ids) else {
            self.push_log("Select exactly two surfaces or polysurfaces".into());
            return;
        };
        let result = (|| -> Result<_, viboceros_command::CommandError> {
            let mut shapes = Vec::new();
            let mut snapshots = Vec::new();
            for id in ids {
                let object = self.document.object(id).unwrap();
                snapshots.push(object.geometry_snapshot().clone());
                shapes.push(match object.geometry() {
                    Geometry::Brep(b) => b.clone(),
                    Geometry::NurbsSurface(s) => viboceros_geometry::Brep::try_surface_face(
                        s.clone(),
                        self.document.tolerance(),
                    )?,
                    _ => unreachable!(),
                });
            }
            Ok((
                viboceros_command::boolean_two::prepare(
                    &shapes[0],
                    &shapes[1],
                    self.document.tolerance(),
                )?,
                snapshots,
            ))
        })();
        match result {
            Ok((candidates, snapshots)) => {
                let p = self.boolean_two_prompt.as_mut().unwrap();
                p.sources = Some(ids);
                p.candidates = Some(candidates);
                p.snapshots = snapshots;
                self.document.clear_selection();
                self.update_boolean_two_scene();
            }
            Err(error) => {
                self.cancel_boolean_two();
                self.push_log(format!("Error: {error}"));
            }
        }
    }
    fn update_boolean_two_scene(&mut self) {
        let Some(p) = self.boolean_two_prompt.as_mut() else {
            return;
        };
        let Some(ids) = p.sources else {
            return;
        };
        let index = Mode::ALL.iter().position(|&m| m == p.mode).unwrap();
        if p.scenes[index].is_none() {
            let pieces = p.candidates.as_ref().unwrap().get(p.mode);
            let mut scene = self.document.clone();
            let _ = scene.clear_history();
            let result = (|| -> Result<(), viboceros_command::CommandError> {
                scene.begin_transaction("Boolean2Objects preview")?;
                viboceros_command::boolean_two::accept(&mut scene, ids, &pieces, true)?;
                scene.commit_transaction()?;
                scene.clear_history()?;
                Ok(())
            })();
            if let Err(error) = result {
                self.push_log(format!("Error: {error}"));
                return;
            }
            p.scenes[index] = Some(scene);
        }
        let mode = p.mode.name();
        self.push_log(format!(
            "Boolean2Objects: {mode}; click for next result, Enter accepts, Esc cancels"
        ));
    }
    pub(super) fn validate_boolean_two(&mut self) {
        if self
            .boolean_two_prompt
            .as_ref()
            .is_some_and(|p| !p.current(&self.document))
        {
            self.cancel_boolean_two();
            self.push_log("Boolean2Objects sources changed; select them again".into());
        }
    }
    pub(super) fn cycle_boolean_two(&mut self) -> bool {
        let Some(p) = self.boolean_two_prompt.as_ref() else {
            return false;
        };
        if !p.cycling() {
            return false;
        }
        if !p.current(&self.document) {
            self.cancel_boolean_two();
            self.push_log("Boolean2Objects sources changed; select them again".into());
            return true;
        }
        self.boolean_two_prompt.as_mut().unwrap().mode = p.mode.next();
        self.update_boolean_two_scene();
        true
    }
    pub(super) fn cancel_boolean_two(&mut self) {
        if self.boolean_two_prompt.take().is_some() {
            self.document.clear_selection();
        }
    }
    pub(super) fn continue_boolean_two(&mut self, input: &str) -> bool {
        let Some(p) = self.boolean_two_prompt.as_ref() else {
            return false;
        };
        if input.trim_start_matches('_').eq_ignore_ascii_case("Cancel") {
            self.cancel_boolean_two();
            return true;
        }
        if !p.current(&self.document) {
            self.cancel_boolean_two();
            self.push_log("Boolean2Objects sources changed; select them again".into());
            return true;
        }
        if input.is_empty() {
            if !p.cycling() {
                self.prepare_boolean_two();
                return true;
            }
            let ids = p.sources.unwrap();
            let pieces = p.candidates.as_ref().unwrap().get(p.mode);
            let delete = p.delete_input;
            let result = (|| -> Result<(), viboceros_command::CommandError> {
                self.document.begin_transaction("Boolean2Objects")?;
                match viboceros_command::boolean_two::accept(
                    &mut self.document,
                    ids,
                    &pieces,
                    delete,
                ) {
                    Ok(()) => {
                        self.document.commit_transaction()?;
                        Ok(())
                    }
                    Err(error) => {
                        self.document.rollback_transaction()?;
                        Err(error)
                    }
                }
            })();
            match result {
                Ok(()) => {
                    self.boolean_two_prompt = None;
                    self.push_log("Accepted Boolean2Objects result".into());
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            };
            return true;
        }
        if input.eq_ignore_ascii_case("Next") {
            self.cycle_boolean_two();
            return true;
        }
        if let Some((name, value)) = input.split_once('=')
            && name.eq_ignore_ascii_case("DeleteInput")
        {
            if let Some(flag) = match value.to_ascii_lowercase().as_str() {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            } {
                let option = format!(
                    "Boolean2Objects DeleteInput={}",
                    if flag { "Yes" } else { "No" }
                );
                if self.commands.accept_object_selection_input(&option).is_ok() {
                    self.boolean_two_prompt.as_mut().unwrap().delete_input = flag;
                }
            } else {
                self.push_log("DeleteInput expects Yes or No".into());
            }
            return true;
        }
        if !input.eq_ignore_ascii_case("SelAll")
            && !input.eq_ignore_ascii_case("SelNone")
            && self
                .commands
                .recognizes(input.split_whitespace().next().unwrap_or(""))
        {
            self.cancel_boolean_two();
            return false;
        }
        if !p.cycling() {
            if input.eq_ignore_ascii_case("SelNone") {
                self.document.clear_selection();
            } else if input.eq_ignore_ascii_case("SelAll") {
                let ids = self
                    .document
                    .selectable_objects()
                    .filter(|o| {
                        viboceros_command::ObjectSelectionFilter::SurfaceComponents
                            .accepts_object(o)
                    })
                    .map(|o| o.id())
                    .collect::<Vec<_>>();
                let _ = self.document.select_command_results(ids);
            } else if let Ok(ids) = input
                .split(',')
                .map(str::parse::<ObjectId>)
                .collect::<Result<Vec<_>, _>>()
            {
                self.pick_boolean_two_objects(ids, SelectionMode::Add);
            }
        }
        self.command_input.clear();
        true
    }
    pub(super) fn pick_boolean_two_objects(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
        mode: SelectionMode,
    ) {
        for id in ids {
            if self.document.is_object_selectable(id)
                && self.document.object(id).is_some_and(|o| {
                    viboceros_command::ObjectSelectionFilter::SurfaceComponents.accepts_object(o)
                })
            {
                let _ = self.document.select_objects_direct(
                    [id],
                    if mode == SelectionMode::Replace {
                        SelectionMode::Add
                    } else {
                        mode
                    },
                );
            }
        }
    }
}
