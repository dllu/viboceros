//! Shared immutable ownership, copied results and atomic soft-edit acceptance.
use super::*;

#[derive(Clone, Debug)]
pub(crate) struct SoftEditState {
    draft: refinement::Draft,
    copies: Vec<Geometry>,
    replacement: Option<Geometry>,
}
impl std::ops::Deref for SoftEditState {
    type Target = refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.draft
    }
}
impl std::ops::DerefMut for SoftEditState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.draft
    }
}
impl SoftEditState {
    pub(crate) fn new(draft: refinement::Draft) -> Self {
        Self {
            draft,
            copies: Vec::new(),
            replacement: None,
        }
    }
    pub(crate) fn working_geometry(&self) -> &Geometry {
        self.replacement.as_ref().unwrap_or(&self.source)
    }
    pub(crate) fn accept(&mut self, result: Geometry, copy: bool) {
        if copy {
            self.copies.push(result.clone());
        } else {
            self.replacement = Some(result.clone());
        }
        self.result = result;
        self.picks += 1;
    }
    pub(crate) fn scene(
        &self,
        doc: &Document,
        hover: Option<Geometry>,
        copy: bool,
    ) -> Result<Document, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("Soft edit source changed; start again"));
        }
        let mut scene = doc.clone();
        scene.clear_history()?;
        if let Some(g) = &self.replacement {
            scene.replace_object_geometries([(self.object, g.clone())])?;
        }
        for g in &self.copies {
            scene.add_geometry(g.clone())?;
        }
        if let Some(g) = hover {
            if copy {
                scene.add_geometry(g)?;
            } else {
                scene.replace_object_geometries([(self.object, g)])?;
            }
        }
        Ok(scene)
    }
    pub(crate) fn commit(
        &self,
        doc: &mut Document,
        release_selection: bool,
        retain_geometry_text: bool,
    ) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("Soft edit source changed; start again"));
        }
        if self.picks == 0 {
            return Ok(format!("{}: no edits", self.command));
        }
        let text = if retain_geometry_text {
            doc.object(self.object)
                .unwrap()
                .geometry_user_text()
                .clone()
        } else {
            BTreeMap::new()
        };
        if release_selection {
            doc.release_command_selection_on_history_replay([self.object])?;
            doc.clear_selection();
        }
        if let Some(g) = &self.replacement {
            doc.replace_object_geometries_with_history(
                [(self.object, g.clone())],
                viboceros_document::ReplacementHistory::EveryReplacement,
            )?;
            for (k, v) in &text {
                doc.set_object_geometry_user_text([self.object], k, Some(v))?;
            }
        }
        for g in &self.copies {
            doc.add_geometry_with_metadata(
                g.clone(),
                viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id()),
                text.clone(),
            )?;
        }
        Ok(format!("{}: {} edit(s)", self.command, self.picks))
    }
}
