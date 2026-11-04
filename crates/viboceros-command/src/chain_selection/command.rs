use super::*;
pub(crate) struct SelChainCommand {
    selection: component_selection::ComponentSelection,
    preferences: Preferences,
}
impl SelChainCommand {
    pub fn new(
        selection: component_selection::ComponentSelection,
        preferences: Preferences,
    ) -> Self {
        Self {
            selection,
            preferences,
        }
    }
}
impl Command for SelChainCommand {
    fn name(&self) -> &'static str {
        "SelChain"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (options, seed) = arguments::script(
            args,
            self.preferences.get().unwrap_or_else(|| defaults(doc)),
            doc.tolerance(),
        )?;
        let mut session = Session::new(doc, &self.selection, options)?;
        session.pick(doc, seed)?;
        let _ = session.enter(doc)?;
        session.apply(doc, &self.selection)?;
        self.preferences.set(Some(options));
        Ok(format!(
            "Selected {} chain segment(s)",
            session.targets().len()
        ))
    }
}

impl CommandRegistry {
    pub fn chain_selection_options(&self, doc: &Document) -> CurveChainOptions {
        self.chain_options.get().unwrap_or_else(|| defaults(doc))
    }
    pub fn remember_chain_selection_options(
        &self,
        options: CurveChainOptions,
    ) -> Result<(), CommandError> {
        options.validate()?;
        self.chain_options.set(Some(options));
        Ok(())
    }
}
