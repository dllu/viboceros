//! Two-point measurements, independent of selection and model history.
use super::*;
use crate::{CommandContext, parse_point};

pub(crate) struct DistanceCommand;

impl Command for DistanceCommand {
    fn name(&self) -> &'static str {
        "Distance"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, arguments, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (start, consumed) = parse_point(arguments)?;
        let (end, second) = parse_point(&arguments[consumed..])?;
        let target = match &arguments[consumed + second..] {
            [] => None,
            [option] => {
                let (name, value) = option
                    .split_once('=')
                    .ok_or(CommandError::Usage("Distance start end [Units=name]"))?;
                if !name.eq_ignore_ascii_case("Units") {
                    return Err(CommandError::Usage("Distance start end [Units=name]"));
                }
                Some(
                    crate::model_units::parse_units(value)
                        .ok_or(CommandError::Usage("unknown display units"))?,
                )
            }
            _ => return Err(CommandError::Usage("Distance start end [Units=name]")),
        };
        let scale = if let Some(target) = &target {
            // Unitless/unset metadata does not establish a physical conversion.
            if document
                .units()
                .meters_per_unit()
                .map_err(viboceros_document::DocumentError::from)?
                .is_none()
                || target
                    .meters_per_unit()
                    .map_err(viboceros_document::DocumentError::from)?
                    .is_none()
            {
                return Err(CommandError::Usage(
                    "Distance display conversion requires physical source and target units",
                ));
            }
            document
                .units()
                .scale_to(target)
                .map_err(viboceros_document::DocumentError::from)?
        } else {
            1.
        };
        let distance = match start.distance_to(end) {
            Ok(distance) => distance,
            Err(_) if scale < 1. => {
                let local = context
                    .construction_plane
                    .with_origin(start)
                    .scaled_coordinates_of(end, scale)?;
                let world = CommandContext::default()
                    .construction_plane
                    .with_origin(start)
                    .scaled_coordinates_of(end, scale)?;
                let distance = viboceros_geometry::Vector3::try_from(world)?.length()?;
                return Ok(format!(
                    "{}\n{}\nDistance = {}{}",
                    describe_scaled("CPlane", local, 1.)?,
                    describe_scaled("World", world, 1.)?,
                    format_measurement(distance),
                    target
                        .map(|units| format!(" {}", units.name()))
                        .unwrap_or_default(),
                ));
            }
            Err(error) => return Err(error.into()),
        };
        // Project the displacement directly. Subtracting two coordinates
        // measured from a remote CPlane origin would lose small differences.
        let local = context
            .construction_plane
            .with_origin(start)
            .coordinates_of(end)?;
        let world = CommandContext::default()
            .construction_plane
            .with_origin(start)
            .coordinates_of(end)?;
        Ok(format!(
            "{}\n{}\nDistance = {}{}",
            describe_scaled("CPlane", local, scale)?,
            describe_scaled("World", world, scale)?,
            format_measurement(display_value(distance, scale)?),
            target
                .map(|units| format!(" {}", units.name()))
                .unwrap_or_default(),
        ))
    }
}

fn display_value(value: Real, scale: Real) -> Result<Real, CommandError> {
    let result = value * scale;
    if !result.is_finite() || (value != 0. && result == 0.) {
        return Err(viboceros_document::DocumentError::from(
            viboceros_geometry::UnitError::UnrepresentableScale,
        )
        .into());
    }
    Ok(result)
}

#[cfg(test)]
fn describe(name: &str, delta: [Real; 3]) -> String {
    describe_scaled(name, delta, 1.).unwrap()
}

fn describe_scaled(name: &str, delta: [Real; 3], scale: Real) -> Result<String, CommandError> {
    let [x, y, z] = delta;
    let azimuth = if x == 0. && y == 0. {
        0.
    } else {
        let angle = y.atan2(x).to_degrees().rem_euclid(360.);
        // rem_euclid can round a tiny negative angle up to its divisor.
        if angle == 360. { 0. } else { angle }
    };
    let elevation = if delta == [0.; 3] {
        0.
    } else {
        let scale = x.abs().max(y.abs()).max(z.abs());
        (z / scale).atan2((x / scale).hypot(y / scale)).to_degrees()
    };
    let [x, y, z, azimuth, elevation] = [
        display_value(x, scale)?,
        display_value(y, scale)?,
        display_value(z, scale)?,
        azimuth,
        elevation,
    ]
    .map(format_measurement);
    Ok(format!(
        "{name} angles and deltas: xy = {azimuth} elevation = {elevation} dx = {x} dy = {y} dz = {z}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_geometry::{Frame3, Point3, Vector3};

    #[test]
    fn display_conversion_precedes_source_distance_overflow() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let before = format!("{document:?}");
        for input in [
            "Distance -1e308,0 1e308,0 Units=km",
            "Distance 0,0,0 1.2e308,1.6e308,0 Units=km",
        ] {
            let report = registry.execute(&mut document, input).unwrap();
            let fields: Vec<_> = report.lines().last().unwrap().split_whitespace().collect();
            let distance: f64 = fields[2].parse().unwrap();
            assert!((distance / 2e302 - 1.).abs() < 3e-16, "{report}");
            assert_eq!(format!("{document:?}"), before);
        }
    }

    #[test]
    fn display_units_scale_only_reported_lengths_and_preserve_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry
            .execute(&mut document, "Units Meters Scale=No")
            .unwrap();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        let original = registry.execute(&mut document, "Distance 0,0 3,4").unwrap();
        let converted = registry
            .execute(&mut document, "Distance 0,0 3,4 Units=cm")
            .unwrap();
        assert!(converted.ends_with("Distance = 500 Centimetres"));
        for (raw, scaled) in original.lines().take(2).zip(converted.lines()) {
            assert_eq!(raw.split(" dx =").next(), scaled.split(" dx =").next());
            assert!(scaled.ends_with("dx = 300 dy = 400 dz = 0"));
        }
        assert_eq!(format!("{document:?}"), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 1);

        registry
            .execute(
                &mut document,
                "Units Custom MetersPerUnit=0.25 Scale=No Name=quarter metre",
            )
            .unwrap();
        assert!(
            registry
                .execute(&mut document, "Distance 0,0 4,0 uNiTs=MeTrEs")
                .unwrap()
                .ends_with("Distance = 1 Metres")
        );
    }

    #[test]
    fn display_units_reject_ambiguous_units_and_unrepresentable_results() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        let before = format!("{document:?}");
        for input in [
            "Distance 0,0 1,0 Units=unknown",
            "Distance 0,0 1,0 Units=Unset",
            "Distance 0,0 1,0 Units=Unitless",
            "Distance 0,0 1,0 Units=m Units=cm",
            "Distance 0,0 1e308,0 Units=Angstroms",
            "Distance 0,0 1e-320,0 Units=km",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(format!("{document:?}"), before);
        }
        registry
            .execute(&mut document, "Units None Scale=No")
            .unwrap();
        let before = format!("{document:?}");
        assert!(
            registry
                .execute(&mut document, "Distance 0,0 1,0 Units=m")
                .is_err()
        );
        assert!(
            registry
                .execute(&mut document, "Distance 0,0 1,0")
                .unwrap()
                .ends_with("Distance = 1")
        );
        assert_eq!(format!("{document:?}"), before);
    }

    #[test]
    fn distance_reports_world_and_rotated_cplane_without_history_changes() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 1,2,3").unwrap();
        registry.execute(&mut document, "SelAll").unwrap();
        registry.execute(&mut document, "Point 9,8,7").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        let before = format!("{document:?}");
        let context = CommandContext {
            construction_plane: Frame3::try_from_directions(
                Point3::try_new(1e100, -1e100, 1e100).unwrap(),
                Vector3::try_new(0., 1., 0.).unwrap(),
                Vector3::try_new(0., 0., 1.).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        };
        let output = registry
            .execute_in_context(&mut document, "dIsTaNcE 1,2,3 4,6,15", context)
            .unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert!(lines[0].ends_with("dx = 4 dy = 12 dz = 3"));
        assert!(lines[1].ends_with("dx = 3 dy = 4 dz = 12"));
        assert_eq!(lines[2], "Distance = 13");
        assert_eq!(format!("{document:?}"), before);
        registry.execute(&mut document, "Redo").unwrap();
        assert_eq!(document.objects().count(), 2);
    }

    #[test]
    fn distance_preserves_tiny_large_and_zero_values() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        for (input, expected) in [
            ("Distance 0,0 0,0", 0.),
            ("Distance 0 0 0 3 4 0", 5.),
            ("Distance 0,0 1e-300,0", 1e-300),
            ("Distance 0,0 1e300,0", 1e300),
        ] {
            let result = registry.execute(&mut document, input).unwrap();
            let value: f64 = result.split_whitespace().last().unwrap().parse().unwrap();
            assert_eq!(value, expected);
        }
        assert!(describe("World", [0., 0., 1.]).contains("xy = 0 elevation = 90"));
        assert!(describe("World", [0., -1., 0.]).contains("xy = 270 elevation = 0"));
        assert!(describe("World", [1., -f64::MIN_POSITIVE, 0.]).contains("xy = 0 elevation = 0"));
    }

    #[test]
    fn invalid_distance_queries_leave_the_document_unchanged() {
        let mut document = Document::default();
        let before = format!("{document:?}");
        let registry = CommandRegistry::with_builtins();
        for input in [
            "Distance",
            "Distance 0,0",
            "Distance 0,0 1,1 extra",
            "Distance nan,0 1,1",
            "Distance -1e308,0 1e308,0",
        ] {
            assert!(registry.execute(&mut document, input).is_err(), "{input}");
            assert_eq!(format!("{document:?}"), before);
        }
    }
}
