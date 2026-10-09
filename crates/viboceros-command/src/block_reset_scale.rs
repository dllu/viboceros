//! Remembered instance scale mode and the shared block selection getter.
use super::*;
use viboceros_document::BlockScaleResetMode;

#[derive(Default)]
pub(super) struct BlockResetScaleCommand(remembered::Remembered<BlockScaleResetMode>);
const USAGE: &str = "BlockResetScale [Mode=One|Automatic]";
fn parse(args: &[&str], default: BlockScaleResetMode) -> Result<BlockScaleResetMode, CommandError> {
    let value = match args {
        [] => return Ok(default),
        [value] => value
            .split_once('=')
            .filter(|(key, _)| key.trim_start_matches('_').eq_ignore_ascii_case("Mode"))
            .map(|(_, value)| value)
            .ok_or(CommandError::Usage(USAGE))?,
        [key, value] if key.trim_start_matches('_').eq_ignore_ascii_case("Mode") => value,
        _ => return Err(CommandError::Usage(USAGE)),
    };
    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "one" => Ok(BlockScaleResetMode::One),
        "automatic" => Ok(BlockScaleResetMode::Automatic),
        _ => Err(CommandError::Usage(USAGE)),
    }
}
fn token(mode: BlockScaleResetMode) -> &'static str {
    match mode {
        BlockScaleResetMode::One => "One",
        BlockScaleResetMode::Automatic => "Automatic",
    }
}
impl Command for BlockResetScaleCommand {
    fn name(&self) -> &'static str {
        "BlockResetScale"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let mode = parse(args, self.0.get())?;
        Ok(Some(ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Blocks,
            options: vec![],
            menus: vec![],
            choices: vec![ChoiceSelectionOption {
                name: "Mode",
                value: token(mode),
                choices: &["One", "Automatic"],
                toggle: None,
            }],
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn accept_object_selection_options(&self, args: &[&str]) -> Result<(), CommandError> {
        let mode = parse(args, self.0.get())?;
        self.0.set(mode);
        Ok(())
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mode = parse(args, self.0.get())?;
        self.0.set(mode);
        let ids = doc
            .selected_objects()
            .filter(|o| matches!(o.geometry(), Geometry::BlockInstance(_)))
            .map(|o| o.id())
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let count = doc.reset_block_scale(ids, mode)?;
        Ok(format!(
            "Reset scale on {count} block instance(s) using {}",
            token(mode)
        ))
    }
}

#[cfg(test)]
mod tests;
