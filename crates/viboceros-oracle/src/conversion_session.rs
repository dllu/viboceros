//! Self-seeded command-option sequences, independent of Rhino's prior session.
use super::*;

#[cfg(test)]
mod tests;
use crate::conversion::{ConversionFixture, delete_option, run_command};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionCommand {
    ConvertToBeziers,
    ConvertToSingleSpans,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Direction {
    U,
    V,
    Both,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SingleSpanFixture {
    #[serde(flatten)]
    pub geometry: ConversionFixture,
    pub direction: Option<Direction>,
    #[serde(default)]
    pub toggles: u8,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Step {
    pub command: ConversionCommand,
    #[serde(flatten)]
    pub conversion: SingleSpanFixture,
    #[serde(default)]
    pub undo_after: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversionSessionFixture {
    pub steps: Vec<Step>,
}

fn execute(
    f: &SingleSpanFixture,
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
    if command == ConversionCommand::ConvertToBeziers && (f.direction.is_some() || f.toggles > 0) {
        return Err(ProbeError::FixtureInvariant(
            "Bezier conversion has no direction option",
        ));
    }
    let direction = f
        .direction
        .map(|d| format!(" Direction={d:?}"))
        .unwrap_or_default();
    let script = format!(
        "{command:?}{direction}{}{}",
        delete_option(f.geometry.delete_input),
        " Toggle".repeat(usize::from(f.toggles))
    );
    Ok(run_command(&f.geometry, tolerance, registry, &script, undo)?.0)
}

pub(super) fn run_single(
    f: &SingleSpanFixture,
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
    for step in &f.steps {
        if seeded.insert(step.command)
            && (step.conversion.geometry.delete_input.is_none()
                || (step.command == ConversionCommand::ConvertToSingleSpans
                    && step.conversion.direction.is_none()))
        {
            return Err(invalid());
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
