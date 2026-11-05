//! Surface partitions, source preservation and complete history comparisons.
use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    #[serde(flatten)]
    pub profile: revolve_deformable::Fixture,
    pub deformable: bool,
    pub crease_splitting: bool,
    pub split_at_tangents: bool,
}
impl Fixture {
    pub fn input(&self) -> String {
        let f = &self.profile;
        let p = |v: [f64; 3]| format!("{},{},{}", v[0], v[1], v[2]);
        let yes = |b| if b { "Yes" } else { "No" };
        format!(
            "Revolve {} {} {} StartAngle={} PointCount={} Deformable={} SplitAtTangents={} DeleteInput={} Output=Surface",
            p(f.origin),
            p(f.axis_end),
            f.sweep_degrees,
            f.start_degrees,
            f.point_count,
            yes(self.deformable),
            yes(self.split_at_tangents),
            yes(f.delete_input)
        )
    }
    pub fn configure(
        &self,
        commands: &CommandRegistry,
        doc: &mut Document,
    ) -> Result<(), ProbeError> {
        commands.execute(
            doc,
            if self.crease_splitting {
                "CreaseSplitting Enable"
            } else {
                "CreaseSplitting Disable"
            },
        )?;
        Ok(())
    }
}
mod record;
pub use record::{adjacency, boundary_samples, geometry_record, snapshot};

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned revolution workflow required",
        ));
    }
    let (mut doc, source) = revolve_deformable::setup(&f.profile, tolerance)?;
    let commands = CommandRegistry::with_builtins();
    f.configure(&commands, &mut doc)?;
    let before = snapshot(&doc, source)?;
    commands.execute(&mut doc, &f.input())?;
    let after = snapshot(&doc, source)?;
    let adjacency = adjacency(&doc, source)?;
    let boundaries = boundary_samples(&doc, source)?;
    commands.execute(&mut doc, "Undo")?;
    let undo = snapshot(&doc, source)?;
    commands.execute(&mut doc, "Redo")?;
    let redo = snapshot(&doc, source)?;
    Ok((
        json!({"success":true,"before":before,"after":after,"undo":undo,"redo":redo,"adjacency":adjacency,"boundaries":boundaries}),
        0,
    ))
}
