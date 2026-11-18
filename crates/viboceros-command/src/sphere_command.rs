//! Exact NURBS sphere creation from complete arguments or an interactive definition.
use crate::{
    Command, CommandContext, CommandError, CommandRegistry, ObjectSelectionPrompt,
    circle_input::{CircleDefinition, CircleSizeMode},
    option_name_eq, parse_finite_real, parse_point, point_fitting,
    remembered::Remembered,
    require_consumed,
    sphere_input::{Preferences, SphereInput},
};
use std::sync::Arc;
use viboceros_document::{Document, Geometry, ObjectId};
use viboceros_drafting::PointInput;
use viboceros_geometry::{GeometryError, LengthUnitSystem, NurbsSurface, Tolerance};

const USAGE: &str =
    "Sphere center radius | Sphere center point-on-sphere | Sphere 2Point/3Point/4Point points";
const FIT_USAGE: &str = "Sphere FitPoints (select points or grips)";

pub(super) struct SphereCommand(pub(super) Arc<Remembered<Preferences>>);

impl CommandRegistry {
    pub fn sphere_preferences(&self) -> Preferences {
        self.sphere_preferences.get()
    }

    pub fn remember_sphere_preferences(&self, preferences: Preferences) {
        self.sphere_preferences.set(preferences);
    }
}

impl Command for SphereCommand {
    fn name(&self) -> &'static str {
        "Sphere"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["Sph"]
    }

    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(
            matches!(args, [option] if option_name_eq(option, "FitPoints"))
                .then(|| point_fitting::selection_prompt(point_fitting::PointFitKind::Sphere)),
        )
    }

    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if args.first().is_some_and(|s| option_name_eq(s, "FitPoints")) {
            require_consumed(args, 1, FIT_USAGE)?;
            return point_fitting::run(point_fitting::PointFitKind::Sphere, doc, &[], true);
        }
        self.run_in_context(doc, args, context)
    }

    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        if args.first().is_some_and(|s| option_name_eq(s, "FitPoints")) {
            require_consumed(args, 1, FIT_USAGE)?;
            return point_fitting::run(point_fitting::PointFitKind::Sphere, doc, &[], false);
        }
        let mut preferences = self.0.get();
        let (definition, release) = if let Some(definition) = positional_definition(args) {
            let definition = definition?;
            preferences.radius = definition.radius;
            (definition, None)
        } else {
            from_arguments(
                doc,
                args,
                SphereInput::with_preferences(context.construction_plane, preferences),
                &mut preferences,
            )?
        };
        let id = add(doc, definition, release)?;
        self.0.set(preferences);
        Ok(message(id, definition.radius))
    }
}

pub(super) fn from_arguments(
    doc: &Document,
    args: &[&str],
    input: SphereInput,
    preferences: &mut Preferences,
) -> Result<(CircleDefinition, Option<ObjectId>), CommandError> {
    let mut release = None;
    let definition = if let [option, rest @ ..] = args
        && option_name_eq(option, "AroundCurve")
    {
        let around = crate::circle_around_input::from_arguments(doc, rest, (*preferences).into())?;
        if around.postselected {
            release = Some(around.source);
        }
        *preferences = around.preferences.into();
        CircleDefinition::from_circle(around.circle)?
    } else if let [option, rest @ ..] = args
        && option_name_eq(option, "Tangent")
    {
        let tangent = crate::circle_tangent_input::sphere_from_arguments(
            doc,
            rest,
            input.construction_plane(),
            (*preferences).into(),
        )?;
        *preferences = tangent.preferences.into();
        CircleDefinition::from_circle(tangent.circle)?
    } else {
        input_definition(doc, args, input, preferences)?
    };
    Ok((definition, release))
}

fn positional_definition(args: &[&str]) -> Option<Result<CircleDefinition, CommandError>> {
    let (center, consumed) = parse_point(args).ok()?;
    let remaining = &args[consumed..];
    let radius = match remaining {
        [number] if number.parse::<f64>().is_ok() => parse_finite_real(number),
        _ => {
            let (point, count) = parse_point(remaining).ok()?;
            if count != remaining.len() {
                return None;
            }
            center.distance_to(point).map_err(Into::into)
        }
    };
    Some(radius.map(|radius| {
        CircleDefinition {
            frame: CommandContext::default()
                .construction_plane
                .with_origin(center),
            radius,
        }
    }))
}

fn input_definition(
    doc: &Document,
    mut words: &[&str],
    mut input: SphereInput,
    preferences: &mut Preferences,
) -> Result<CircleDefinition, CommandError> {
    while let Some((&word, rest)) = words.split_first() {
        if input.option(word) {
            input.update_size_mode(preferences);
            words = rest;
            continue;
        }
        let named = word.split_once('=');
        let size = if let Some((name, value)) = named {
            if !input.option(name) || !input.requests_size() {
                return Err(CommandError::Usage(USAGE));
            }
            input.update_size_mode(preferences);
            Some(
                size_argument(input, value, doc.units(), *preferences)?
                    .ok_or_else(|| CommandError::InvalidNumber(value.into()))?,
            )
        } else if input.requests_size() && (words.len() != 3 || word.contains(',')) {
            size_argument(input, word, doc.units(), *preferences)?
        } else {
            None
        };
        let definition = if let Some(size) = size {
            words = rest;
            input.number(size)?
        } else {
            let (point, consumed) = parse_point(words)?;
            words = &words[consumed..];
            input.point(point)?
        };
        if let Some(definition) = definition {
            if !words.is_empty() {
                return Err(CommandError::Usage(USAGE));
            }
            validate_size(definition, doc.tolerance())?;
            input.update_radius(preferences, definition.radius);
            return Ok(definition);
        }
    }
    Err(CommandError::Usage(USAGE))
}

pub fn size_argument(
    input: SphereInput,
    word: &str,
    units: &LengthUnitSystem,
    preferences: Preferences,
) -> Result<Option<f64>, CommandError> {
    if word.is_empty() || option_name_eq(word, "Enter") {
        return Ok(input.remembered_size(preferences));
    }
    let units = if input.size_mode() == CircleSizeMode::Area {
        &LengthUnitSystem::Unset
    } else {
        units
    };
    PointInput::parse_length_with_units(word, units)
        .transpose()
        .map_err(|_| CommandError::InvalidNumber(word.into()))
}

fn validate_size(definition: CircleDefinition, tolerance: Tolerance) -> Result<(), GeometryError> {
    if definition.radius.is_finite() && definition.radius > tolerance.absolute() {
        Ok(())
    } else {
        Err(GeometryError::Degenerate {
            context: "sphere radius must exceed document tolerance",
        })
    }
}

pub fn commit(
    doc: &mut Document,
    definition: CircleDefinition,
    release: Option<ObjectId>,
) -> Result<String, CommandError> {
    validate_size(definition, doc.tolerance())?;
    doc.begin_transaction("Sphere")?;
    let result = (|| -> Result<_, CommandError> {
        let id = add(doc, definition, release)?;
        doc.commit_transaction()?;
        Ok(message(id, definition.radius))
    })();
    if result.is_err() {
        doc.rollback_transaction()?;
    }
    result
}

fn add(
    doc: &mut Document,
    definition: CircleDefinition,
    release: Option<ObjectId>,
) -> Result<ObjectId, CommandError> {
    let surface = NurbsSurface::try_sphere(definition.frame, definition.radius)?;
    if let Some(id) = release {
        doc.release_command_selection_on_history_replay([id])?;
        doc.select_command_results([])?;
    }
    Ok(doc.add_geometry(Geometry::NurbsSurface(surface))?)
}

fn message(id: ObjectId, radius: f64) -> String {
    format!("Added NURBS sphere {id} (radius {radius:.6})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::Point3;

    #[test]
    fn complete_sphere_modes_share_geometry_and_preserve_history_on_invalid_input() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry.execute(&mut doc, "Line 0,0,0 0,10,10").unwrap();
        let guide = doc.objects().next().unwrap().id();
        let around = format!("Sphere AroundCurve Curve={guide} 0,3,3 6,3,8");
        let cases = [
            ("Sphere 2Point 0,0,-3 0,0,3", [0., 3., 0.]),
            ("Sphere 3Point -2,0,0 2,0,0 Radius 3 0,5,0", [2., 0., 0.]),
            ("Sphere 4Point 3,0,0 0,3,0 -3,0,0 0,0,6", [3.75, 0., 2.25]),
            (
                "Sphere 4Point 3,0,0 0,3,0 -3,0,0 Radius=4 0,0,10",
                [4., 0., 7.0_f64.sqrt()],
            ),
            (
                "Sphere 0,0,0 Orientation 1,1,1 Diameter=2",
                [
                    -std::f64::consts::FRAC_1_SQRT_2,
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.,
                ],
            ),
            (
                "Sphere Tangent Point 3,0,0 Point 0,3,0 Point -3,0,0",
                [3., 0., 0.],
            ),
        ];
        for (command, seam) in cases.into_iter().chain([(around.as_str(), [6., 0.5, 5.5])]) {
            registry
                .execute(&mut doc, command)
                .unwrap_or_else(|e| panic!("{command}: {e}"));
            let Geometry::NurbsSurface(sphere) = doc.objects().last().unwrap().geometry() else {
                panic!("expected exact Sphere surface");
            };
            let seam = Point3::try_from(seam).unwrap();
            assert!(
                sphere.evaluate(0., 0.).unwrap().distance_to(seam).unwrap() < 1e-10,
                "{command}"
            );
            assert_eq!(doc.undo_label(), Some("Sphere"));
        }
        registry.execute(&mut doc, "Undo").unwrap();
        let prefs = registry.sphere_preferences();
        let objects = doc.objects().cloned().collect::<Vec<_>>();
        for command in [
            "Sphere 2Point 0,0 0,0",
            "Sphere 0,0 Diameter=5 trailing",
            "Sphere 3Point 0,0 3,0 Radius=1",
            "Sphere 4Point 3,0 0,3 -3,0 0,-3",
        ] {
            assert!(registry.execute(&mut doc, command).is_err(), "{command}");
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), objects);
            assert_eq!(registry.sphere_preferences(), prefs);
            assert_eq!(doc.redo_label(), Some("Sphere"));
        }
    }
}
