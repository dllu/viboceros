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
        _document: &mut Document,
        arguments: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (start, consumed) = parse_point(arguments)?;
        let (end, second) = parse_point(&arguments[consumed..])?;
        require_consumed(arguments, consumed + second, "Distance start end")?;
        let distance = start.distance_to(end)?;
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
            "{}\n{}\nDistance = {}",
            describe("CPlane", local),
            describe("World", world),
            format_measurement(distance),
        ))
    }
}

fn describe(name: &str, delta: [Real; 3]) -> String {
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
    let [x, y, z, azimuth, elevation] = [x, y, z, azimuth, elevation].map(format_measurement);
    format!(
        "{name} angles and deltas: xy = {azimuth} elevation = {elevation} dx = {x} dy = {y} dz = {z}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandRegistry;
    use viboceros_geometry::{Frame3, Point3, Vector3};

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
