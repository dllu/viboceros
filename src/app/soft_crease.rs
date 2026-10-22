//! Edge selection and staged SubDCrease getters; preview never edits the document.
use super::*;
use viboceros_command::{
    CommandError, ComponentSelectionKind,
    soft_crease::{SoftCreaseLabel, SoftCreaseSession},
};

pub(super) fn eligible_edge(document: &Document, pick: &crate::viewport::ComponentPick) -> bool {
    pick.kind == ComponentSelectionKind::SubDEdge
        && document.object(pick.object).is_some_and(|object| {
            matches!(object.geometry(), Geometry::SubD(subd)
                if subd.edge_face_counts().get(pick.index) == Some(&2))
        })
}

#[derive(Clone, Copy, Debug)]
enum Getter {
    Outer,
    Vertex(usize, usize),
    Constant(Option<usize>),
}

pub(super) struct Prompt {
    tolerance: Tolerance,
    editor: Option<SoftCreaseSession>,
    getter: Getter,
    pub labels: Vec<SoftCreaseLabel>,
    pub text_size: f32,
    scene: Option<Document>,
}
impl Prompt {
    pub fn selecting(&self) -> bool {
        self.editor.is_none()
    }
    pub fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub fn overlay(&self) -> Option<crate::viewport::soft_crease::Overlay<'_>> {
        self.editor.as_ref()?;
        Some(crate::viewport::soft_crease::Overlay {
            labels: &self.labels,
            text_size: self.text_size,
            highlighted: if let Getter::Vertex(c, v) = self.getter {
                Some((c, v))
            } else {
                None
            },
        })
    }
    pub fn hint(&self) -> &'static str {
        if self.selecting() {
            return "Select interior SubD edges; Enter continues, Esc cancels";
        }
        match self.getter {
            Getter::Outer => {
                "Select highlighted vertex; Enter finishes (CreaseAll ConstantWeight SmallerText LargerText)"
            }
            Getter::Vertex(_, _) => {
                "Weight at vertex (0 to 100); Enter continues (CreaseThisChain ConstantWeightChain SmallerText LargerText)"
            }
            Getter::Constant(_) => {
                "Constant weight (0 to 100); Enter continues (Crease SmallerText LargerText)"
            }
        }
    }
    fn refresh(&mut self, document: &Document) -> Result<(), CommandError> {
        let editor = self.editor.as_ref().unwrap();
        editor.validate(document)?;
        let mut scene = document.clone();
        scene.replace_object_geometries_preserving_control_selection(
            editor
                .candidates()
                .map(|(id, s)| (id, Geometry::SubD(s.clone()))),
        )?;
        self.labels = editor.labels();
        self.scene = Some(scene);
        Ok(())
    }
}

impl VibocerosApp {
    pub(super) fn picking_soft_crease_edges(&self) -> bool {
        self.soft_crease_prompt
            .as_ref()
            .is_some_and(Prompt::selecting)
    }
    fn soft_crease_targets(&self) -> Result<Vec<(ObjectId, usize)>, CommandError> {
        Ok(self
            .component_selection
            .checked_picks(&self.document)
            .map_err(|_| CommandError::CreaseStale)?
            .into_iter()
            .filter(|pick| eligible_edge(&self.document, pick))
            .map(|pick| (pick.object, pick.index))
            .collect())
    }
    pub(super) fn try_start_soft_crease(&mut self, input: &str) -> bool {
        let words = input.split_whitespace().collect::<Vec<_>>();
        if words.len() != 1
            || !words[0]
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SubDCrease")
        {
            return false;
        }
        let targets = match self.soft_crease_targets() {
            Ok(t) => t,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        let prepared = if targets.is_empty() {
            Ok(None)
        } else {
            SoftCreaseSession::prepare(&self.document, targets).map(Some)
        };
        self.cancel_interactive_command(false);
        match prepared {
            Ok(editor) => {
                let mut prompt = Prompt {
                    tolerance: self.document.tolerance(),
                    editor,
                    getter: Getter::Outer,
                    labels: Vec::new(),
                    text_size: 14.,
                    scene: None,
                };
                if prompt.selecting() {
                    self.component_selection.clear();
                    self.document.clear_selection();
                } else if let Err(e) = prompt.refresh(&self.document) {
                    self.push_log(format!("Error: {e}"));
                    return true;
                }
                self.push_log(format!("SubDCrease: {}", prompt.hint()));
                self.soft_crease_prompt = Some(prompt);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.command_input.clear();
        true
    }
    pub(super) fn try_continue_soft_crease(&mut self, input: &str) -> bool {
        let Some(mut prompt) = self.soft_crease_prompt.take() else {
            return false;
        };
        let token = input.trim();
        let option = token.trim_start_matches(['_', '-']);
        if option.eq_ignore_ascii_case("Cancel") {
            self.component_selection.clear();
            self.document.clear_selection();
            self.command_input.clear();
            self.push_log("SubDCrease cancelled".into());
            return true;
        }
        let enter = token.is_empty() || option.eq_ignore_ascii_case("Enter");
        if prompt.selecting()
            && enter
            && self
                .soft_crease_targets()
                .is_ok_and(|targets| targets.is_empty())
        {
            self.component_selection.clear();
            self.document.clear_selection();
            self.command_input.clear();
            self.push_log("SubDCrease cancelled: no interior edges selected".into());
            return true;
        }
        let result = if prompt.selecting() {
            if enter {
                if self.document.tolerance() != prompt.tolerance {
                    Err(CommandError::CreaseStale)
                } else {
                    self.soft_crease_targets()
                        .and_then(|t| SoftCreaseSession::prepare(&self.document, t))
                        .and_then(|e| {
                            prompt.editor = Some(e);
                            prompt.refresh(&self.document)
                        })
                }
            } else if option.eq_ignore_ascii_case("SelNone") {
                self.component_selection.clear();
                self.document.clear_selection();
                Ok(())
            } else {
                Err(CommandError::Usage(
                    "select interior SubD edges, then press Enter",
                ))
            }
        } else if enter && matches!(prompt.getter, Getter::Outer) {
            match prompt.editor.as_ref().unwrap().commit(&mut self.document) {
                Ok(changed) => {
                    self.component_selection.clear();
                    self.command_input.clear();
                    self.push_log(format!("SubDCrease: changed {} object(s)", changed.len()));
                    return true;
                }
                Err(e) => Err(e),
            }
        } else if enter {
            prompt.getter = Getter::Outer;
            Ok(())
        } else if option.eq_ignore_ascii_case("SmallerText")
            || option.eq_ignore_ascii_case("LargerText")
        {
            prompt.text_size = (prompt.text_size
                + if option.eq_ignore_ascii_case("LargerText") {
                    2.
                } else {
                    -2.
                })
            .clamp(8., 48.);
            Ok(())
        } else if let Ok(weight) = token.parse::<f64>() {
            let editor = prompt.editor.as_mut().unwrap();
            let result = match prompt.getter {
                Getter::Vertex(c, v) => editor.vertex(c, v, weight),
                Getter::Constant(c) => editor.constant(c, weight),
                Getter::Outer => editor.constant(None, weight),
            };
            result.and_then(|()| {
                prompt.getter = Getter::Outer;
                prompt.refresh(&self.document)
            })
        } else if option.eq_ignore_ascii_case("ConstantWeight") {
            prompt.getter = Getter::Constant(None);
            Ok(())
        } else if option.eq_ignore_ascii_case("ConstantWeightChain")
            && matches!(prompt.getter, Getter::Vertex(..))
        {
            let Getter::Vertex(c, _) = prompt.getter else {
                unreachable!()
            };
            prompt.getter = Getter::Constant(Some(c));
            Ok(())
        } else if option.eq_ignore_ascii_case("CreaseAll")
            || option.eq_ignore_ascii_case("CreaseThisChain")
            || option.eq_ignore_ascii_case("Crease")
        {
            let scope = match (option.to_ascii_lowercase().as_str(), prompt.getter) {
                ("creaseall", _) | ("crease", Getter::Constant(None)) => Ok(None),
                ("creasethischain", Getter::Vertex(c, _))
                | ("crease", Getter::Constant(Some(c))) => Ok(Some(c)),
                _ => Err(CommandError::Usage(
                    "highlight a vertex to crease its chain",
                )),
            };
            scope
                .and_then(|c| prompt.editor.as_mut().unwrap().crease(c))
                .and_then(|()| {
                    prompt.getter = Getter::Outer;
                    prompt.refresh(&self.document)
                })
        } else if token
            .split_whitespace()
            .next()
            .is_some_and(|w| self.commands.recognizes(w))
        {
            self.component_selection.clear();
            self.document.clear_selection();
            return false;
        } else {
            Err(CommandError::Usage(prompt.hint()))
        };
        match result {
            Ok(()) => self.push_log(prompt.hint().into()),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        self.soft_crease_prompt = Some(prompt);
        self.command_input.clear();
        true
    }
    pub(super) fn pick_soft_crease_vertex(&mut self, chain: usize, vertex: usize) {
        if let Some(prompt) = &mut self.soft_crease_prompt
            && prompt
                .labels
                .iter()
                .any(|l| l.chain == chain && l.vertex == vertex)
        {
            prompt.getter = Getter::Vertex(chain, vertex);
            let hint = prompt.hint();
            self.push_log(hint.into());
        }
    }
    pub(super) fn validate_soft_crease(&mut self) {
        if self.soft_crease_prompt.as_ref().is_some_and(|p| {
            p.editor
                .as_ref()
                .is_some_and(|e| e.validate(&self.document).is_err())
        }) {
            self.cancel_soft_crease();
            self.push_log("SubDCrease sources changed; select again".into());
        }
    }
    pub(super) fn cancel_soft_crease(&mut self) {
        if self.soft_crease_prompt.take().is_some() {
            self.component_selection.clear();
            self.document.clear_selection();
        }
    }
}

#[cfg(test)]
mod tests;
