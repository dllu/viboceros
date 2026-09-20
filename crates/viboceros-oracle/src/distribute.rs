//! Actual distribution of named source geometry, with identity and group checks.
use super::*;
use crate::object_layout;
#[cfg(test)]
use crate::object_layout::sample;
use viboceros_command::CommandContext;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct DistributeFixture {
    #[serde(flatten)]
    pub layout: object_layout::ObjectLayoutFixture,
    pub direction: Direction,
    pub mode: Mode,
    pub spacing: Option<f64>,
    pub references: Option<[[f64; 3]; 2]>,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Direction {
    XAxis,
    YAxis,
    ZAxis,
    #[serde(rename = "Direction")]
    Points,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Mode {
    Gap,
    Center,
}

pub(super) fn run(f: &DistributeFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid distribution fixture");
    let direction = if f.direction == Direction::Points {
        let points = f.references.ok_or_else(invalid)?;
        format!(
            "Direction {}",
            points
                .iter()
                .map(|p| format!("{},{},{}", p[0], p[1], p[2]))
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else {
        format!("{:?}", f.direction)
    };
    let spacing = f
        .spacing
        .map_or_else(|| "Automatic".to_owned(), |v| v.to_string());
    let command = format!("Distribute {direction} Mode={:?} Spacing={spacing}", f.mode);
    object_layout::run(&f.layout, tolerance, 3, |document, plane| {
        match CommandRegistry::with_builtins().execute_in_context(
            document,
            &command,
            CommandContext {
                construction_plane: plane,
            },
        ) {
            Ok(_) => Ok(true),
            Err(CommandError::InsufficientDistributionObjects { .. }) => Ok(false),
            Err(error) => Err(error.into()),
        }
    })
}
