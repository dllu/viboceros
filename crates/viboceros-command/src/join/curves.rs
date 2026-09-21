//! Curve-chain policy; copying/deletion and selection belong to the command.
use super::*;

pub(super) fn stage(
    sources: &[&viboceros_document::Object],
    tolerance: Tolerance,
    postselected: bool,
    copy_inputs: bool,
) -> Result<JoinPlan, CommandError> {
    let curves = sources
        .iter()
        .map(|o| {
            o.geometry()
                .curve_ref()
                .ok_or(CommandError::UnsupportedJoinGeometry)
                .map(|c| c.to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Closed selections survive untouched and do not affect the curve-family
    // assembly decision, but the entire family has been preflighted above.
    let mut indices = Vec::new();
    let mut open = Vec::new();
    for (index, curve) in curves.into_iter().enumerate() {
        if !curve.as_ref().is_closed()? {
            indices.push(index);
            open.push(curve);
        }
    }
    if open.is_empty() {
        return Err(CommandError::NoOpenCurvesToJoin);
    }
    let components = join_curves(
        &open,
        CurveJoinOptions {
            tolerance: tolerance.absolute(),
            preserve_direction: false,
            style: if postselected {
                viboceros_geometry::CurveJoinStyle::Seeded
            } else {
                viboceros_geometry::CurveJoinStyle::Batch
            },
        },
        tolerance,
    )?;
    let mut copies = Vec::new();
    let mut consumed = Vec::new();
    let mut closed_on_pick = false;
    for component in components {
        let joined = component.source_indices();
        if joined.len() < 2 {
            continue;
        }
        let mut curve = component.curve().clone();
        if postselected && curve.as_ref().is_closed()? {
            closed_on_pick = true;
            if copy_inputs && copy_restores_seed_seam(&open[joined[0]], tolerance)? {
                curve = curve.try_change_closed_seam(
                    component
                        .seed_start_parameter()
                        .expect("a joined seeded component records its mapped seed parameter"),
                )?;
            }
        }
        copies.push((sources[indices[joined[0]]].id(), Geometry::from(curve)));
        consumed.extend(joined.iter().map(|&i| sources[indices[i]].id()));
    }
    let description = format!(
        "Joined {} curve(s) into {} curve(s); {} curve(s) unchanged",
        consumed.len(),
        copies.len(),
        sources.len() - consumed.len()
    );
    Ok(JoinPlan {
        copies,
        consumed,
        description,
        closed_on_pick,
    })
}

fn copy_restores_seed_seam(
    seed: &viboceros_geometry::Curve3,
    tolerance: Tolerance,
) -> Result<bool, CommandError> {
    // The measured standalone two-control-point NURBS policy compares native and
    // linear midpoint positions using the absolute document tolerance. This
    // is not an exact-weight test or a maximum parameter-deviation bound.
    if let viboceros_geometry::Curve3::NurbsCurve(curve) = seed
        && curve.degree() == 1
        && curve.control_points().len() == 2
    {
        let controls = curve.control_points();
        let linear_midpoint = controls[0].point().midpoint(controls[1].point())?;
        let native_midpoint = curve.evaluate(seed.as_ref().parameter_at(0.5)?)?;
        Ok(native_midpoint.distance_to(linear_midpoint)? <= tolerance.absolute())
    } else {
        Ok(true)
    }
}
