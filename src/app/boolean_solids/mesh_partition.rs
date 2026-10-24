//! MeshSplit options layered on the shared two-set selection and plane getter.
use super::super::project_plane::PlaneGetter;
use super::*;

#[derive(Clone, Debug)]
pub(in crate::app) struct PartitionOptions {
    pub coplanar: &'static str,
    pub ngons: bool,
    pub discard: bool,
    pub permit: bool,
    pub planes: Vec<String>,
    pub plane_getter: Option<PlaneGetter>,
    pub pending_choice: Option<&'static str>,
    pub trim_history: Option<std::rc::Rc<std::cell::RefCell<viboceros_document::HistoryGroup>>>,
}
impl PartitionOptions {
    pub fn from_prompt(prompt: &viboceros_command::ObjectSelectionPrompt) -> Self {
        Self {
            coplanar: prompt
                .choices
                .iter()
                .find(|p| p.name == "CoplanarFaces")
                .map_or("Keep", |p| p.value),
            ngons: prompt
                .options
                .iter()
                .find(|p| p.name == "CreateNgons")
                .is_none_or(|p| p.value),
            discard: prompt
                .options
                .iter()
                .find(|p| p.name == "DiscardUnsplitMeshes")
                .is_none_or(|p| p.value),
            permit: false,
            planes: vec![],
            plane_getter: None,
            pending_choice: None,
            trim_history: None,
        }
    }
    pub fn suffix(&self, display: bool) -> String {
        let mut result = format!(
            " CreateNgons={} DiscardUnsplitMeshes={}",
            if self.ngons { "Yes" } else { "No" },
            if self.discard { "Yes" } else { "No" }
        );
        // Keep may display the intersection command's Mesh mode. Only an
        // explicitly accepted Keep answer changes that shared output mode.
        if display {
            result.push_str(&format!(" CoplanarFaces={}", self.coplanar));
        }
        if self.permit {
            result.push_str(" PermitSplittingInputsWithEachOther");
        }
        for plane in &self.planes {
            result.push_str(&format!(" InfinitePlane={plane}"));
        }
        result
    }
}
impl VibocerosApp {
    pub(in crate::app) fn continue_mesh_partition_option(
        &mut self,
        mut prompt: TwoSetsPrompt,
        input: &str,
    ) -> bool {
        let word = input.trim_start_matches(['_', '-']);
        let options = prompt.boolean.as_mut().unwrap();
        let partition = options.partition.as_mut().unwrap();
        if word.eq_ignore_ascii_case("PermitSplittingInputsWithEachOther") {
            if prompt.first.as_ref().is_none_or(|f| f.len() < 2) {
                self.push_log("Select multiple targets before permitting shared cutters".into());
            } else {
                partition.permit = true;
                self.document.clear_selection();
            }
        } else {
            let words = word.split_whitespace().collect::<Vec<_>>();
            let (name, value) = if let Some(pending) = partition.pending_choice {
                (pending, Some(word))
            } else {
                let token = words.first().copied().unwrap_or("");
                let (name, value) = token
                    .split_once('=')
                    .map_or((token, words.get(1).copied()), |(n, v)| (n, Some(v)));
                let name = match name.to_ascii_lowercase().as_str() {
                    "tolerance" => "Tolerance",
                    "coplanarfaces" => "CoplanarFaces",
                    "createngons" => "CreateNgons",
                    "discardunsplitmeshes" => "DiscardUnsplitMeshes",
                    _ => return false,
                };
                (name, value)
            };
            let value = if let Some(value) = value {
                value.trim_start_matches('_').to_owned()
            } else if name == "Tolerance" || name == "CoplanarFaces" {
                partition.pending_choice = Some(name);
                self.push_log(
                    if name == "Tolerance" {
                        "Tolerance: Auto, Document or Zero"
                    } else {
                        "CoplanarFaces: Drop or Keep"
                    }
                    .into(),
                );
                self.intersection_prompt = Some(prompt);
                self.command_input.clear();
                return true;
            } else {
                let current = if name == "CreateNgons" {
                    partition.ngons
                } else {
                    partition.discard
                };
                if current { "No" } else { "Yes" }.into()
            };
            let command =
                if partition.pending_choice.is_none() && (word.contains('=') || words.len() > 1) {
                    format!("MeshSplit {}", word.replace('_', ""))
                } else {
                    format!("MeshSplit {name}={value}")
                };
            match self.commands.accept_object_selection_input(&command) {
                Ok(()) => {
                    let description = self
                        .commands
                        .object_selection_prompt("MeshSplit")
                        .unwrap()
                        .unwrap();
                    let updated = PartitionOptions::from_prompt(&description);
                    partition.coplanar = updated.coplanar;
                    partition.ngons = updated.ngons;
                    partition.discard = updated.discard;
                    partition.pending_choice = None;
                    options.kind = options
                        .kind
                        .with_mesh_tolerance(description.choices[0].value);
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.intersection_prompt = Some(prompt);
        self.command_input.clear();
        self.log_intersection_prompt();
        true
    }
}
