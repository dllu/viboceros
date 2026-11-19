//! Attach one cap in the supplied surface's parameter space.
use super::*;

impl Brep {
    /// Caps all naked-edge cycles with one trimmed face on the supplied support.
    /// Existing spatial edges and vertices are retained. Every edge must have
    /// an exact inverse proposal whose complete image certifies at model
    /// tolerance. The cycles must form one outer boundary and any nested holes.
    pub fn try_cap_on_surface(
        &self,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Option<Self>, GeometryError> {
        let cycles = boundary::cycles(self);
        let naked = self
            .edge_use_counts()
            .into_iter()
            .filter(|&count| count == 1)
            .count();
        if cycles.is_empty() || cycles.iter().map(Vec::len).sum::<usize>() != naked {
            return Ok(None);
        }
        if cycles.len() > MAX_CAP_LOOPS {
            return cap_limit();
        }
        let Some(mut certificate) = surface.prepare_surface_curve_bounds()? else {
            return Ok(None);
        };
        let mut budget = WorkBudget(MAX_CAP_CONTROLS);
        let mut loops = Vec::with_capacity(cycles.len());
        for edges in cycles {
            let mut trims = Vec::with_capacity(edges.len());
            for (index, reversed) in edges {
                let edge = &self.edges[index];
                budget.charge(edge.curve.control_points().len())?;
                let Ok(curve) = surface.try_pullback_exact_curve(&edge.curve, tolerance) else {
                    return Ok(None);
                };
                if certificate
                    .bound(&curve, &edge.curve, tolerance.absolute())?
                    .is_none()
                {
                    return Ok(None);
                }
                trims.push(BrepTrim::try_new(
                    oriented_edge_vertices(edge, reversed),
                    Some(index),
                    reversed,
                    if reversed { curve.reversed()? } else { curve },
                    BrepTrimType::Mated,
                    SurfaceIso::NotIso,
                    [0.; 2],
                )?);
            }
            let boundary = BrepLoop::try_new(BrepLoopType::Outer, trims)?;
            charge_samples(&boundary, &mut budget)?;
            let area = sampled_loop_signed_area(&boundary)?;
            if area == 0. {
                return Ok(None);
            }
            let log_size = log_sampled_area(&boundary, &mut budget)?;
            loops.push((boundary, area.is_sign_negative(), log_size));
        }
        let outer = loops
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.2.total_cmp(&b.2))
            .map(|(index, _)| index)
            .expect("nonempty cap cycles");
        loops.swap(0, outer);
        let reverse = loops[0].1;
        let mut boundaries = Vec::with_capacity(loops.len());
        for (index, (mut boundary, clockwise, _)) in loops.into_iter().enumerate() {
            if clockwise != (reverse ^ (index != 0)) {
                return Ok(None);
            }
            if index != 0 {
                boundary.loop_type = BrepLoopType::Inner;
            }
            if reverse {
                reverse_cap_loop(&mut boundary)?;
            }
            boundaries.push(boundary);
        }
        if !valid_region(&boundaries.iter().collect::<Vec<_>>(), &mut budget)? {
            return Ok(None);
        }
        let face = BrepFace::try_new(surface.clone(), reverse, boundaries)?;
        let closed = self.with_attached_caps(vec![face], tolerance)?;
        Ok(closed.is_solid().then_some(closed))
    }
}
