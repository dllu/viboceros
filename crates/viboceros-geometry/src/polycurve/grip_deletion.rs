//! Delete controls in their leaf bases without flattening the composite.
use super::*;
use std::collections::BTreeSet;
impl PolyCurve3 {
    pub fn try_delete_grips(
        &self,
        indices: &BTreeSet<usize>,
    ) -> Result<Option<Self>, GeometryError> {
        self.delete_grips(indices, false)
    }
    pub fn try_delete_edit_grips(
        &self,
        indices: &BTreeSet<usize>,
    ) -> Result<Option<Self>, GeometryError> {
        self.delete_grips(indices, true)
    }
    fn delete_grips(
        &self,
        indices: &BTreeSet<usize>,
        edit: bool,
    ) -> Result<Option<Self>, GeometryError> {
        let count = self.grip_layout()?.0.len();
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices.len() == count {
            return Ok(None);
        }
        if indices.is_empty() || count - indices.len() < 2 {
            return Ok(Some(self.clone()));
        }
        if self.is_closed()? && indices.contains(&0) {
            return Err(invalid(
                "deleting a closed composite seam requires a joint edit",
            ));
        }
        let mut result = self.clone();
        result.segments = result
            .segments
            .iter()
            .enumerate()
            .map(|(i, s)| s.try_reparameterized(result.parameters[i]..=result.parameters[i + 1]))
            .collect::<Result<_, _>>()?;
        for &index in indices.iter().rev() {
            if result.grip_layout()?.0.len() <= 2 {
                continue;
            }
            let owners = result.control_point_owners(index)?;
            if owners.len() > 1 {
                let small = owners
                    .iter()
                    .filter_map(|&(i, _)| {
                        result.segments[i]
                            .to_nurbs()
                            .ok()
                            .filter(|c| c.control_points().len() == 2)
                            .map(|_| i)
                    })
                    .next_back();
                let Some(owner) = small else {
                    return Err(invalid(
                        "deleting a junction between nonlinear leaves requires a joint edit",
                    ));
                };
                if owner != 0 && owner + 1 != result.segments.len() {
                    return Err(invalid(
                        "deleting an interior two-control leaf would disconnect the composite",
                    ));
                }
                let leaf = result.segments.remove(owner);
                if owner == 0 {
                    result.parameters.remove(0);
                    result.segments[0] = join_endpoint(
                        &result.segments[0],
                        leaf.evaluate(*leaf.domain().start())?,
                        true,
                    )?;
                } else {
                    result.parameters.remove(owner + 1);
                    let last = result.segments.len() - 1;
                    result.segments[last] = join_endpoint(
                        &result.segments[last],
                        leaf.evaluate(*leaf.domain().end())?,
                        false,
                    )?;
                }
            } else {
                let (owner, local) = owners[0];
                let curve = result.segments[owner].to_nurbs()?;
                if curve.control_points().len() == 2 {
                    if owner > 0 && owner + 1 < result.segments.len() {
                        return Err(invalid(
                            "deleting an interior two-control leaf would disconnect the composite",
                        ));
                    }
                    result.segments.remove(owner);
                    result
                        .parameters
                        .remove(if owner == 0 { 0 } else { owner + 1 });
                } else {
                    let picked = BTreeSet::from([local]);
                    let next = (if edit {
                        curve.try_delete_edit_grips(&picked)
                    } else {
                        curve.try_delete_grips(&picked)
                    })?
                    .ok_or_else(|| invalid("deleting a whole nonlinear composite leaf"))?;
                    if next == curve {
                        continue;
                    }
                    let old = curve.domain();
                    let new = next.domain();
                    let length = scaled_ratio(
                        result.parameters[owner + 1] - result.parameters[owner],
                        *new.end() - *new.start(),
                        *old.end() - *old.start(),
                    )?;
                    let end = result.parameters[owner] + length;
                    let shift = end - result.parameters[owner + 1];
                    for parameter in &mut result.parameters[owner + 1..] {
                        *parameter += shift;
                    }
                    result.segments[owner] = CurveSegment3::NurbsCurve(next);
                }
            }
            let mut parameters = vec![*result.segments[0].domain().start()];
            for segment in &result.segments {
                let d = segment.domain();
                parameters.push(parameters.last().unwrap() + (*d.end() - *d.start()));
            }
            result = Self::try_with_segment_domains(result.segments, parameters)?;
        }
        Ok(Some(result))
    }
}

fn join_endpoint(
    segment: &CurveSegment3,
    point: Point3,
    start: bool,
) -> Result<CurveSegment3, GeometryError> {
    let mut result = segment.clone();
    let domain = segment.domain();
    if let CurveSegment3::NurbsCurve(c) = segment {
        let t = c.closest_parameter(point, Tolerance::try_new(f64::MIN_POSITIVE, 1e-14, 1e-12)?)?;
        if t > *domain.start() && t < *domain.end() {
            let trimmed = if start {
                t..=*domain.end()
            } else {
                *domain.start()..=t
            };
            result = result.try_trimmed(trimmed)?.try_reparameterized(domain)?;
        }
    }
    result.try_with_endpoints(start.then_some(point), (!start).then_some(point))
}
