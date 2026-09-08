//! Self-seeded command-option sequences, independent of Rhino's prior session.
use super::*;

#[cfg(test)]
mod bezier_tests;
#[cfg(test)]
mod mesh_tests;
#[cfg(test)]
mod nurbs_tests;
#[cfg(test)]
mod postselection_tests;
#[cfg(test)]
mod tests;
use crate::conversion::{ConversionFixture, delete_option, run_command};
use crate::curve_join_close::CurveInput;
use crate::object_source::{ObjectSource, VertexSource};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionCommand {
    ConvertToBeziers,
    ConvertToSingleSpans,
    ToNURBS,
    MeshToNURB,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Direction {
    U,
    V,
    Both,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversionOptions {
    #[serde(flatten)]
    pub geometry: ConversionFixture,
    pub direction: Option<Direction>,
    #[serde(default)]
    pub toggles: u8,
    pub trim_triangular_faces: Option<bool>,
    pub use_ngons: Option<bool>,
}

impl ConversionOptions {
    fn selected_sources(&self) -> impl Iterator<Item = &ObjectSource> {
        self.geometry
            .sources
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                self.geometry
                    .selected
                    .as_ref()
                    .is_none_or(|s| s.contains(i))
            })
            .map(|(_, s)| s)
    }
}

fn converts_to_nurbs(source: &ObjectSource) -> bool {
    match source {
        ObjectSource::Vertices(VertexSource::Mesh { .. }) => true,
        ObjectSource::Curved(c) => matches!(c.as_ref(), plane_arrays::ArraySource::Curve(c)
            if !matches!(c, CurveInput::Nurbs { .. } | CurveInput::Ellipse { .. })),
        _ => false,
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Step {
    pub command: ConversionCommand,
    #[serde(flatten)]
    pub conversion: ConversionOptions,
    #[serde(default)]
    pub undo_after: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversionSessionFixture {
    pub steps: Vec<Step>,
}

fn execute(
    f: &ConversionOptions,
    command: ConversionCommand,
    undo: bool,
    tolerance: Tolerance,
    registry: &CommandRegistry,
) -> Result<Value, ProbeError> {
    if f.toggles > 3 || (matches!(f.direction, Some(Direction::Both)) && f.toggles > 0) {
        return Err(ProbeError::FixtureInvariant(
            "invalid conversion toggle count/direction",
        ));
    }
    if command != ConversionCommand::ConvertToSingleSpans
        && (f.direction.is_some() || f.toggles > 0)
    {
        return Err(ProbeError::FixtureInvariant(
            "direction options require ConvertToSingleSpans",
        ));
    }
    let direction = f
        .direction
        .map(|d| format!(" Direction={d:?}"))
        .unwrap_or_default();
    if f.trim_triangular_faces.is_some()
        && !matches!(
            command,
            ConversionCommand::ToNURBS | ConversionCommand::MeshToNURB
        )
    {
        return Err(ProbeError::FixtureInvariant(
            "mesh options require a mesh conversion command",
        ));
    }
    if f.use_ngons.is_some() && command != ConversionCommand::MeshToNURB {
        return Err(ProbeError::FixtureInvariant(
            "n-gon options require MeshToNURB",
        ));
    }
    if command == ConversionCommand::MeshToNURB && f.geometry.delete_input.is_some() {
        return Err(ProbeError::FixtureInvariant(
            "MeshToNURB has no deletion choice",
        ));
    }
    let mesh = f
        .trim_triangular_faces
        .map(|t| format!(" TrimTriangularFaces={}", if t { "Yes" } else { "No" }))
        .unwrap_or_default();
    let script = format!(
        "{command:?}{direction}{mesh}{}{}{}",
        f.use_ngons
            .map(|n| format!(" UseNgons={}", if n { "Yes" } else { "No" }))
            .unwrap_or_default(),
        delete_option(f.geometry.delete_input),
        " Toggle".repeat(usize::from(f.toggles))
    );
    Ok(run_command(&f.geometry, tolerance, registry, &script, undo)?.0)
}

pub(super) fn run_single(
    f: &ConversionOptions,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.toggles > 0 && f.direction.is_none() {
        return Err(ProbeError::FixtureInvariant(
            "standalone Toggle probe requires explicit direction",
        ));
    }
    Ok((
        execute(
            f,
            ConversionCommand::ConvertToSingleSpans,
            false,
            tolerance,
            &CommandRegistry::with_builtins(),
        )?,
        0,
    ))
}

pub(super) fn run_nurbs(
    f: &ConversionOptions,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    Ok((
        execute(
            f,
            ConversionCommand::ToNURBS,
            false,
            tolerance,
            &CommandRegistry::with_builtins(),
        )?,
        0,
    ))
}

pub(super) fn run_mesh(
    f: &ConversionOptions,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    Ok((
        execute(
            f,
            ConversionCommand::MeshToNURB,
            false,
            tolerance,
            &CommandRegistry::with_builtins(),
        )?,
        0,
    ))
}

pub(super) fn run(
    f: &ConversionSessionFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let invalid =
        || ProbeError::FixtureInvariant("conversion session requires at most 32 self-seeded steps");
    if f.steps.is_empty() || f.steps.len() > 32 {
        return Err(invalid());
    }
    let mut seeded = BTreeSet::new();
    let mut direction = None;
    let mut mesh_seeded = false;
    for step in &f.steps {
        if seeded.insert(step.command)
            && ((step.command != ConversionCommand::MeshToNURB
                && step.conversion.geometry.delete_input.is_none())
                || (step.command == ConversionCommand::MeshToNURB
                    && (step.conversion.trim_triangular_faces.is_none()
                        || step.conversion.use_ngons.is_none()))
                || (step.command == ConversionCommand::ConvertToSingleSpans
                    && step.conversion.direction.is_none())
                || (step.command == ConversionCommand::ConvertToBeziers
                    && step.conversion.geometry.cancel)
                || (step.command == ConversionCommand::ToNURBS
                    && (step.conversion.geometry.cancel
                        || !step.conversion.selected_sources().any(converts_to_nurbs))))
        {
            return Err(invalid());
        }
        if step.command == ConversionCommand::ToNURBS
            && step
                .conversion
                .selected_sources()
                .any(|s| matches!(s, ObjectSource::Vertices(VertexSource::Mesh { .. })))
        {
            if !mesh_seeded && step.conversion.trim_triangular_faces.is_none() {
                return Err(invalid());
            }
            mesh_seeded = true;
        }
        if step.command == ConversionCommand::ConvertToSingleSpans {
            direction = step.conversion.direction.or(direction);
            if step.conversion.toggles > 0 {
                direction = Some(match direction {
                    Some(Direction::U) if step.conversion.toggles % 2 == 1 => Direction::V,
                    Some(Direction::V) if step.conversion.toggles % 2 == 1 => Direction::U,
                    Some(d @ (Direction::U | Direction::V)) => d,
                    _ => return Err(invalid()),
                });
            }
        }
    }
    let registry = CommandRegistry::with_builtins();
    let states = f
        .steps
        .iter()
        .map(|s| execute(&s.conversion, s.command, s.undo_after, tolerance, &registry))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((json!({"states":states}), 0))
}
