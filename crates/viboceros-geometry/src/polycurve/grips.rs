//! Shared junction and seam grips, rebuilt in their original parent intervals.
use super::*;
use crate::{NurbsCurve, WeightedPoint3};
use std::collections::BTreeSet;
impl PolyCurve3 {
    /// Greville stations belong to each native leaf, with one shared station
    /// at a junction. Elevating a joined curve to one NURBS adds extra stations.
    pub fn edit_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        let (mut points, mappings) = self.grip_layout()?;
        for (segment, mapping) in self.segments.iter().zip(mappings) {
            let stations = segment.to_nurbs()?.edit_points()?;
            if stations.len() < mapping.len() {
                return Err(invalid("composite edit-station layout"));
            }
            for (local, index) in mapping.into_iter().enumerate() {
                points[index] = stations[local].point;
            }
        }
        Ok(points)
    }

    /// Reconstruct only the affected leaf bases, retaining their parent charts.
    pub fn with_mapped_edit_grips(
        &self,
        indices: &BTreeSet<usize>,
        map: impl Fn(Point3) -> Result<Point3, GeometryError>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let (points, mappings) = self.grip_layout()?;
        if let Some(&index) = indices.last().filter(|&&i| i >= points.len()) {
            return Err(GeometryError::InvalidControlPointIndex {
                index,
                count: points.len(),
            });
        }
        let mut segments = Vec::new();
        for (segment, mapping) in self.segments.iter().zip(mappings) {
            if !mapping.iter().any(|i| indices.contains(i)) {
                segments.push(segment.clone());
                continue;
            }
            let curve = segment.to_nurbs()?;
            let mut stations = curve
                .edit_points()?
                .into_iter()
                .map(|p| p.point)
                .collect::<Vec<_>>();
            for (local, index) in mapping.iter().enumerate() {
                if indices.contains(index) {
                    stations[local] = map(stations[local])?;
                }
            }
            for i in mapping.len()..stations.len() {
                stations[i] = stations[i % mapping.len()];
            }
            let result = curve.try_with_edit_points(&stations, tolerance)?;
            segments.push(if result == curve {
                segment.clone()
            } else {
                CurveSegment3::NurbsCurve(result)
            });
        }
        Self::try_with_segment_domains(segments, self.parameters.clone())
    }

    /// Owners of a logical grip, including both sides of a shared junction.
    pub fn control_point_owners(&self, index: usize) -> Result<Vec<(usize, usize)>, GeometryError> {
        let (points, mappings) = self.grip_layout()?;
        if index >= points.len() {
            return Err(GeometryError::InvalidControlPointIndex {
                index,
                count: points.len(),
            });
        }
        Ok(mappings
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.iter()
                    .enumerate()
                    .filter_map(move |(j, k)| (*k == index).then_some((i, j)))
            })
            .collect())
    }

    /// Remove a logical grip. At a junction with an end two-control leaf,
    /// collapse that leaf and move its neighbor to the remaining endpoint.
    /// The neighbor's native endpoint-editing policy preserves its other end.
    pub fn try_remove_control_point(&self, index: usize) -> Result<Self, GeometryError> {
        let owners = self.control_point_owners(index)?;
        if owners.len() == 1 {
            return self.try_remove_segment_control_point(owners[0].0, owners[0].1);
        }
        for &(owner, local) in &owners {
            let leaf = &self.segments[owner];
            if leaf.to_nurbs()?.control_points().len() != 2 {
                continue;
            }
            // Validate seam/interior restrictions before editing a neighbor.
            let mut result = self.try_remove_segment_control_point(owner, local)?;
            if owner == 0 {
                let point = leaf.evaluate(*leaf.domain().start())?;
                result.segments[0] = result.segments[0].try_with_endpoints(Some(point), None)?;
            } else {
                let point = leaf.evaluate(*leaf.domain().end())?;
                let last = result.segments.len() - 1;
                result.segments[last] =
                    result.segments[last].try_with_endpoints(None, Some(point))?;
            }
            return Self::try_with_segment_domains(result.segments, result.parameters);
        }
        Err(invalid(
            "removing a shared composite junction requires a joint edit",
        ))
    }

    /// Remove one leaf control while retaining the parent chart. Deleting an end
    /// segment keeps the remaining parent intervals and leaf representations.
    /// Junction edits that would disconnect the composite are rejected.
    pub fn try_remove_segment_control_point(
        &self,
        owner: usize,
        local: usize,
    ) -> Result<Self, GeometryError> {
        let curve = self.segment(owner)?.to_nurbs()?;
        if local >= curve.control_points().len() {
            return Err(GeometryError::InvalidControlPointIndex {
                index: local,
                count: curve.control_points().len(),
            });
        }
        if self.is_closed()?
            && (owner == 0 && local == 0
                || owner + 1 == self.segments.len() && local + 1 == curve.control_points().len())
        {
            return Err(invalid(
                "removing a closed composite seam grip requires a joint edit",
            ));
        }
        let mut segments = self.segments.clone();
        let mut parameters = self.parameters.clone();
        if curve.control_points().len() == 2 {
            if segments.len() == 1 {
                return Err(GeometryError::InsufficientControlPoints {
                    degree: 1,
                    required: 2,
                    actual: 1,
                });
            }
            if owner > 0 && owner + 1 < segments.len() {
                return Err(invalid(
                    "removing an interior two-control segment would disconnect the composite",
                ));
            }
            segments.remove(owner);
            parameters.remove(if owner == 0 { 0 } else { owner + 1 });
        } else {
            if local == 0 && owner > 0
                || local + 1 == curve.control_points().len() && owner + 1 < segments.len()
            {
                return Err(invalid(
                    "removing a shared composite junction requires a joint edit",
                ));
            }
            let edited = curve
                .try_remove_control_point_preserving_weights(local)?
                .try_reparameterized(curve.domain())?;
            segments[owner] = CurveSegment3::NurbsCurve(edited);
        }
        Self::try_with_segment_domains(segments, parameters)
    }

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
