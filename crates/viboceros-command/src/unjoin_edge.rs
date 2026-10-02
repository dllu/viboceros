//! Atomic multi-source edge separation with exact source identities.
use super::*;
use viboceros_document::{GeometrySnapshot, ReplacementHistory};
#[cfg(test)]
mod tests;
const USAGE: &str =
    "UnjoinEdge object-id edge-index[,edge-index...] [object-id edge-index[,edge-index...] ...]";
#[derive(Clone, Debug)]
struct Plan {
    object: ObjectId,
    source: GeometrySnapshot,
    parts: Vec<Brep>,
}
#[derive(Clone, Debug)]
pub struct UnjoinEdgeSelection {
    plans: Vec<Plan>,
    tolerance: Tolerance,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnjoinEdgeResult {
    pub updated: Vec<ObjectId>,
    pub added: Vec<ObjectId>,
}
impl UnjoinEdgeSelection {
    /// Read-only preparation; all indices and sources are validated before any
    /// geometry is changed. Duplicate edge references are accepted once.
    pub fn prepare(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
    ) -> Result<Self, CommandError> {
        Self::prepare_with_order(document, picks, false)
    }
    /// Preselection enumerates edge tables, rather than click order.
    pub fn prepare_preselected(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
    ) -> Result<Self, CommandError> {
        Self::prepare_with_order(document, picks, true)
    }
    fn prepare_with_order(
        document: &Document,
        picks: impl IntoIterator<Item = (ObjectId, usize)>,
        preselected: bool,
    ) -> Result<Self, CommandError> {
        let mut selected = BTreeMap::<ObjectId, Vec<usize>>::new();
        for (count, (object, edge)) in picks.into_iter().enumerate() {
            if count >= 100_000 {
                return Err(CommandError::Usage(USAGE));
            }
            if !document.is_object_selectable(object) {
                return Err(CommandError::UnjoinEdgeUnavailable);
            }
            selected.entry(object).or_default().push(edge);
        }
        let mut plans = Vec::with_capacity(selected.len());
        for object in document.objects() {
            if !selected.contains_key(&object.id()) {
                continue;
            }
            let converted;
            let brep = match object.geometry() {
                Geometry::Brep(brep) => brep,
                Geometry::NurbsSurface(surface) => {
                    converted = Brep::try_surface_face_with_native_edge_parameters(
                        surface.clone(),
                        document.tolerance(),
                    )?;
                    &converted
                }
                _ => return Err(CommandError::UnjoinEdgeUnavailable),
            };
            let mut edges = selected.remove(&object.id()).unwrap();
            // Validate even references that would be filtered out. Deduplicate
            // before applying the command's measured ordering policy.
            let mut seen = vec![false; brep.edges().len()];
            for &edge in &edges {
                if edge >= seen.len() {
                    return Err(CommandError::Usage(USAGE));
                }
            }
            let joined = brep.edges_shared_by_distinct_faces();
            edges.retain(|&edge| joined[edge] && !std::mem::replace(&mut seen[edge], true));
            if preselected {
                edges.sort_unstable_by(|a, b| b.cmp(a));
            }
            // Native command batches rotate equal-parent references for at
            // most eight picks. Larger batches keep their gathered order.
            // The geometry API separately preserves supplied edge order.
            if edges.len() <= 8 && !edges.is_empty() {
                edges.rotate_left(1);
            }
            let parts = brep.try_unjoin_edges(&edges, document.tolerance())?;
            plans.push(Plan {
                object: object.id(),
                source: object.geometry_snapshot().clone(),
                parts,
            });
        }
        Ok(Self {
            plans,
            tolerance: document.tolerance(),
        })
    }
    pub fn changes_geometry(&self) -> bool {
        self.plans.iter().any(|plan| !plan.parts.is_empty())
    }
    pub fn validate_source(&self, document: &Document) -> Result<(), CommandError> {
        if document.tolerance() != self.tolerance
            || self.plans.iter().any(|plan| {
                !document.is_object_selectable(plan.object)
                    || document
                        .object(plan.object)
                        .is_none_or(|object| object.geometry_snapshot() != &plan.source)
            })
        {
            return Err(CommandError::UnjoinEdgeStale);
        }
        Ok(())
    }
    pub fn commit(&self, document: &mut Document) -> Result<UnjoinEdgeResult, CommandError> {
        run_command_transaction(document, "UnjoinEdge", |doc| self.apply(doc))
    }
    fn apply(&self, document: &mut Document) -> Result<UnjoinEdgeResult, CommandError> {
        self.validate_source(document)?;
        let mut result = UnjoinEdgeResult::default();
        document.clear_selection();
        for plan in &self.plans {
            let Some(first) = plan.parts.first() else {
                continue;
            };
            let source = document.object(plan.object).unwrap();
            let attrs = source.attributes().clone();
            let groups = source.group_ids().to_vec();
            let text = source.geometry_user_text().clone();
            document.replace_object_geometries_with_history(
                [(plan.object, Geometry::Brep(first.clone()))],
                ReplacementHistory::EveryReplacement,
            )?;
            let mut order = vec![plan.object];
            result.updated.push(plan.object);
            for part in &plan.parts[1..] {
                let id = document
                    .add_geometry_with_attributes(Geometry::Brep(part.clone()), attrs.clone())?;
                document.set_object_group_memberships(id, groups.clone())?;
                for (key, value) in &text {
                    document.set_object_geometry_user_text([id], key, Some(value))?;
                }
                result.added.push(id);
                order.push(id);
            }
            document.move_objects_to_end_in_order(order)?;
        }
        Ok(result)
    }
}
pub(super) struct UnjoinEdgeCommand;
impl Command for UnjoinEdgeCommand {
    fn name(&self) -> &'static str {
        "UnjoinEdge"
    }
    fn component_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ComponentSelectionPrompt>, CommandError> {
        Ok(arguments.is_empty().then_some(ComponentSelectionPrompt {
            command: "UnjoinEdge",
            kind: ComponentSelectionKind::BrepEdge,
            options: Vec::new(),
            numbers: Vec::new(),
        }))
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.is_empty() || !arguments.len().is_multiple_of(2) {
            return Err(CommandError::Usage(USAGE));
        }
        let mut picks = Vec::new();
        for pair in arguments.chunks_exact(2) {
            let object = pair[0]
                .parse::<ObjectId>()
                .map_err(|_| CommandError::Usage(USAGE))?;
            for edge in pair[1].split(',') {
                picks.push((
                    object,
                    edge.parse::<usize>()
                        .map_err(|_| CommandError::Usage(USAGE))?,
                ));
            }
        }
        let result = UnjoinEdgeSelection::prepare(document, picks)?.apply(document)?;
        Ok(format!(
            "Separated {} source(s), added {} component(s)",
            result.updated.len(),
            result.added.len()
        ))
    }
}
