//! Command dispatch and per-family options outside model history.
use super::*;
#[derive(Clone, Copy)]
struct Defaults {
    subd: f64,
    mesh: f64,
    surface: Options,
}
impl Default for Defaults {
    fn default() -> Self {
        Self {
            subd: 0.1,
            mesh: 1.,
            surface: Options {
                distance: 1.,
                ..Options::default()
            },
        }
    }
}
#[derive(Default)]
pub(crate) struct InsetCommand(remembered::Remembered<Defaults>);
impl Command for InsetCommand {
    fn name(&self) -> &'static str {
        "Inset"
    }
    fn inset_options(&self, mesh: bool) -> Option<Options> {
        let d = self.0.get();
        Some(Options {
            distance: if mesh { d.mesh } else { d.subd },
            mode: Mode::Group,
            ..Options::default()
        })
    }
    fn inset_surface_options(&self) -> Option<Options> {
        Some(self.0.get().surface)
    }
    fn remember_inset_surface_options(&self, options: Options) -> bool {
        let mut d = self.0.get();
        d.surface = Options {
            loose: false,
            ..options
        };
        self.0.set(d);
        true
    }
    fn remember_inset_options(&self, options: Options, mesh: bool) -> bool {
        let mut d = self.0.get();
        if mesh {
            d.mesh = options.distance;
        } else {
            d.subd = options.distance;
        }
        self.0.set(d);
        true
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let mesh = doc
            .selected_objects()
            .all(|o| matches!(o.geometry(), Geometry::Mesh(_)));
        let surface = doc
            .selected_objects()
            .all(|o| surface::is_surface(o.geometry()));
        let input = parse(
            args,
            if surface {
                self.inset_surface_options()
            } else {
                self.inset_options(mesh)
            }
            .unwrap(),
        )?;
        if !input.faces_supplied && !surface {
            return Err(CommandError::Usage(
                "Inset on selected objects requires Faces=All or Faces=0,1,...; use the face-selection API otherwise",
            ));
        }
        let targets = selected_targets(doc, input.faces.as_deref())?;
        let plan = Plan::prepare(doc, targets, input.options)?;
        let message = plan.apply(doc)?;
        if surface {
            self.remember_inset_surface_options(input.options);
        } else {
            self.remember_inset_options(input.options, mesh);
        }
        Ok(message)
    }
}
impl CommandRegistry {
    pub fn inset_surface_options(&self) -> Options {
        self.commands
            .iter()
            .find_map(|c| c.inset_surface_options())
            .unwrap_or(Options {
                distance: 1.,
                ..Options::default()
            })
    }
    pub fn remember_inset_surface_options(&self, options: Options) {
        for c in &self.commands {
            if c.remember_inset_surface_options(options) {
                break;
            }
        }
    }
    pub fn inset_options(&self, mesh: bool) -> Options {
        self.commands
            .iter()
            .find_map(|c| c.inset_options(mesh))
            .unwrap_or_default()
    }
    pub fn remember_inset_options(&self, options: Options, mesh: bool) {
        for c in &self.commands {
            if c.remember_inset_options(options, mesh) {
                break;
            }
        }
    }
}
