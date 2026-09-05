use super::*;

/// Exact degree/knot matching, followed by Rhino's common-basis end-weight policy.
pub(super) fn prepare(curves: &[NurbsCurve]) -> Result<Vec<NurbsCurve>, GeometryError> {
    let degree = curves.iter().map(NurbsCurve::degree).max().unwrap();
    let mut sections = Vec::with_capacity(curves.len());
    let limit = || GeometryError::LoftResourceLimit {
        context: "compatible section controls",
        maximum: MAX_LOFT_SECTION_CONTROLS,
    };
    for curve in curves {
        // Predict the exact elevated/clamped control count before a dense solve.
        let count = degree
            + 1
            + curve
                .knots()
                .chunk_by(|a, b| a == b)
                .filter(|g| g[0] > *curve.domain().start() && g[0] < *curve.domain().end())
                .map(|g| degree - curve.degree() + g.len())
                .sum::<usize>();
        if count > MAX_LOFT_SECTION_CONTROLS {
            return Err(limit());
        }
        let scale = curve
            .control_points()
            .iter()
            .map(|c| c.weight().abs())
            .fold(0.0, Real::max);
        let normalized = NurbsCurve::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|c| WeightedPoint3::try_new(c.point(), c.weight() / scale))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knots().to_vec(),
        )?;
        sections.push(
            normalized
                .clamped_to_active_domain()?
                .try_change_degree(degree, false)?
                .try_reparameterized(0.0..=1.0)?,
        );
    }
    let mut union = sections
        .iter()
        .flat_map(|c| {
            c.knots()
                .chunk_by(|a, b| a == b)
                .filter(|g| g[0] > 0.0 && g[0] < 1.0)
                .map(|g| (g[0], g.len()))
        })
        .collect::<Vec<_>>();
    union.sort_by(|a, b| a.0.total_cmp(&b.0));
    let union = union
        .chunk_by(|a, b| a.0 == b.0)
        .map(|g| (g[0].0, g.iter().map(|x| x.1).max().unwrap()))
        .collect::<Vec<_>>();
    if degree + 1 + union.iter().map(|x| x.1).sum::<usize>() > MAX_LOFT_SECTION_CONTROLS {
        return Err(limit());
    }
    for section in &mut sections {
        for &(knot, multiplicity) in &union {
            *section = section.try_insert_knot(knot, multiplicity)?;
        }
        *section = section.try_normalized_end_weights()?;
    }
    // Rhino normalizes weights after structural matching and retains the first
    // normalized section's knots for the whole control net. For unequal endpoint
    // weight ratios this is not necessarily shape-preserving on later sections.
    let knots = sections[0].knots().to_vec();
    for section in &mut sections[1..] {
        *section =
            NurbsCurve::try_new_rational(degree, section.control_points().to_vec(), knots.clone())?;
    }
    Ok(sections)
}
