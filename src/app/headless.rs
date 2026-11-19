//! Persistent, windowless host for the same document, getters and views as the desktop app.
mod capture;
mod input;
mod model;
mod surfaces;
mod widgets;

use super::*;
use crate::viewport_gpu::readback::OffscreenRenderer;
use input::{Step, ViewRef};
use model::{ComponentRef, GripRef};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

const PROTOCOL: u32 = 1;
const SCREEN_SIZE: egui::Vec2 = egui::vec2(1200.0, 800.0);

type SessionError = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, SessionError>;

#[derive(Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
enum Request {
    Info,
    Reset,
    Commands,
    Run {
        steps: Vec<Step>,
        select: Option<Vec<String>>,
        track_changes: bool,
        stop_on_error: bool,
        #[serde(default)]
        require_idle: bool,
    },
    Objects {
        ids: Option<Vec<String>>,
    },
    Layers,
    Groups,
    Selection,
    Select {
        ids: Vec<String>,
    },
    Components {
        id: String,
    },
    SelectComponents {
        components: Vec<ComponentRef>,
    },
    Grips {
        id: String,
    },
    SelectGrips {
        grips: Vec<GripRef>,
    },
    Viewports,
    Widgets,
    AddSurfacePatches {
        surface: surfaces::Definition,
        patches: Vec<surfaces::Patch>,
    },
    EditBrepFaceCharts {
        id: String,
        edits: Vec<surfaces::ChartEdit>,
    },
    SewBrepEdges {
        pairs: Vec<(ComponentRef, ComponentRef, bool)>,
        anchor: Option<String>,
        vertex_distance: Option<f64>,
    },
    SurfaceDefinition {
        id: String,
        face: Option<usize>,
    },
    SurfaceJets {
        id: String,
        face: Option<usize>,
        samples: Vec<surfaces::Sample>,
    },
    PullbackBrepEdge {
        edge: ComponentRef,
        id: String,
        face: Option<usize>,
        #[serde(default)]
        reversed: bool,
        endpoints: Option<[[f64; 2]; 2]>,
    },
    Project {
        point: [f64; 3],
        view: Option<ViewRef>,
    },
    Screenshot {
        #[serde(flatten)]
        capture: capture::Screenshot,
    },
}

pub(crate) fn run() -> Result<()> {
    let input = io::stdin().lock();
    let mut output = io::BufWriter::new(io::stdout().lock());
    let mut session = Session::new();
    for line in input.lines() {
        let line = line?;
        let result = serde_json::from_str(&line)
            .map_err(SessionError::from)
            .and_then(|request| session.handle(request));
        let response = match result {
            Ok(result) => json!({"result": result}),
            Err(error) => json!({"error": error.to_string()}),
        };
        serde_json::to_writer(&mut output, &response)?;
        writeln!(output)?;
        output.flush()?;
    }
    Ok(())
}

struct Session {
    app: VibocerosApp,
    context: egui::Context,
    renderer: Option<OffscreenRenderer>,
}

impl Session {
    fn new() -> Self {
        let mut session = Self {
            app: VibocerosApp::with_storage(None, Default::default()),
            context: egui::Context::default(),
            renderer: None,
        };
        session.context.set_visuals(egui::Visuals::light());
        session.context.enable_accesskit();
        session.frame(vec![], egui::Modifiers::NONE);
        session
    }

    fn handle(&mut self, request: Request) -> Result<Value> {
        let document = &self.app.document;
        Ok(match request {
            Request::Info => json!({
                "protocol": PROTOCOL,
                "version": env!("CARGO_PKG_VERSION"),
                "native_smlib": true,
                "path": self.app.document_path,
                "units": document.units().name(),
                "absolute_tolerance": document.tolerance().absolute(),
                "pending": self.pending(),
                "prompt": self.prompt(),
                "screen_size": [SCREEN_SIZE.x, SCREEN_SIZE.y],
            }),
            Request::Reset => {
                let renderer = self.renderer.take();
                *self = Self::new();
                self.renderer = renderer;
                Value::Null
            }
            Request::Commands => {
                let mut names =
                    command_line::available_commands(&self.app.commands).collect::<Vec<_>>();
                names.sort_unstable_by_key(|name| name.to_ascii_lowercase());
                names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
                json!(names)
            }
            Request::Run {
                steps,
                select,
                track_changes,
                stop_on_error,
                require_idle,
            } => {
                if require_idle && self.pending() {
                    return Err(
                        "finish or cancel the active command before changing project files".into(),
                    );
                }
                self.run_steps(steps, select, track_changes, stop_on_error)?
            }
            Request::Objects { ids } => model::objects(document, ids)?,
            Request::Layers => model::layers(document),
            Request::Groups => model::groups(document),
            Request::Selection => self.selection(),
            Request::Select { ids } => {
                self.select(ids)?;
                self.selection()
            }
            Request::Components { id } => model::components(document, id)?,
            Request::SelectComponents { components } => {
                self.select_components(components)?;
                self.selection()
            }
            Request::Grips { id } => model::grips(document, id)?,
            Request::SelectGrips { grips } => {
                self.select_grips(grips)?;
                self.selection()
            }
            Request::Viewports => self.viewports(),
            Request::Widgets => self.widgets(),
            Request::AddSurfacePatches { surface, patches } => {
                if self.pending() {
                    return Err("finish the active command before adding surfaces".into());
                }
                surfaces::add_patches(&mut self.app.document, surface, patches)?
            }
            Request::EditBrepFaceCharts { id, edits } => {
                if self.pending() {
                    return Err("finish the active command before editing face charts".into());
                }
                surfaces::edit_face_charts(&mut self.app.document, &id, edits)?
            }
            Request::SewBrepEdges {
                pairs,
                anchor,
                vertex_distance,
            } => {
                if self.pending() {
                    return Err("finish the active command before sewing B-rep edges".into());
                }
                surfaces::sew_edges(
                    &mut self.app.document,
                    pairs,
                    anchor.as_deref(),
                    vertex_distance,
                )?
            }
            Request::SurfaceDefinition { id, face } => surfaces::definition(document, &id, face)?,
            Request::SurfaceJets { id, face, samples } => {
                surfaces::jets(document, &id, face, samples)?
            }
            Request::PullbackBrepEdge {
                edge,
                id,
                face,
                reversed,
                endpoints,
            } => surfaces::pullback_edge(document, edge, &id, face, reversed, endpoints)?,
            Request::Project { point, view } => {
                let position = self.project_point(point, view.as_ref())?;
                json!([position.x, position.y])
            }
            Request::Screenshot { capture } => {
                self.screenshot(capture)?;
                Value::Null
            }
        })
    }

    fn pending(&self) -> bool {
        !self.app.command_file_child_idle()
            || self.app.command_file.is_active()
            || self.app.command_line.file_path.is_some()
            || self.app.selection_menu.is_some()
            || self.app.block_session.is_some()
            || self.app.subcurve_prompt.is_some()
            || self.app.length_choices.is_some()
            || self.app.length_units_pending
    }

    fn prompt(&self) -> Option<&str> {
        self.pending().then(|| self.app.command_hint())
    }

    fn run_steps(
        &mut self,
        steps: Vec<Step>,
        select: Option<Vec<String>>,
        track_changes: bool,
        stop_on_error: bool,
    ) -> Result<Value> {
        if let Some(ids) = select {
            self.select(ids)?;
        }
        let before = track_changes.then(|| model::snapshot(&self.app.document));
        self.app.command_capture = Some(Vec::new());
        let mut errors = Vec::new();
        let mut processed = 0;
        for step in steps {
            let first_message = self.app.command_capture.as_ref().unwrap().len();
            if let Err(error) = self.step(step) {
                errors.push(error.to_string());
            }
            errors.extend(
                self.app.command_capture.as_ref().unwrap()[first_message..]
                    .iter()
                    .filter_map(|message| message.strip_prefix("Error: ").map(str::to_owned)),
            );
            processed += 1;
            if stop_on_error && !errors.is_empty() {
                break;
            }
        }
        let messages = self.app.command_capture.take().unwrap();
        let changes = before.map(|before| model::changes(&self.app.document, before));
        Ok(json!({
            "messages": messages,
            "errors": errors,
            "processed": processed,
            "pending": self.pending(),
            "prompt": self.prompt(),
            "selected": model::selected_ids(&self.app.document),
            "changes": changes,
        }))
    }
}
