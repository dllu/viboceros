//! Bounded public BendSpaceMorph point-map protocol.
use super::*;
use viboceros_geometry::BendPointMorph;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BendPointFixture {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub through: [f64; 3],
    pub angle: Option<f64>,
    pub straight: bool,
    pub symmetric: bool,
    pub points: Vec<[f64; 3]>,
}

pub(super) fn run(
    fixture: &BendPointFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if fixture.points.is_empty()
        || fixture.points.len() > 256
        || fixture.start == fixture.end
        || fixture
            .angle
            .is_some_and(|angle| !angle.is_finite() || angle.abs() > 4. * std::f64::consts::PI)
        || [fixture.start, fixture.end, fixture.through]
            .iter()
            .chain(fixture.points.iter())
            .flatten()
            .any(|value| !value.is_finite() || value.abs() > 1e6)
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid bounded Bend point recipe",
        ));
    }
    let morph = BendPointMorph::try_new(
        Point3::try_from(fixture.start)?,
        Point3::try_from(fixture.end)?,
        Point3::try_from(fixture.through)?,
        fixture.angle,
        fixture.straight,
        fixture.symmetric,
        tolerance,
    );
    let points = fixture
        .points
        .iter()
        .copied()
        .map(Point3::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let (mapped, elapsed) = measure(iterations, || {
        points
            .iter()
            .map(|point| match &morph {
                Ok(morph) => morph.morph_point(*point).map(Point3::to_array),
                // The public SDK returns unchanged points for an invalid morph.
                Err(_) => Ok(point.to_array()),
            })
            .collect::<Result<Vec<_>, GeometryError>>()
    })?;
    Ok((json!({"valid": morph.is_ok(), "points": mapped}), elapsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_bend_recipes_reject_nonfinite_and_oversized_input() {
        let fixture = BendPointFixture {
            start: [0.; 3],
            end: [0., 0., 10.],
            through: [10., 0., 10.],
            angle: None,
            straight: false,
            symmetric: false,
            points: vec![[0., 0., 5.]],
        };
        assert!(run(&fixture, 1, Tolerance::DEFAULT).is_ok());
        let bad = [
            BendPointFixture {
                points: vec![],
                ..fixture.clone()
            },
            BendPointFixture {
                points: vec![[0.; 3]; 257],
                ..fixture.clone()
            },
            BendPointFixture {
                points: vec![[f64::INFINITY, 0., 0.]],
                ..fixture.clone()
            },
            BendPointFixture {
                start: fixture.end,
                ..fixture.clone()
            },
            BendPointFixture {
                angle: Some(f64::NAN),
                ..fixture.clone()
            },
            BendPointFixture {
                through: [1e7, 0., 0.],
                ..fixture.clone()
            },
        ];
        for fixture in bad {
            assert!(run(&fixture, 1, Tolerance::DEFAULT).is_err());
        }
    }
}
