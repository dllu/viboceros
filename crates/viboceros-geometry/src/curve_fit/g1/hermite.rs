//! C1 cubic spline interpolating positions and derivatives on a unit chart.
use crate::{GeometryError, NurbsCurve, Real};

pub(crate) fn hermite_cubics(
    source: &NurbsCurve,
    cuts: &[Real],
) -> Result<NurbsCurve, GeometryError> {
    let jets = cuts
        .iter()
        .map(|t| source.evaluate_with_derivative(*t))
        .collect::<Result<Vec<_>, _>>()?;
    let mut controls = vec![jets[0].0];
    let mut knots = vec![0.; 4];
    for (i, pair) in cuts.windows(2).enumerate() {
        let width = (pair[1] - pair[0]) / 3.;
        controls.push(jets[i].0.translated(jets[i].1.scaled(width)?)?);
        controls.push(jets[i + 1].0.translated(jets[i + 1].1.scaled(-width)?)?);
        if i > 0 {
            knots.extend([pair[0]; 2]);
        }
    }
    controls.push(jets.last().unwrap().0);
    knots.extend([1.; 4]);
    NurbsCurve::try_new(3, controls, knots)
}
