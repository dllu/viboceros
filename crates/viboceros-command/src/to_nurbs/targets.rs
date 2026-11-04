//! Owned object and edge targets, with native source and component ordering.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ToNurbsTarget {
    Whole(ObjectId),
    Edge(ObjectId, usize),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToNurbsContext {
    pub postselected: bool,
    /// SubD appearance when the input was accepted, before option editing.
    pub control_net: bool,
}
impl ToNurbsTarget {
    pub fn object(self) -> ObjectId {
        match self {
            Self::Whole(id) | Self::Edge(id, _) => id,
        }
    }
}
pub(super) struct Input {
    pub order: Vec<ToNurbsTarget>,
    pub whole: BTreeSet<ObjectId>,
    pub kinds: Kinds,
}
impl Input {
    pub fn prepare(
        doc: &Document,
        targets: Option<&[ToNurbsTarget]>,
        post: bool,
    ) -> Result<Self, CommandError> {
        if targets.is_some_and(|t| t.len() > 100_000) {
            return Err(CommandError::ToNurbsTargetUnavailable);
        }
        let targets = targets.map_or_else(
            || {
                doc.selected_object_ids()
                    .map(ToNurbsTarget::Whole)
                    .collect::<Vec<_>>()
            },
            <[ToNurbsTarget]>::to_vec,
        );
        if targets.len() > 100_000 {
            return Err(CommandError::ToNurbsTargetUnavailable);
        }
        let whole = targets
            .iter()
            .filter_map(|t| {
                if let ToNurbsTarget::Whole(id) = t {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        let mut kinds = Kinds::default();
        let mut counts = BTreeMap::new();
        for target in targets {
            if !seen.insert(target) {
                continue;
            }
            let obj = doc
                .object(target.object())
                .filter(|_| doc.is_object_selectable(target.object()))
                .ok_or(CommandError::ToNurbsTargetUnavailable)?;
            match target {
                ToNurbsTarget::Whole(_) => {
                    kinds.mesh |= matches!(obj.geometry(), Geometry::Mesh(_));
                    kinds.subd |= matches!(obj.geometry(), Geometry::SubD(_));
                }
                ToNurbsTarget::Edge(id, index) => {
                    if whole.contains(&id) {
                        continue;
                    }
                    let count = if let Some(&count) = counts.get(&id) {
                        count
                    } else {
                        let count = match obj.geometry() {
                            Geometry::Mesh(m) => m.topology_edge_points().len(),
                            Geometry::SubD(s) => s.edges().len(),
                            _ => return Err(CommandError::ToNurbsTargetUnavailable),
                        };
                        counts.insert(id, count);
                        count
                    };
                    if index >= count {
                        return Err(CommandError::ToNurbsTargetUnavailable);
                    }
                }
            }
            order.push(target);
        }
        if order.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        if !post {
            let rank = doc
                .objects()
                .enumerate()
                .map(|(i, o)| (o.id(), i))
                .collect::<BTreeMap<_, _>>();
            order.sort_by_key(|t| rank[&t.object()]);
        }
        Ok(Self {
            order,
            whole,
            kinds,
        })
    }
}

/// Typed component API uses the same transaction and command preferences.
impl CommandRegistry {
    pub fn to_nurbs_target_confirmation(
        &self,
        doc: &Document,
        input: &str,
        targets: &[ToNurbsTarget],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let mut args = input.split_whitespace();
        let name = args.next().ok_or(CommandError::EmptyInput)?;
        if normalize_command_name(name) != "tonurbs" {
            return Err(CommandError::ToNurbsTargetUnavailable);
        }
        let index = self
            .lookup
            .get("tonurbs")
            .ok_or(CommandError::ToNurbsTargetUnavailable)?;
        let command = &self.commands[*index];
        command.to_nurbs_target_confirmation(doc, &args.collect::<Vec<_>>(), targets)
    }
    pub fn execute_to_nurbs_targets(
        &self,
        doc: &mut Document,
        input: &str,
        targets: &[ToNurbsTarget],
        postselected: bool,
    ) -> Result<String, CommandError> {
        let context = ToNurbsContext {
            postselected,
            control_net: doc.subd_control_net(),
        };
        self.execute_to_nurbs_targets_in_context(doc, input, targets, context)
    }
    pub fn execute_to_nurbs_targets_in_context(
        &self,
        doc: &mut Document,
        input: &str,
        targets: &[ToNurbsTarget],
        context: ToNurbsContext,
    ) -> Result<String, CommandError> {
        let mut args = input.split_whitespace();
        let name = args.next().ok_or(CommandError::EmptyInput)?;
        if normalize_command_name(name) != "tonurbs" {
            return Err(CommandError::ToNurbsTargetUnavailable);
        }
        let index = self
            .lookup
            .get("tonurbs")
            .ok_or(CommandError::ToNurbsTargetUnavailable)?;
        let command = &self.commands[*index];
        let args = args.collect::<Vec<_>>();
        run_command_transaction(doc, "ToNURBS", |doc| {
            command.run_to_nurbs_targets(doc, &args, targets, context)
        })
    }
}

pub(super) fn split<'a>(
    args: &'a [&'a str],
) -> Result<(Vec<&'a str>, Option<Selectors<'a>>), CommandError> {
    let mut rest = Vec::new();
    let (mut edges, mut object) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let name = args[i].split('=').next().unwrap();
        if option_name_eq(name, "Edge")
            || option_name_eq(name, "Edges")
            || option_name_eq(name, "Object")
        {
            let (name, value, n) = orient_option(args, i, options::USAGE)?;
            let slot = if option_name_eq(name, "Object") {
                &mut object
            } else {
                &mut edges
            };
            if slot.replace(value).is_some() {
                return Err(CommandError::Usage(options::USAGE));
            }
            i += n;
        } else {
            rest.push(args[i]);
            i += 1;
        }
    }
    match (edges, object) {
        (None, None) => Ok((rest, None)),
        (Some(edges), object) => Ok((rest, Some(Selectors { edges, object }))),
        _ => Err(CommandError::Usage(options::USAGE)),
    }
}
pub(super) struct Selectors<'a> {
    edges: &'a str,
    object: Option<&'a str>,
}
impl Selectors<'_> {
    pub fn resolve(self, doc: &Document) -> Result<Vec<ToNurbsTarget>, CommandError> {
        let id = if let Some(id) = self.object {
            id.parse::<ObjectId>()
                .map_err(|_| CommandError::ToNurbsTargetUnavailable)?
        } else {
            let mut selected = doc.selected_objects();
            let id = selected.next().ok_or(CommandError::NoObjectsSelected)?.id();
            if selected.next().is_some() {
                return Err(CommandError::ToNurbsTargetUnavailable);
            }
            id
        };
        let obj = doc
            .object(id)
            .filter(|_| doc.is_object_selectable(id))
            .ok_or(CommandError::ToNurbsTargetUnavailable)?;
        let count = match obj.geometry() {
            Geometry::Mesh(m) => m.topology_edge_points().len(),
            Geometry::SubD(s) => s.edges().len(),
            _ => return Err(CommandError::ToNurbsTargetUnavailable),
        };
        let indices = if option_name_eq(self.edges, "All") {
            if count > 100_000 {
                return Err(CommandError::ToNurbsTargetUnavailable);
            }
            (0..count).collect::<Vec<_>>()
        } else {
            if self.edges.split(',').count() > 100_000 {
                return Err(CommandError::ToNurbsTargetUnavailable);
            }
            self.edges
                .split(',')
                .map(|v| {
                    v.parse::<usize>()
                        .map_err(|_| CommandError::ToNurbsTargetUnavailable)
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(indices
            .into_iter()
            .map(|i| ToNurbsTarget::Edge(id, i))
            .collect())
    }
}
