use super::*;

pub(crate) struct SelEdgePatternCommand {
    pattern: Pattern,
    selection: ComponentSelection,
}
impl SelEdgePatternCommand {
    pub fn new(selection: ComponentSelection, pattern: Pattern) -> Self {
        Self { selection, pattern }
    }
}
impl Command for SelEdgePatternCommand {
    fn name(&self) -> &'static str {
        self.pattern.command()
    }
    fn records_history(&self) -> bool {
        false
    }
    fn component_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        if !args.is_empty() {
            return Err(CommandError::Usage(self.pattern.usage()));
        }
        Ok(Some(ComponentSelectionPrompt {
            command: self.pattern.command(),
            kind: ComponentSelectionKind::MeshEdge,
            options: Vec::new(),
            numbers: Vec::new(),
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mut preserved = Vec::new();
        let targets = if args.is_empty() {
            let picks = self
                .selection
                .checked_picks(doc)
                .map_err(|_| CommandError::EdgeLoopStale)?;
            let seeds = picks
                .iter()
                .copied()
                .filter(|p| eligible(*p))
                .collect::<Vec<_>>();
            if !seeds.is_empty() {
                preserved = picks.into_iter().filter(|p| !eligible(*p)).collect();
            }
            seeds
        } else {
            selectors(doc, args, self.pattern.usage())?
        };
        let mut session = Session::new(self.pattern);
        session.pick(doc, &targets, ComponentAction::Add)?;
        let mut selected = session.finish(doc)?;
        let owners = selected.iter().map(|p| p.object).collect::<BTreeSet<_>>();
        if !selected.is_empty() {
            selected.extend(preserved);
        }
        let mut seen = BTreeSet::new();
        selected.retain(|p| seen.insert(*p));
        let message = self.pattern.completed_message(&selected);
        self.selection
            .replace(doc, selected)
            .map_err(|_| CommandError::EdgeLoopStale)?;
        doc.select_objects_direct(owners, viboceros_document::SelectionMode::Remove)?;
        Ok(message)
    }
}
fn selectors(
    doc: &Document,
    args: &[&str],
    usage: &'static str,
) -> Result<Vec<ComponentPick>, CommandError> {
    let (mut edges, mut object) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let (name, value, n) = orient_option(args, i, usage)?;
        let slot = if option_name_eq(name, "Edges") || option_name_eq(name, "Edge") {
            &mut edges
        } else if option_name_eq(name, "Object") {
            &mut object
        } else {
            return Err(CommandError::Usage(usage));
        };
        if slot.replace(value).is_some() {
            return Err(CommandError::Usage(usage));
        }
        i += n;
    }
    let edges = edges.ok_or(CommandError::Usage(usage))?;
    let id = if let Some(id) = object {
        id.parse::<ObjectId>()
            .map_err(|_| CommandError::EdgeLoopUnavailable)?
    } else {
        let mut objects = doc.selected_object_ids();
        let id = objects.next().ok_or(CommandError::NoObjectsSelected)?;
        if objects.next().is_some() {
            return Err(CommandError::EdgeLoopUnavailable);
        }
        id
    };
    let obj = doc
        .object(id)
        .filter(|_| doc.is_object_selectable(id))
        .ok_or(CommandError::EdgeLoopUnavailable)?;
    let (kind, count) = match obj.geometry() {
        Geometry::Mesh(m) => (ComponentSelectionKind::MeshEdge, m.topology().edge_count()),
        Geometry::SubD(s) => (ComponentSelectionKind::SubDEdge, s.edges().len()),
        _ => return Err(CommandError::EdgeLoopUnavailable),
    };
    let indices = if option_name_eq(edges, "All") {
        if count > MAX_SEEDS {
            return Err(CommandError::EdgeLoopResourceLimit);
        }
        (0..count).collect::<Vec<_>>()
    } else {
        if edges.split(',').count() > MAX_SEEDS {
            return Err(CommandError::EdgeLoopResourceLimit);
        }
        edges
            .split(',')
            .map(|s| {
                s.parse::<usize>()
                    .map_err(|_| CommandError::EdgeLoopUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(indices
        .into_iter()
        .map(|index| ComponentPick {
            object: id,
            kind,
            index,
        })
        .collect())
}
