//! Remembered Euclidean control averaging, independent of model history.
use super::*;
use viboceros_geometry::{SmoothingCoordinates, SmoothingOptions};

pub const USAGE: &str = "Smooth [SmoothFactor=number] [Steps=integer] [CoordinateSystem=World|CPlane|Object] [X=Yes|No] [Y=Yes|No] [Z=Yes|No] [FixBoundaries=Yes|No]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Coordinates {
    #[default]
    World,
    CPlane,
    Object,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_handles_values_toggles_duplicates_and_rejects_bad_batches() {
        let defaults = Options::default();
        let parsed = parse(
            &[
                "_SmoothFactor",
                "-.5",
                "Steps=3",
                "X",
                "Y=_No",
                "CoordinateSystem",
                "_CPlane",
                "FixBoundaries",
                "X=Yes",
                "_Enter",
            ],
            defaults,
        )
        .unwrap();
        assert_eq!(parsed.smoothing.factor, -0.5);
        assert_eq!(parsed.smoothing.steps, 3);
        assert_eq!(parsed.smoothing.axes, [true, false, true]);
        assert!(!parsed.smoothing.fix_boundaries);
        assert_eq!(parsed.coordinates, Coordinates::CPlane);
        assert_eq!(
            parse(
                &parsed
                    .command_line()
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>(),
                defaults
            )
            .unwrap(),
            parsed
        );
        for bad in [
            "SmoothFactor=NaN",
            "SmoothFactor=inf",
            "Steps=0",
            "Steps=-1",
            "Steps=1.5",
            "Steps=2147483648",
            "X=Maybe",
            "CoordinateSystem=Invalid",
            "Unknown=3",
            "Steps",
            "Enter X=No",
        ] {
            assert!(
                parse(&bad.split_whitespace().collect::<Vec<_>>(), parsed).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn successful_options_survive_undo_and_documents_but_selection_prompts_do_not_commit_them() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Line 0,0,0 2,0,0").unwrap();
        let id = doc.objects().next().unwrap().id();
        doc.select_objects_direct([id], viboceros_document::SelectionMode::Replace)
            .unwrap();
        let original = doc.object(id).unwrap().geometry().clone();
        registry
            .accept_object_selection_options(&prompt(
                parse(&["SmoothFactor=.4"], Options::default()).unwrap(),
            ))
            .unwrap();
        assert_eq!(registry.smooth_options_default(), Options::default());
        registry
            .execute(
                &mut doc,
                "_-Smooth SmoothFactor=.4 Steps=2 FixBoundaries=No",
            )
            .unwrap();
        assert!(matches!(
            doc.object(id).unwrap().geometry(),
            Geometry::NurbsCurve(_)
        ));
        let accepted = registry.smooth_options_default();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &original);
        assert_eq!(registry.smooth_options_default(), accepted);
        let mut other = Document::default();
        let before = registry.smooth_options_default();
        assert!(
            registry
                .execute(&mut other, "Smooth SmoothFactor=.8")
                .is_err()
        );
        assert_eq!(registry.smooth_options_default(), before);
        assert_eq!(
            CommandRegistry::with_builtins().smooth_options_default(),
            Options::default()
        );
    }
}
impl Coordinates {
    pub const fn token(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::CPlane => "CPlane",
            Self::Object => "Object",
        }
    }
    pub fn parse(token: &str) -> Option<Self> {
        [Self::World, Self::CPlane, Self::Object]
            .into_iter()
            .find(|v| option_name_eq(token, v.token()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub smoothing: SmoothingOptions,
    pub coordinates: Coordinates,
}
impl Options {
    pub fn command_line(self) -> String {
        let s = self.smoothing;
        format!(
            "Smooth SmoothFactor={} Steps={} CoordinateSystem={} X={} Y={} Z={} FixBoundaries={}",
            s.factor,
            s.steps,
            self.coordinates.token(),
            yes_no(s.axes[0]),
            yes_no(s.axes[1]),
            yes_no(s.axes[2]),
            yes_no(s.fix_boundaries)
        )
    }
    pub fn geometry_coordinates(self, active: Frame3) -> SmoothingCoordinates {
        match self.coordinates {
            Coordinates::World => SmoothingCoordinates::World,
            Coordinates::CPlane => SmoothingCoordinates::CPlane(active),
            Coordinates::Object => SmoothingCoordinates::Object,
        }
    }
}
fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

/// Accept both explicit values and native bare boolean toggles. Duplicate
/// options are processed in order; invalid batches accept none of their values.
pub fn parse(arguments: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let mut i = 0;
    while i < arguments.len() {
        let token = arguments[i];
        if option_name_eq(token, "Enter") && i + 1 == arguments.len() {
            break;
        }
        let (name, inline) = token
            .split_once('=')
            .map_or((token, None), |(n, v)| (n, Some(v)));
        let boolean = ["X", "Y", "Z", "FixBoundaries"]
            .iter()
            .position(|n| option_name_eq(name, n));
        if let Some(index) = boolean {
            let current = if index == 3 {
                &mut options.smoothing.fix_boundaries
            } else {
                &mut options.smoothing.axes[index]
            };
            if let Some(value) = inline {
                *current = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
            } else if let Some(value) = arguments.get(i + 1).and_then(|v| parse_yes_no(v)) {
                *current = value;
                i += 1;
            } else {
                *current = !*current;
            }
        } else {
            let value = if let Some(v) = inline {
                v
            } else {
                i += 1;
                *arguments.get(i).ok_or(CommandError::Usage(USAGE))?
            };
            if option_name_eq(name, "SmoothFactor") {
                options.smoothing.factor = value
                    .trim_start_matches('_')
                    .parse()
                    .map_err(|_| CommandError::Usage(USAGE))?;
            } else if option_name_eq(name, "Steps") {
                options.smoothing.steps = value
                    .trim_start_matches('_')
                    .parse()
                    .map_err(|_| CommandError::Usage(USAGE))?;
            } else if option_name_eq(name, "CoordinateSystem") {
                options.coordinates =
                    Coordinates::parse(value).ok_or(CommandError::Usage(USAGE))?;
            } else {
                return Err(CommandError::Usage(USAGE));
            }
        }
        i += 1;
    }
    validate(options)?;
    Ok(options)
}
fn validate(options: Options) -> Result<(), CommandError> {
    options.smoothing.validate()?;
    if options.smoothing.steps > i32::MAX as usize {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(())
}

pub fn prompt(options: Options) -> ObjectSelectionPrompt {
    ObjectSelectionPrompt {
        command: "Smooth",
        filter: ObjectSelectionFilter::Smooth,
        options: ["X", "Y", "Z", "FixBoundaries"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| BooleanSelectionOption {
                name,
                value: if i == 3 {
                    options.smoothing.fix_boundaries
                } else {
                    options.smoothing.axes[i]
                },
                aliases: &[],
            })
            .collect(),
        menus: vec![],
        choices: vec![ChoiceSelectionOption {
            name: "CoordinateSystem",
            value: options.coordinates.token(),
            choices: &["World", "CPlane", "Object"],
            toggle: None,
        }],
        workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
    }
}

impl CommandRegistry {
    pub fn smooth_options_default(&self) -> Options {
        self.smooth_preferences.get()
    }
    pub fn accept_smooth_options(&self, options: Options) -> Result<(), CommandError> {
        validate(options)?;
        self.smooth_preferences.set(options);
        Ok(())
    }
}
pub(super) struct SmoothCommand(pub(super) std::sync::Arc<remembered::Remembered<Options>>);
impl SmoothCommand {
    fn apply(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
        postselected: bool,
    ) -> Result<String, CommandError> {
        let options = parse(arguments, self.0.get())?;
        let ids = document
            .selected_objects()
            .filter(|o| o.geometry().supports_smoothing())
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let grips = document
            .selected_control_points()
            .filter(|(p, _)| {
                document
                    .object(p.object)
                    .is_some_and(|o| o.geometry().supports_smoothing())
            })
            .map(|(p, _)| p)
            .collect::<Vec<_>>();
        if ids.is_empty() && grips.is_empty() {
            return Err(CommandError::Usage(
                "Smooth requires selected curves, surfaces, meshes, or control points",
            ));
        }
        if postselected {
            document.release_command_selection_on_history_replay(
                ids.iter().copied().chain(grips.iter().map(|p| p.object)),
            )?;
            document.clear_selection();
        }
        let count = document.smooth_objects_and_grips(
            ids,
            grips,
            options.smoothing,
            options.geometry_coordinates(context.construction_plane),
        )?;
        self.0.set(options);
        Ok(format!("Smoothed {count} object(s)"))
    }
}
impl Command for SmoothCommand {
    fn name(&self) -> &'static str {
        "Smooth"
    }
    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(Some(prompt(parse(arguments, self.0.get())?)))
    }
    fn object_selection_confirmation(
        &self,
        _document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(arguments)
    }
    fn accept_object_selection_options(&self, arguments: &[&str]) -> Result<(), CommandError> {
        parse(arguments, self.0.get())?;
        Ok(())
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.apply(document, arguments, CommandContext::default(), false)
    }
    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.apply(document, arguments, context, false)
    }
    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        self.apply(document, arguments, context, true)
    }
}
