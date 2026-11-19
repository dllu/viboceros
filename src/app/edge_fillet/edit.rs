//! Reopen an applied edge set in a private scene with its original topology.
use super::*;
use crate::viewport::ComponentPick;
use viboceros_document::GeometrySnapshot;

pub(in crate::app) enum Edit {
    Picking(Option<ObjectId>),
    Active(Box<Session>),
}

pub(in crate::app) struct Session {
    pub(in crate::app) object: ObjectId,
    pub(super) live: GeometrySnapshot,
    pub(in crate::app) scene: Document,
}

pub(super) fn argument(word: &str) -> Option<Option<&str>> {
    let word = word.trim_start_matches('_');
    let (name, value) = word
        .split_once('=')
        .map_or((word, None), |(name, value)| (name, Some(value)));
    name.eq_ignore_ascii_case("Edit").then_some(value)
}

pub(in crate::app) fn source_document<'a>(
    edit: &'a Option<Edit>,
    document: &'a Document,
) -> &'a Document {
    match edit {
        Some(Edit::Active(session)) => &session.scene,
        _ => document,
    }
}

pub(in crate::app) fn source_document_mut<'a>(
    edit: &'a mut Option<Edit>,
    document: &'a mut Document,
) -> &'a mut Document {
    match edit {
        Some(Edit::Active(session)) => &mut session.scene,
        _ => document,
    }
}

impl Session {
    pub(super) fn is_current(&self, document: &Document) -> bool {
        document.tolerance() == self.scene.tolerance()
            && document.is_object_selectable(self.object)
            && document
                .object(self.object)
                .is_some_and(|object| object.geometry_snapshot().shares_storage_with(&self.live))
    }
}

impl VibocerosApp {
    pub(in crate::app) fn edge_blend_source_document(&self) -> &Document {
        source_document(&self.edge_blend_edit, &self.document)
    }

    pub(in crate::app) fn edge_blend_edit_target(&self) -> Option<ObjectId> {
        match &self.edge_blend_edit {
            Some(Edit::Active(session)) => Some(session.object),
            _ => None,
        }
    }

    pub(in crate::app) fn picking_edge_blend_edit(&self) -> bool {
        matches!(self.edge_blend_edit, Some(Edit::Picking(_)))
    }

    pub(super) fn begin_edge_blend_edit(&mut self, input: Option<&str>) {
        let id = match input.map(str::parse::<ObjectId>).transpose() {
            Ok(id) => id,
            Err(_) => {
                self.push_log("Enter a valid object ID after Edit=.".into());
                self.command_input.clear();
                return;
            }
        };
        self.edge_blend_edit = Some(Edit::Picking(None));
        self.edge_blend_preview.clear();
        self.edge_blend_adjustment = Adjustment::default();
        self.component_selection.clear();
        self.document.clear_selection();
        self.command_input.clear();
        self.log_edge_blend_prompt();
        if let Some(id) = id {
            self.open_edge_blend_edit_object(id);
        }
    }

    pub(in crate::app) fn select_edge_blend_edit_object(&mut self, id: ObjectId) {
        if !self.document.is_object_selectable(id)
            || !self
                .document
                .object(id)
                .is_some_and(|object| object.geometry_snapshot().edge_blend_recipe().is_some())
        {
            self.log_edge_blend_prompt();
            return;
        }
        match self.document.select_object(id, SelectionMode::Replace) {
            Ok(_) => {
                self.edge_blend_edit = Some(Edit::Picking(Some(id)));
                self.push_log("Press Enter to edit the selected edge blend.".into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
    }

    pub(super) fn confirm_edge_blend_edit_object(&mut self) {
        if let Some(Edit::Picking(Some(id))) = self.edge_blend_edit {
            self.open_edge_blend_edit_object(id);
        } else {
            self.finish_edge_fillet(false);
        }
    }

    fn open_edge_blend_edit_object(&mut self, id: ObjectId) {
        let Some((live, recipe)) = self
            .document
            .object(id)
            .and_then(|object| {
                let live = object.geometry_snapshot();
                Some((live.clone(), live.edge_blend_recipe()?.clone()))
            })
            .filter(|_| self.document.is_object_selectable(id))
        else {
            self.push_log("Select a joined edge blend with an available edge set.".into());
            return;
        };
        let Some((_, first_sizes)) = recipe.edges.first() else {
            self.push_log("This object has no saved edges to edit.".into());
            return;
        };
        self.document.clear_selection();
        let mut scene = self.document.clone();
        let restored = scene
            .clear_history()
            .and_then(|()| scene.restore_edge_blend_source(id))
            .and_then(|restored| scene.clear_history().map(|()| restored));
        match restored {
            Ok(true) => {}
            Ok(false) => {
                self.push_log("The original edge blend source is unavailable.".into());
                return;
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                return;
            }
        }
        let picks = recipe
            .edges
            .iter()
            .map(|(index, _)| ComponentPick {
                object: id,
                index: *index,
                kind: ComponentSelectionKind::BrepEdge,
            })
            .collect::<Vec<_>>();
        if let Err(error) = self.component_selection.replace(&scene, picks.clone()) {
            self.push_log(format!("Error: {error}"));
            return;
        }
        self.edge_blend_adjustment = Adjustment::default();
        self.edge_blend_adjustment.sizes = picks
            .into_iter()
            .zip(recipe.edges.iter())
            .map(|(pick, (_, sizes))| (pick, (recipe.source.clone(), sizes.clone())))
            .collect();

        let mut prompt = self.edge_fillet_prompt.expect("active edge blend prompt");
        prompt.kind = Kind::from(recipe.mode);
        prompt.radius = first_sizes.ends[0];
        prompt.tolerance = self.document.tolerance();
        prompt.adjusting = true;
        prompt.size_input = None;
        self.edge_fillet_prompt = Some(prompt);
        self.edge_blend_edit = Some(Edit::Active(Box::new(Session {
            object: id,
            live,
            scene,
        })));
        self.log_edge_blend_prompt();
    }

    pub(in crate::app) fn open_edge_blend_double_click(&mut self, id: ObjectId) -> bool {
        if !self.component_preselection_available() || self.document.is_block_editing() {
            return false;
        }
        let Some(kind) = self
            .document
            .object(id)
            .filter(|_| self.document.is_object_selectable(id))
            .and_then(|object| object.geometry_snapshot().edge_blend_recipe())
            .map(|recipe| Kind::from(recipe.mode))
        else {
            return false;
        };
        self.try_start_edge_fillet(&format!("{} Edit={id}", kind.command()))
    }

    pub(super) fn restore_latest_edge_blend(&mut self, prompt: Prompt) -> bool {
        let Some(Edit::Active(session)) = &self.edge_blend_edit else {
            return false;
        };
        if !session.is_current(&self.document) {
            self.push_log("The edge blend changed. Reopen Edit to continue.".into());
            return true;
        }
        let object = session.object;
        let result = self
            .document
            .begin_transaction(prompt.name())
            .and_then(|()| {
                if let Err(error) = self.document.restore_edge_blend_source(object) {
                    self.document.rollback_transaction()?;
                    return Err(error);
                }
                self.document.commit_transaction()
            });
        match result {
            Ok(_) => {
                self.edge_fillet_prompt = None;
                self.edge_blend_edit = None;
                self.edge_blend_adjustment = Adjustment::default();
                self.edge_blend_preview.clear();
                self.component_selection.clear();
                self.push_log("Removed the latest edge set.".into());
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }
}
