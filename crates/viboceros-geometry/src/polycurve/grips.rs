//! Shared junction and seam grips, rebuilt in their original parent intervals.
use super::*;
use crate::{NurbsCurve, WeightedPoint3};
use std::collections::BTreeSet;
impl PolyCurve3 {
    pub(super) fn grip_layout(&self) -> Result<(Vec<Point3>, Vec<Vec<usize>>), GeometryError> {
        let mut points = Vec::new();
        let mut mappings = Vec::new();
        for segment in &self.segments {
            let mut mapping = Vec::new();
            for (i, p) in segment.extract_point_locations()?.into_iter().enumerate() {
                let index = if i == 0
                    && points
                        .last()
                        .is_some_and(|&last| curve_points_coincident(last, p))
                {
                    points.len() - 1
                } else {
                    points.push(p);
                    points.len() - 1
                };
                mapping.push(index);
            }
            mappings.push(mapping);
        }
        if self.is_closed()?
            && points
                .first()
                .zip(points.last())
                .is_some_and(|(&a, &b)| curve_points_coincident(a, b))
        {
            let last = points.len() - 1;
            points.pop();
            for mapping in &mut mappings {
                for i in mapping {
                    if *i == last {
                        *i = 0;
                    }
                }
            }
        }
        Ok((points, mappings))
    }
    /// Maps logical grips once, sharing junctions and closed seams. Real edits
    /// rebuild NURBS leaves in the original parent intervals; no-ops retain the
    /// complete representation, including numerically distinct seam aliases.
    pub fn with_mapped_grips(
        &self,
        indices: &BTreeSet<usize>,
        map: impl Fn(Point3) -> Result<Point3, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let (points, mappings) = self.grip_layout()?;
        if let Some(&index) = indices.last().filter(|&&i| i >= points.len()) {
            return Err(GeometryError::InvalidControlPointIndex {
                index,
                count: points.len(),
            });
        }
        let mapped = points
            .iter()
            .enumerate()
            .map(|(i, &p)| if indices.contains(&i) { map(p) } else { Ok(p) })
            .collect::<Result<Vec<_>, _>>()?;
        if mapped == points {
            return Ok(self.clone());
        }
        let segments = self
            .segments
            .iter()
            .enumerate()
            .map(|(i, segment)| {
                let source = segment
                    .to_nurbs()?
                    .try_reparameterized(self.parameters[i]..=self.parameters[i + 1])?;
                let mapping = &mappings[i];
                let controls = source
                    .control_points()
                    .iter()
                    .enumerate()
                    .map(|(j, c)| {
                        WeightedPoint3::try_new(mapped[mapping[j % mapping.len()]], c.weight())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mut result = NurbsCurve::try_new_rational(
                    source.degree(),
                    controls,
                    source.knots().to_vec(),
                )?;
                if source.has_rational_representation() {
                    result = result.with_rational_representation();
                }
                Ok(CurveSegment3::NurbsCurve(result))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Self::try_with_segment_domains(segments, self.parameters.clone())
    }
}
