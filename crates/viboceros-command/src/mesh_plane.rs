//! A quadrilateral grid over a rectangle constructed from points or dimensions.
use super::*;
use crate::rectangle_input::{RectangleDefinition, RectangleInput, RectangleScript};

pub const USAGE: &str = "MeshPlane [XCount=n YCount=n] [Center|3Point [EdgeMidpoint]|Vertical|AroundCurve Curve=id|Edge=id:index] points-or-dimensions";
pub const DEFAULT_FACE_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preferences {
    pub counts: [usize; 2],
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            counts: [DEFAULT_FACE_COUNT; 2],
        }
    }
}

impl Preferences {
    pub fn validate(self) -> Result<(), CommandError> {
        let [x_count, y_count] = self.counts;
        if x_count == 0 || y_count == 0 {
            return Err(GeometryError::InvalidMeshPlaneFaceCount { x_count, y_count }.into());
        }
        if x_count
            .checked_mul(y_count)
            .is_none_or(|faces| faces > MAX_MESH_PLANE_FACES)
        {
            return Err(GeometryError::TooManyMeshFaces.into());
        }
        Ok(())
    }

    pub fn option(&mut self, args: &[&str]) -> Result<Option<usize>, CommandError> {
        if let Some(count) = args
            .first()
            .and_then(|word| word.trim_start_matches('_').parse::<usize>().ok())
        {
            if count == 0 {
                return Ok(None);
            }
            self.counts = [count; 2];
            return Ok(Some(1));
        }
        let Some((axis, count, consumed)) = axis_count(args)? else {
            return Ok(None);
        };
        self.counts[axis] = count;
        Ok(Some(consumed))
    }
}

fn axis_index(argument: &str) -> Option<usize> {
    let name = argument.split_once('=').map_or(argument, |(name, _)| name);
    ["XCount", "YCount"]
        .iter()
        .position(|axis| option_name_eq(name, axis))
}

fn axis_count(args: &[&str]) -> Result<Option<(usize, usize, usize)>, CommandError> {
    let Some(axis) = args.first().and_then(|word| axis_index(word)) else {
        return Ok(None);
    };
    let (value, consumed) = if let Some((_, value)) = args[0].split_once('=') {
        (value, 1)
    } else {
        (*args.get(1).ok_or(CommandError::Usage(USAGE))?, 2)
    };
    let count = value
        .trim_start_matches('_')
        .parse::<usize>()
        .ok()
        .filter(|&n| n > 0)
        .ok_or_else(|| CommandError::InvalidMeshPlaneFaceCount(value.to_owned()))?;
    Ok(Some((axis, count, consumed)))
}

fn leading_counts(
    args: &[&str],
    mut preferences: Preferences,
) -> Result<(Preferences, usize), CommandError> {
    let mut index = 0;
    let mut seen = [false; 2];
    while let Some((axis, count, consumed)) = axis_count(&args[index..])? {
        if seen[axis] {
            return Err(CommandError::Usage(USAGE));
        }
        seen[axis] = true;
        preferences.counts[axis] = count;
        index += consumed;
    }
    Ok((preferences, index))
}

pub fn geometry(
    rectangle: RectangleDefinition,
    preferences: Preferences,
    tolerance: Tolerance,
) -> Result<TriangleMesh, GeometryError> {
    let frame = crate::subd_box::base_plane(rectangle)?;
    let [origin, u, _, v] = rectangle.corners;
    TriangleMesh::try_plane_grid(
        frame,
        [0., origin.distance_to(u)?],
        [0., origin.distance_to(v)?],
        preferences.counts[0],
        preferences.counts[1],
        tolerance,
    )
}

fn rectangle_arguments(
    document: &Document,
    args: &[&str],
    plane: Frame3,
    mut preferences: Preferences,
) -> Result<(TriangleMesh, Preferences), CommandError> {
    let mut script = RectangleScript::new(RectangleInput::new(plane));
    let mut index = 0;
    let mut seen = [false; 2];
    while index < args.len() {
        let rest = &args[index..];
        if script.input.anchor().is_none() {
            if let Some(axis) = axis_index(rest[0])
                && std::mem::replace(&mut seen[axis], true)
            {
                return Err(CommandError::Usage(USAGE));
            }
            if let Some(consumed) = preferences.option(rest)? {
                index += consumed;
                continue;
            }
        }
        if !script.has_curve() && box_input::rectangle_option(&mut script.input, rest[0], true) {
            index += 1;
            continue;
        }
        let (consumed, rectangle) = script.value(document, rest, false).map_err(|e| match e {
            CommandError::Usage(_) => CommandError::Usage(USAGE),
            other => other,
        })?;
        index += consumed;
        if let Some(rectangle) = rectangle {
            require_consumed(args, index, USAGE)?;
            preferences.validate()?;
            let mesh = geometry(rectangle, preferences, document.tolerance())?;
            return Ok((mesh, preferences));
        }
    }
    Err(CommandError::Usage(USAGE))
}

// Keep coordinate-only scripts, including separated point coordinates and
// settings after both points, on their original construction-plane path.
fn corner_arguments(
    args: &[&str],
    preferences: Preferences,
) -> Result<(Point3, Point3, Preferences), CommandError> {
    let (mut preferences, index) = leading_counts(args, preferences)?;
    let (first, n) = parse_point(&args[index..])?;
    let (opposite, m) = parse_point(&args[index + n..])?;
    let mut consumed = index + n + m;
    if index == 0 {
        let (options, trailing) = leading_counts(&args[consumed..], preferences)?;
        preferences = options;
        consumed += trailing;
    }
    require_consumed(args, consumed, USAGE)?;
    Ok((first, opposite, preferences))
}

pub(super) struct MeshPlaneCommand(pub std::sync::Arc<remembered::Remembered<Preferences>>);

impl Command for MeshPlaneCommand {
    fn name(&self) -> &'static str {
        "MeshPlane"
    }

    fn run(&self, document: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(document, args, CommandContext::default())
    }

    fn run_in_context(
        &self,
        document: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let plane = context.construction_plane;
        let preferences = self.0.get();
        let (mesh, preferences) = match corner_arguments(args, preferences) {
            Ok((first, opposite, preferences)) => {
                preferences.validate()?;
                let frame = plane.with_origin(first);
                let [x, y, _] = frame.coordinates_of(opposite)?;
                let mesh = TriangleMesh::try_plane_grid(
                    frame,
                    [x.min(0.), x.max(0.)],
                    [y.min(0.), y.max(0.)],
                    preferences.counts[0],
                    preferences.counts[1],
                    document.tolerance(),
                )?;
                (mesh, preferences)
            }
            Err(error @ CommandError::InvalidMeshPlaneFaceCount(_)) => return Err(error),
            Err(_) => rectangle_arguments(document, args, plane, preferences)?,
        };
        let id = document.add_geometry(Geometry::Mesh(mesh))?;
        self.0.set(preferences);
        let [x, y] = preferences.counts;
        Ok(format!("Added mesh plane {id} ({x} × {y} faces)"))
    }
}

impl CommandRegistry {
    pub fn mesh_plane_preferences(&self) -> Preferences {
        self.mesh_plane_preferences.get()
    }

    pub fn remember_mesh_plane_preferences(&self, value: Preferences) {
        self.mesh_plane_preferences.set(value);
    }
}

#[cfg(test)]
pub(super) fn counts_from_arguments(args: &[&str]) -> Result<Preferences, CommandError> {
    let (counts, consumed) = leading_counts(args, Preferences::default())?;
    require_consumed(args, consumed, USAGE)?;
    counts.validate()?;
    Ok(counts)
}
