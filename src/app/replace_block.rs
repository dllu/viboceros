//! Read-only target picking and definition choice for ReplaceBlock.
use super::*;
use blocks::PendingBlock;
use viboceros_document::BlockDefinitionId;

#[derive(Clone, Debug)]
pub(super) struct ReplaceBlockInput {
    sources: Vec<ObjectId>,
    all: bool,
    name_entry: bool,
    chooser: Option<DefinitionChooser>,
}

#[derive(Clone, Debug, Default)]
struct DefinitionChooser {
    filter: String,
    selected: Option<BlockDefinitionId>,
}

impl ReplaceBlockInput {
    pub(super) fn new(sources: Vec<ObjectId>, all: bool) -> Self {
        Self {
            sources,
            all,
            name_entry: false,
            chooser: None,
        }
    }
}

impl VibocerosApp {
    fn replace_block_input(&self) -> Option<&ReplaceBlockInput> {
        match (&self.block_session, self.active_command) {
            (Some(PendingBlock::Replace(input)), Some(InteractiveCommand::ReplaceBlock)) => {
                Some(input)
            }
            _ => None,
        }
    }

    pub(super) fn replacing_block(&self) -> bool {
        self.replace_block_input().is_some()
    }

    pub(super) fn picking_replace_block(&self) -> bool {
        self.replace_block_input()
            .is_some_and(|input| !input.name_entry && input.chooser.is_none())
    }

    pub(super) fn log_replace_block_peers(&mut self) {
        let Some(input) = self.replace_block_input() else {
            return;
        };
        let Some(Geometry::BlockInstance(first)) = input
            .sources
            .first()
            .and_then(|id| self.document.object(*id))
            .map(|o| o.geometry())
        else {
            return;
        };
        let definition = first.reference().definition();
        let sources = input
            .sources
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let count = self.document.objects().filter(|o|
            !sources.contains(&o.id()) && matches!(o.geometry(), Geometry::BlockInstance(i) if i.reference().definition() == definition)
        ).count();
        if count != 0 {
            self.push_log(format!("{count} additional instance(s); All includes them, None keeps only the selected sources"));
        }
    }

    pub(super) fn accept_replace_block_definition(&mut self, target: BlockDefinitionId) -> bool {
        let Some(input) = self.replace_block_input().cloned() else {
            return false;
        };
        let Some(definition) = self.document.block_definition(target) else {
            self.push_log("Error: replacement definition was removed".into());
            return false;
        };
        let name = definition.name().to_owned();
        match self
            .document
            .replace_block_instances(target, input.sources, input.all)
        {
            Ok(count) => {
                self.push_log(format!("Replaced {count} instance(s) with '{name}'"));
                self.cancel_interactive_command(false);
                true
            }
            Err(error) => {
                self.push_log(format!("Error: {error}"));
                false
            }
        }
    }

    /// Consume target clicks without selecting the target or its group peers.
    pub(super) fn pick_replace_block(&mut self, id: Option<ObjectId>) -> bool {
        if !self.replacing_block() {
            return false;
        }
        if !self.picking_replace_block() {
            return true;
        }
        if let Some(id) = id {
            let target = self.document.object(id).and_then(|o| match o.geometry() {
                Geometry::BlockInstance(i) if self.document.is_object_selectable(id) => {
                    Some(i.reference().definition())
                }
                _ => None,
            });
            if let Some(target) = target {
                self.accept_replace_block_definition(target);
            } else {
                self.push_log("Pick a visible, unlocked block instance".into());
            }
        }
        true
    }

    pub(super) fn continue_replace_block(&mut self, text: &str) -> bool {
        let Some(mut input) = self.replace_block_input().cloned() else {
            return false;
        };
        if (!input.name_entry
            && text
                .split_whitespace()
                .next()
                .is_some_and(|w| self.commands.recognizes(w)))
            || text
                .trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Cancel")
        {
            return false;
        }
        let keyword = text.trim().trim_start_matches('_');
        if !input.name_entry && keyword.eq_ignore_ascii_case("SelectFromBlockDefinitionList") {
            input.chooser = Some(DefinitionChooser::default());
            input.name_entry = false;
            self.block_session = Some(PendingBlock::Replace(input));
        } else if !input.name_entry && keyword.eq_ignore_ascii_case("BlockDefinitionName") {
            input.name_entry = true;
            input.chooser = None;
            self.block_session = Some(PendingBlock::Replace(input));
            self.push_log(
                "ReplaceBlock: enter a definition name; quote names containing spaces".into(),
            );
        } else {
            let parsed = viboceros_command::blocks::tokenize(text).and_then(|words| {
                if input.name_entry {
                    return if words.len() == 1 && !words[0].is_empty() {
                        Ok((None, Some(words[0])))
                    } else {
                        Err(viboceros_command::CommandError::Usage(
                            "enter one definition name; quote names containing spaces",
                        ))
                    };
                }
                let (all, target) = viboceros_command::replace_block::options(&words)?;
                let scope = words.iter().any(|w| {
                    let w = w.trim_start_matches('_');
                    w.eq_ignore_ascii_case("All") || w.eq_ignore_ascii_case("None")
                });
                Ok((scope.then_some(all), target))
            });
            match parsed {
                Ok((all, name)) => {
                    if let Some(all) = all {
                        input.all = all;
                    }
                    self.block_session = Some(PendingBlock::Replace(input));
                    if let Some(name) = name {
                        if let Some(definition) = self.document.block_definition_by_name(name) {
                            self.accept_replace_block_definition(definition.id());
                        } else {
                            self.push_log(format!(
                                "Error: block definition '{name}' was not found"
                            ));
                        }
                    }
                }
                Err(error) => self.push_log(format!("Error: {error}")),
            }
        }
        self.command_input.clear();
        true
    }

    pub(super) fn show_replace_block_choices(&mut self, ui: &mut egui::Ui) {
        let Some(input) = self.replace_block_input() else {
            return;
        };
        let all = input.all;
        let mut choice = None;
        ui.horizontal_wrapped(|ui| {
            for (label, selected) in [
                ("All", all),
                ("None", !all),
                ("SelectFromBlockDefinitionList", false),
                ("BlockDefinitionName", false),
            ] {
                if ui.selectable_label(selected, label).clicked() {
                    choice = Some(label);
                }
            }
        });
        if let Some(choice) = choice {
            if let Some(PendingBlock::Replace(input)) = &mut self.block_session {
                if choice == "All" || choice == "None" {
                    input.all = choice == "All";
                    return;
                }
                input.name_entry = false;
            }
            self.continue_replace_block(choice);
        }
    }

    pub(super) fn show_replace_block_chooser(&mut self, ctx: &egui::Context) {
        if !self.replacing_block() {
            return;
        }
        let Some(PendingBlock::Replace(input)) = &mut self.block_session else {
            return;
        };
        let Some(chooser) = &mut input.chooser else {
            return;
        };
        if chooser
            .selected
            .is_some_and(|id| self.document.block_definition(id).is_none())
        {
            chooser.selected = None;
        }
        let mut rows = self.document.block_definitions().collect::<Vec<_>>();
        rows.sort_by(|a, b| block_manager::natural_compare(a.name(), b.name()));
        let mut open = true;
        let mut accept = None;
        let mut cancel = false;
        egui::Window::new("Replacement block definition")
            .id(egui::Id::new("replace-block-definition-list"))
            .open(&mut open)
            .collapsible(false)
            .default_width(420.)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Find");
                    ui.text_edit_singleline(&mut chooser.filter);
                });
                let filter = chooser.filter.to_lowercase();
                let mut shown = 0;
                egui::ScrollArea::vertical()
                    .max_height(320.)
                    .show(ui, |ui| {
                        for definition in rows {
                            if !definition.name().to_lowercase().contains(&filter) {
                                continue;
                            }
                            shown += 1;
                            let response = ui.selectable_label(
                                chooser.selected == Some(definition.id()),
                                definition.name(),
                            );
                            if response.clicked() {
                                chooser.selected = Some(definition.id());
                            }
                            if response.double_clicked() {
                                accept = Some(definition.id());
                            }
                        }
                    });
                // A filtered-out selection must not be accepted accidentally.
                let selected = chooser.selected.filter(|id| {
                    self.document
                        .block_definition(*id)
                        .is_some_and(|d| d.name().to_lowercase().contains(&filter))
                });
                if shown == 0 {
                    ui.label("No matching block definitions");
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(selected.is_some(), egui::Button::new("Replace"))
                        .clicked()
                    {
                        accept = selected;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if cancel || !open {
            self.cancel_interactive_command(true);
        } else if let Some(target) = accept {
            self.accept_replace_block_definition(target);
        }
    }
}
