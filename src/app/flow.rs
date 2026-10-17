//! Two immutable guides, a shared preview, and one staged Flow edit.
use super::*;
use std::{cell::RefCell, sync::Arc};
use viboceros_command::flow::{self as solver, GuideReference, Options};
use viboceros_geometry::CurveFlowPointMorph;

struct Guide {
    reference: GuideReference,
    parent: viboceros_document::GeometrySnapshot,
    tolerance: Tolerance,
}
impl Guide {
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.reference.id)
            && doc
                .object(self.reference.id)
                .is_some_and(|o| self.parent.shares_storage_with(o.geometry_snapshot()))
    }
}
pub(super) struct Session {
    sources: Vec<ObjectId>,
    grips: Vec<viboceros_document::ControlPointId>,
    postselected: bool,
    options: Options,
    base: Option<Guide>,
    target: Option<Guide>,
    morph: Option<Arc<CurveFlowPointMorph>>,
    pick_pointer: Option<(usize, egui::Pos2)>,
    cache: RefCell<crate::viewport::FlowPreviewCache>,
}
impl Session {
    pub(super) fn preview(&self) -> Option<crate::viewport::FlowPreview<'_>> {
        Some(crate::viewport::FlowPreview {
            sources: &self.sources,
            grips: &self.grips,
            morph: self.morph.as_ref()?,
            options: self.options,
            cache: &self.cache,
        })
    }
}
impl VibocerosApp {
    pub(super) fn picking_flow_guide(&self) -> bool {
        self.flow_session
            .as_ref()
            .is_some_and(|s| s.base.is_none() || s.target.is_none())
    }
    pub(super) fn capture_flow_pointer(&mut self, pointer: Option<egui::Pos2>) {
        if self.picking_flow_guide()
            && let Some(s) = &mut self.flow_session
        {
            s.pick_pointer = pointer.map(|p| (self.active_viewport, p));
        }
    }
    pub(super) fn try_start_flow_input(
        &mut self,
        input: &str,
        picked: Option<&[ObjectId]>,
    ) -> bool {
        let mut tokens = input.split_whitespace();
        if !tokens.next().is_some_and(|n| {
            n.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Flow")
        }) {
            return false;
        }
        let args = tokens.collect::<Vec<_>>();
        let Ok((mut options, refs)) = solver::options(&args, self.commands.flow_options()) else {
            return false;
        };
        if refs.base.is_some() && refs.target.is_some() {
            return false;
        }
        self.cancel_interactive_command(false);
        let copy = self.commands.begin_copy_options("Flow").unwrap();
        if !args.iter().any(|a| {
            a.split('=').next().is_some_and(|k| {
                k.trim_start_matches(['_', '-'])
                    .eq_ignore_ascii_case("Copy")
            })
        }) {
            options.copy = copy;
        }
        let sources = picked.map_or_else(
            || self.document.selected_object_ids().collect(),
            |p| p.to_vec(),
        );
        let grips = self
            .document
            .selected_control_points()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        if sources.is_empty() && grips.is_empty() {
            self.start_transform_source_prompt("Flow");
            self.object_prompt.as_mut().unwrap().command_override = Some(input.into());
            return true;
        }
        self.flow_session = Some(Session {
            sources,
            grips,
            postselected: picked.is_some(),
            options,
            base: None,
            target: None,
            morph: None,
            pick_pointer: None,
            cache: RefCell::default(),
        });
        self.active_command = Some(InteractiveCommand::Flow);
        if let Some(r) = refs.base {
            self.pick_flow_guide(Some(r.id), r.edge);
        }
        if let Some(r) = refs.target {
            // A target supplied before the base is saved without changing the getter order.
            match self.capture_flow_guide(r) {
                Ok(g) => self.flow_session.as_mut().unwrap().target = Some(g),
                Err(e) => self.push_log(format!("Error: {e}")),
            }
        }
        self.flow_prompt();
        true
    }
    fn capture_flow_guide(&self, r: GuideReference) -> Result<Guide, String> {
        let s = self.flow_session.as_ref().unwrap();
        if s.sources.contains(&r.id) || s.grips.iter().any(|g| g.object == r.id) {
            return Err("Flow guides must be separate from the source objects".into());
        }
        let o = self
            .document
            .object(r.id)
            .filter(|_| self.document.is_object_selectable(r.id))
            .ok_or("Curve is hidden, locked, or missing")?;
        viboceros_command::curve_reference::resolve(
            o.geometry(),
            r.edge,
            self.document.tolerance(),
        )
        .ok_or("Select a curve or surface edge")?;
        Ok(Guide {
            reference: r,
            parent: o.geometry_snapshot().clone(),
            tolerance: self.document.tolerance(),
        })
    }
    pub(super) fn pick_flow_guide(&mut self, id: Option<ObjectId>, edge: Option<usize>) -> bool {
        if !self.picking_flow_guide() {
            return false;
        }
        let result = id
            .ok_or_else(|| "Select a Flow guide".to_owned())
            .and_then(|id| self.capture_flow_guide(GuideReference { id, edge }));
        match result {
            Ok(g) => {
                let s = self.flow_session.as_mut().unwrap();
                let reverse = s.pick_pointer.take().and_then(|(index, p)| {
                    self.viewports[index].flow_pick_reversed(
                        &g.parent,
                        g.reference.edge,
                        p,
                        self.document.tolerance(),
                    )
                });
                if s.base.is_none() {
                    s.base = Some(g);
                    if let Some(v) = reverse {
                        s.options.reverse_base = v;
                    }
                } else {
                    s.target = Some(g);
                    if let Some(v) = reverse {
                        s.options.reverse_target = v;
                    }
                }
                self.component_selection.clear_choices();
                self.prepare_flow_preview();
                self.flow_prompt();
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
    fn prepare_flow_preview(&mut self) {
        let s = self.flow_session.as_mut().unwrap();
        s.morph = None;
        let (Some(a), Some(b)) = (&s.base, &s.target) else {
            return;
        };
        match solver::prepare(&self.document, a.reference, b.reference, s.options) {
            Ok(m) => s.morph = Some(Arc::new(m)),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
    fn flow_prompt(&mut self) {
        let Some(s) = &self.flow_session else { return };
        let phase = if s.base.is_none() {
            "select the base curve near its start"
        } else if s.target.is_none() {
            "select the target curve near its start"
        } else if s.morph.is_none() {
            "choose supported guides with Base or Target"
        } else {
            "preview; Enter applies, Base or Target selects that guide again"
        };
        self.push_log(format!(
            "Flow: {phase}; {}; Esc cancels",
            s.options.arguments()
        ));
    }
    pub(super) fn validate_flow_guides(&mut self) {
        let Some(s) = &mut self.flow_session else {
            return;
        };
        let base = s.base.as_ref().is_some_and(|g| !g.current(&self.document));
        let target = s
            .target
            .as_ref()
            .is_some_and(|g| !g.current(&self.document));
        if base || target {
            if base {
                s.base = None;
            }
            if target {
                s.target = None;
            }
            s.morph = None;
            self.component_selection.clear_choices();
            self.push_log("Flow guide changed or became unavailable; select it again".into());
            self.flow_prompt();
        }
    }
    pub(super) fn try_continue_flow(&mut self, input: &str) -> bool {
        if self.flow_session.is_none() {
            return false;
        }
        self.validate_flow_guides();
        let word = input.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Cancel") {
            self.cancel_interactive_command(true);
            return true;
        }
        if input.is_empty() || word.eq_ignore_ascii_case("Enter") {
            let s = self.flow_session.as_ref().unwrap();
            if let (Some(a), Some(b), Some(_)) = (&s.base, &s.target, &s.morph) {
                let reference = |name: &str, r: GuideReference| match r.edge {
                    Some(edge) => format!("{name}Edge={},{}", r.id, edge),
                    None => format!("{name}={}", r.id),
                };
                let command = format!(
                    "Flow {} {} {} {}={} {}",
                    reference("Base", a.reference),
                    reference("Target", b.reference),
                    s.options.arguments(),
                    if s.postselected {
                        "PickedSources"
                    } else {
                        "Sources"
                    },
                    s.sources
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    super::transform_sources::grip_argument(&s.grips, s.postselected)
                );
                match self.commands.execute(&mut self.document, &command) {
                    Ok(message) => {
                        self.push_log(message);
                        self.cancel_interactive_command(false);
                    }
                    Err(e) => self.push_log(format!("Error: {e}")),
                }
            } else {
                self.flow_prompt();
            }
            return true;
        }
        if word.eq_ignore_ascii_case("Base") || word.eq_ignore_ascii_case("Target") {
            let s = self.flow_session.as_mut().unwrap();
            if word.eq_ignore_ascii_case("Base") {
                s.base = None;
            } else {
                s.target = None;
            }
            s.morph = None;
            s.pick_pointer = None;
            self.flow_prompt();
            return true;
        }
        let words = input.split_whitespace().collect::<Vec<_>>();
        if self.picking_flow_guide() {
            let r = match words.as_slice() {
                [key, id] if key.trim_start_matches('_').eq_ignore_ascii_case("SelID") => {
                    id.parse().ok().map(|id| GuideReference { id, edge: None })
                }
                [token] => token
                    .parse()
                    .ok()
                    .map(|id| GuideReference { id, edge: None })
                    .or_else(|| {
                        let (key, value) = token.split_once('=')?;
                        if key.eq_ignore_ascii_case("Edge") {
                            viboceros_command::curve_reference::parse_edge(value).map(
                                |(id, edge)| GuideReference {
                                    id,
                                    edge: Some(edge),
                                },
                            )
                        } else {
                            None
                        }
                    }),
                _ => None,
            };
            if let Some(r) = r {
                self.capture_flow_pointer(None);
                return self.pick_flow_guide(Some(r.id), r.edge);
            }
        }
        let prior = self.flow_session.as_ref().unwrap().options;
        let args = if !word.contains(['=', ' ']) {
            let value = match word.to_ascii_lowercase().as_str() {
                "copy" => Some(prior.copy),
                "rigid" => Some(prior.rigid),
                "rigidgroups" => Some(prior.rigid_groups),
                "stretch" => Some(prior.stretch),
                "preservestructure" => Some(prior.preserve_structure),
                "reversebase" => Some(prior.reverse_base),
                "reversetarget" => Some(prior.reverse_target),
                _ => None,
            };
            value
                .map(|v| format!("{word}={}", if v { "No" } else { "Yes" }))
                .unwrap_or_else(|| input.into())
        } else {
            input.into()
        };
        match solver::options(&args.split_whitespace().collect::<Vec<_>>(), prior) {
            Ok((options, refs)) if refs.base.is_none() && refs.target.is_none() => {
                self.flow_session.as_mut().unwrap().options = options;
                if options.geometry() != prior.geometry() {
                    let same_guides = options.reverse_base == prior.reverse_base
                        && options.reverse_target == prior.reverse_target
                        && options.stretch == prior.stretch;
                    let s = self.flow_session.as_mut().unwrap();
                    if same_guides && let Some(morph) = &s.morph {
                        s.morph = Some(Arc::new(
                            (**morph)
                                .clone()
                                .with_preserve_structure(options.preserve_structure),
                        ));
                    } else {
                        self.prepare_flow_preview();
                    }
                }
                self.flow_prompt();
            }
            Ok(_) => self.push_log("Error: select a guide with SelID or Edge=parent,index".into()),
            Err(e) => self.push_log(format!("Error: {e}")),
        }
        true
    }
}
