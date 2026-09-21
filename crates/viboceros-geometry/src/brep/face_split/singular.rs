//! Subdivide collapsed UV boundaries without inventing spatial edges/vertices.
use super::*;

pub(super) fn split(
    face: &mut BrepFace,
    axis: usize,
    cut: Real,
    budget: &mut Budget,
) -> Result<(), GeometryError> {
    if !face
        .loops
        .iter()
        .flat_map(|l| &l.trims)
        .any(|t| t.edge.is_none())
    {
        return Ok(());
    }
    let mut plans = BTreeMap::new();
    {
        let frame = face.local_parameter_frame()?;
        let local_cut = crate::parameter::exact_difference(cut, frame.origin[axis]).ok_or(
            GeometryError::InvalidBrepTopology {
                context: "face partition knot cannot be localized exactly",
            },
        )?;
        for (ring_index, (ring, local_ring)) in face.loops.iter().zip(&frame.face.loops).enumerate()
        {
            for (trim_index, (trim, local)) in ring.trims.iter().zip(&local_ring.trims).enumerate()
            {
                if trim.edge.is_some() || rings::side(&trim.curve, axis, cut)?.is_some() {
                    continue;
                }
                let roots = curves::crossing_parameters(&local.curve, axis, local_cut, budget)?;
                let original = curves::lift(&trim.curve)?;
                let domain = original.domain();
                let mut breaks = vec![*domain.start()];
                breaks.extend(roots);
                breaks.push(*domain.end());
                let linear = certificate::linear_endpoints(&original).is_some();
                let mut pieces = Vec::new();
                for pair in breaks.windows(2) {
                    budget.charge(original.control_points().len().saturating_mul(4))?;
                    let mut piece = original.try_trimmed(pair[0]..=pair[1])?;
                    if linear {
                        // Proposal only: the whole ordered-locus certificate
                        // below must authorize both rounded endpoint repairs.
                        let mut controls = piece.control_points().to_vec();
                        let last = controls.len() - 1;
                        for (slot, t) in [(0, pair[0]), (last, pair[1])] {
                            controls[slot] = WeightedPoint3::try_new(
                                original.evaluate(t)?,
                                controls[slot].weight(),
                            )?;
                        }
                        piece = NurbsCurve::try_new_rational(
                            piece.degree(),
                            controls,
                            piece.knots().to_vec(),
                        )?;
                    }
                    pieces.push(piece);
                }
                curves::exact_partition(&pieces.iter().collect::<Vec<_>>(), &original, budget)?;
                let pieces = pieces
                    .into_iter()
                    .map(|piece| {
                        let mut result = trim.clone();
                        result.curve = NurbsCurve2::try_new_rational(
                            piece.degree(),
                            piece
                                .control_points()
                                .iter()
                                .map(|p| {
                                    WeightedPoint2::try_new(
                                        Point2::try_new(p.point().x(), p.point().y())?,
                                        p.weight(),
                                    )
                                })
                                .collect::<Result<_, _>>()?,
                            piece.knots().to_vec(),
                        )?;
                        Ok(result)
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                plans.insert((ring_index, trim_index), pieces);
            }
        }
    }
    if plans.is_empty() {
        return Ok(());
    }
    for (ring_index, ring) in face.loops.iter_mut().enumerate() {
        let mut trims = Vec::new();
        for (trim_index, trim) in std::mem::take(&mut ring.trims).into_iter().enumerate() {
            if let Some(pieces) = plans.remove(&(ring_index, trim_index)) {
                trims.extend(pieces);
            } else {
                trims.push(trim);
            }
        }
        ring.trims = trims;
    }
    Ok(())
}
