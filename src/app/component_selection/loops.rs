//! Double clicks reuse the first click's selection state, outside model history.
use super::*;
use std::collections::BTreeSet;

#[derive(Debug)]
pub(super) struct Click {
    pick: ComponentPick,
    source: GeometrySnapshot,
    before: Vec<ComponentPick>,
    after: BTreeSet<ComponentPick>,
    remove: bool,
}
fn eligible(p: ComponentPick) -> bool {
    matches!(
        p.kind,
        ComponentSelectionKind::MeshEdge | ComponentSelectionKind::SubDEdge
    )
}

impl VibocerosApp {
    fn loop_picking(&self) -> bool {
        self.component_preselection_available() || self.picking_nurbs_edges()
    }
    pub(super) fn remember_loop_click(&mut self, picks: &[ComponentPick], action: ComponentAction) {
        self.component_selection.loop_click = None;
        let [pick] = picks else { return };
        if !eligible(*pick) || !self.loop_picking() {
            return;
        }
        let Ok(before) = self.component_selection.checked_picks(&self.document) else {
            return;
        };
        let Some(obj) = self.document.object(pick.object) else {
            return;
        };
        let mut after = before.iter().copied().collect::<BTreeSet<_>>();
        let remove = matches!(action, ComponentAction::Remove)
            || matches!(action, ComponentAction::Toggle) && after.contains(pick);
        if remove {
            after.remove(pick);
        } else {
            after.insert(*pick);
        }
        self.component_selection.loop_click = Some(Click {
            pick: *pick,
            source: obj.geometry_snapshot().clone(),
            before,
            after,
            remove,
        });
    }
    pub(in crate::app) fn accept_component_loop_click(&mut self, mut click: ComponentClick) {
        if !self.loop_picking() || click.modifiers.alt {
            self.accept_component_click(click);
            return;
        }
        click.picks.retain(|p| eligible(*p));
        if click.picks.len() != 1 {
            self.accept_component_click(click.clone());
            if self.component_selection.has_choices() {
                self.component_selection.loop_choice = Some(click);
            }
            return;
        }
        let pick = click.picks[0];
        let result = (|| -> Result<_, String> {
            let now = self
                .component_selection
                .checked_picks(&self.document)
                .map_err(str::to_owned)?;
            let object = self
                .document
                .object(pick.object)
                .filter(|_| self.document.is_object_selectable(pick.object))
                .ok_or("loop source unavailable")?;
            let (before, remove) = if let Some(previous) = self
                .component_selection
                .loop_click
                .take()
                .filter(|c| c.pick == pick)
            {
                if previous.source != *object.geometry_snapshot()
                    || previous.after != now.iter().copied().collect()
                {
                    return Err("loop input changed; pick the edge again".into());
                }
                (previous.before, previous.remove)
            } else {
                let remove = if click.preselection || click.modifiers.shift {
                    now.contains(&pick)
                } else {
                    click.modifiers.ctrl || click.modifiers.command
                };
                (now, remove)
            };
            let (path, closed) = match object.geometry() {
                Geometry::Mesh(m) => {
                    let p = m.try_edge_loop(pick.index).map_err(|e| e.to_string())?;
                    let closed = p.is_closed();
                    (p.edges, closed)
                }
                Geometry::SubD(s) => {
                    let p = s
                        .try_edge_loop(
                            pick.index,
                            viboceros_geometry::SubDEdgeChainType::EqualEdgeTagAndOrdinary,
                        )
                        .map_err(|e| e.to_string())?;
                    let closed = p.is_closed();
                    (p.edges.into_iter().map(|e| e.index).collect(), closed)
                }
                _ => return Err("mesh or SubD edge required".into()),
            };
            let selected = before
                .iter()
                .filter(|p| p.object == pick.object && p.kind == pick.kind)
                .map(|p| p.index)
                .collect();
            let indices = range(&path, pick.index, closed, &selected, remove);
            Ok((
                indices
                    .into_iter()
                    .map(|index| ComponentPick { index, ..pick })
                    .collect::<Vec<_>>(),
                remove,
            ))
        })();
        match result {
            Ok((picks, remove)) => {
                self.select_components(
                    picks,
                    if remove {
                        ComponentAction::Remove
                    } else {
                        ComponentAction::Add
                    },
                );
                self.component_selection.loop_click = None;
                self.component_selection
                    .shared
                    .order_edges(pick.object, pick.kind);
                self.renew_nurbs_edge_order(pick.object);
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
}

/// Clip additions between two anchors; remove only the selected connected run.
fn range(
    path: &[usize],
    seed: usize,
    closed: bool,
    selected: &BTreeSet<usize>,
    remove: bool,
) -> Vec<usize> {
    let Some(at) = path.iter().position(|&i| i == seed) else {
        return Vec::new();
    };
    let n = path.len();
    let walk = |forward: bool| {
        let mut side = Vec::new();
        let mut i = at;
        for _ in 1..n {
            i = if forward {
                if i + 1 == n {
                    if !closed {
                        break;
                    }
                    0
                } else {
                    i + 1
                }
            } else if i == 0 {
                if !closed {
                    break;
                }
                n - 1
            } else {
                i - 1
            };
            if remove && !selected.contains(&path[i]) {
                break;
            }
            side.push(i);
            if !remove && selected.contains(&path[i]) {
                break;
            }
        }
        side
    };
    if remove && !selected.contains(&seed) {
        return Vec::new();
    }
    let left = walk(false);
    let right = walk(true);
    let keep = left
        .into_iter()
        .chain([at])
        .chain(right)
        .collect::<BTreeSet<_>>();
    path.iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, &e)| e)
        .collect()
}
