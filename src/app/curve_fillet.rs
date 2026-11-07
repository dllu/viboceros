//! Ordered source picks and cached Fillet candidates outside model history.
use super::*;
use viboceros_command::fillet::{Preferences, Prepared, parse, parse_radius, prepare};
use viboceros_geometry::Real;
mod input;
mod radius;

pub(super) struct Session {
    picks: Vec<(viboceros_document::Object, Point3)>,
    options: Preferences,
    tolerance: Tolerance,
    layer: viboceros_document::LayerId,
    pub pointer: Option<egui::Pos2>,
    pending: Option<&'static str>,
    radius_start: Option<Point3>,
    preview: Option<Prepared>,
    preview_options: Option<Preferences>,
    scene: Option<Document>,
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.picks.len() < 2 && self.pending.is_none()
    }
    pub(super) fn wants_point(&self) -> bool {
        self.pending == Some("Radius") || self.picks.len() == 2
    }
    pub(super) fn scene(&self) -> Option<&Document> {
        self.scene.as_ref()
    }
    pub(super) fn hint(&self) -> &'static str {
        if self.radius_start.is_some() {
            "Second distance point for radius"
        } else if self.pending == Some("Radius") {
            "Fillet radius or first distance point; Enter keeps the radius"
        } else if self.pending == Some("ExtendArcsBy") {
            "Extend arcs by Arc or Line"
        } else if self.pending == Some("ExtendOtherCurvesBy") {
            "Extend other curves by Line or Smooth"
        } else if self.picks.is_empty() {
            "Select first curve near the end to fillet"
        } else if self.picks.len() == 1 {
            "Select second curve near the end to fillet"
        } else {
            "Radius or first distance point; Join / Trim / ExtendArcsBy / ExtendOtherCurvesBy; Enter accepts"
        }
    }
    fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.layer == doc.current_layer_id()
            && self.picks.iter().all(|(old, _)| {
                doc.is_object_selectable(old.id())
                    && doc.object(old.id()).is_some_and(|new| {
                        old.geometry_snapshot()
                            .shares_storage_with(new.geometry_snapshot())
                            && old.attributes() == new.attributes()
                            && old.group_ids() == new.group_ids()
                            && old.geometry_user_text() == new.geometry_user_text()
                    })
            })
    }
}

impl VibocerosApp {
    pub(super) fn start_curve_fillet(&mut self, text: &str) -> bool {
        let words = text.split_whitespace().collect::<Vec<_>>();
        if words.first().is_none_or(|w| {
            !w.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Fillet")
        }) {
            return false;
        }
        let options = match parse(
            &words[1..],
            self.commands.fillet_preferences(),
            self.document.units(),
        ) {
            Ok(o) if o.sources.is_some() || o.picks.iter().any(Option::is_some) => return false,
            Ok(o) => o.preferences,
            Err(e) => {
                self.push_log(format!("Error: {e}"));
                return true;
            }
        };
        self.cancel_interactive_command(false);
        self.document.clear_selection();
        self.curve_fillet_session = Some(Session {
            picks: Vec::new(),
            options,
            tolerance: self.document.tolerance(),
            layer: self.document.current_layer_id(),
            pointer: None,
            pending: None,
            radius_start: None,
            preview: None,
            preview_options: None,
            scene: None,
        });
        self.active_command = Some(InteractiveCommand::Fillet { anchor: None });
        self.push_log(format!("> {text}"));
        self.curve_fillet_prompt();
        true
    }
    fn curve_fillet_prompt(&mut self) {
        if let Some(s) = &self.curve_fillet_session {
            self.commands
                .remember_fillet_preferences(s.options)
                .expect("validated Fillet preferences");
            self.push_log(format!("Fillet: {} ({})", s.hint(), s.options.arguments()));
        }
    }
    pub(super) fn picking_curve_fillet(&self) -> bool {
        self.plane_prompt.is_none()
            && self
                .curve_fillet_session
                .as_ref()
                .is_some_and(Session::selecting)
    }
    pub(super) fn validate_curve_fillet(&mut self) {
        if self
            .curve_fillet_session
            .as_ref()
            .is_some_and(|s| !s.current(&self.document))
        {
            self.cancel_interactive_command(false);
            self.push_log(
                "Fillet sources or document settings changed; pick the curves again".into(),
            );
        }
    }
    pub(super) fn pick_curve_fillet(&mut self, id: Option<ObjectId>) -> bool {
        if self.curve_fillet_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        if self.picking_curve_fillet() {
            let point = id.and_then(|id| {
                self.viewports[self.active_viewport].curve_pick_point(
                    self.document.object(id)?.geometry(),
                    None,
                    self.curve_fillet_session.as_ref()?.pointer?,
                    self.document.tolerance(),
                )
            });
            if let (Some(id), Some(point)) = (id, point) {
                self.pick_curve_fillet_at(id, point);
            }
        }
        true
    }
    pub(super) fn pick_curve_fillet_at(&mut self, id: ObjectId, point: Point3) {
        self.validate_curve_fillet();
        if !self.picking_curve_fillet() {
            return;
        }
        let object = self
            .document
            .object(id)
            .filter(|o| {
                self.document.is_object_selectable(id) && o.geometry().curve_ref().is_some()
            })
            .cloned();
        let Some(object) = object else {
            self.push_log("Select an editable curve".into());
            return;
        };
        let s = self.curve_fillet_session.as_mut().unwrap();
        if s.picks.iter().any(|(o, _)| o.id() == id) {
            self.push_log("Select a different source curve".into());
            return;
        }
        s.picks.push((object, point));
        self.document
            .set_transient_selection(s.picks.iter().map(|(o, _)| o.id()))
            .expect("validated curve references");
        self.document
            .mark_object_selection_temporary(s.picks.iter().map(|(o, _)| o.id()));
        if s.picks.len() == 2 {
            self.update_curve_fillet_preview(None, true);
            if !self.curve_fillet_session.as_ref().unwrap().options.dynamic {
                self.commit_curve_fillet();
                return;
            }
        }
        self.curve_fillet_prompt();
    }
    fn update_curve_fillet_preview(&mut self, radius: Option<Real>, announce: bool) {
        let Some(s) = &mut self.curve_fillet_session else {
            return;
        };
        if s.picks.len() != 2 {
            return;
        }
        let mut options = s.options;
        if let Some(r) = radius {
            options.radius = r;
        }
        if s.preview_options == Some(options) {
            return;
        }
        s.preview_options = Some(options);
        s.preview = None;
        s.scene = None;
        let picks = [
            (s.picks[0].0.id(), s.picks[0].1),
            (s.picks[1].0.id(), s.picks[1].1),
        ];
        let result = prepare(&self.document, picks, options).and_then(|p| {
            let scene = if options.dynamic {
                let mut scene = self.document.clone();
                // Native keeps both originals highlighted with the prospective
                // pieces. Only acceptance replaces the sources.
                for geometry in p.geometries() {
                    scene.add_geometry(geometry.clone())?;
                }
                scene.clear_history()?;
                Some(scene)
            } else {
                None
            };
            Ok((p, scene))
        });
        match result {
            Ok((p, scene)) => {
                s.preview = Some(p);
                s.scene = scene;
            }
            Err(e) if announce => self.push_log(format!("Fillet preview: {e}")),
            Err(_) => {}
        }
    }
    fn commit_curve_fillet(&mut self) {
        self.validate_curve_fillet();
        let Some(s) = self.curve_fillet_session.as_ref() else {
            return;
        };
        let Some(prepared) = &s.preview else {
            self.push_log("No valid fillet; change the radius or cancel".into());
            return;
        };
        match prepared.commit(&mut self.document) {
            Ok(_) => {
                self.cancel_interactive_command(false);
                self.push_log("Filleted two curves".into());
            }
            Err(e) => self.push_log(format!("Error: {e}")),
        }
    }
}
