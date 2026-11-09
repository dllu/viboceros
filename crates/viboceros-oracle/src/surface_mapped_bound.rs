//! Complete bounds with supplied separable normalized U/V correspondences.
use super::*;

/// Accept authored inputs and the existing fitted/native surface readback.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Definition {
    Authored(NurbsSurfaceDefinition),
    Readback {
        degree: [usize; 2],
        control_count: [usize; 2],
        control_points: Vec<ControlPoint>,
        knots_u: Vec<f64>,
        knots_v: Vec<f64>,
        #[serde(default)]
        domain_u: Option<[f64; 2]>,
        #[serde(default)]
        domain_v: Option<[f64; 2]>,
    },
}
impl Definition {
    fn surface(&self) -> Result<NurbsSurface, GeometryError> {
        match self {
            Self::Authored(definition) => nurbs_surface_from_definition(definition),
            Self::Readback {
                degree,
                control_count,
                control_points,
                knots_u,
                knots_v,
                domain_u,
                domain_v,
            } => nurbs_surface_from_definition(&NurbsSurfaceDefinition {
                degree_u: degree[0],
                degree_v: degree[1],
                control_point_count_u: control_count[0],
                control_point_count_v: control_count[1],
                control_points: control_points.clone(),
                knots_u: knots_u.clone(),
                knots_v: knots_v.clone(),
                domain_u: *domain_u,
                domain_v: *domain_v,
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub first: Definition,
    pub second: Definition,
    pub correspondence: [Vec<[f64; 2]>; 2],
    #[serde(default)]
    pub absolute_tolerance: Option<f64>,
}

pub(super) fn run(
    fixture: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let first = fixture.first.surface()?;
    let second = fixture.second.surface()?;
    let limit = fixture.absolute_tolerance.unwrap_or(tolerance.absolute());
    let build = || {
        first.mapped_surface_deviation_bound(
            &second,
            fixture.correspondence.each_ref().map(Vec::as_slice),
            limit,
        )
    };
    let mut bound = build()?;
    let started = Instant::now();
    for _ in 0..iterations {
        bound = black_box(build()?);
    }
    let elapsed =
        u64::try_from(started.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?;
    Ok((
        json!({"maximum_deviation_bound":bound,"certified":bound.is_some()}),
        elapsed,
    ))
}
