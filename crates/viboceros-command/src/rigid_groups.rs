//! Shared placement origins for selected rigid group members.
use crate::CommandError;
use std::collections::{BTreeMap, BTreeSet};
use viboceros_document::{Document, GroupId, ObjectId};
use viboceros_geometry::{BoundingBox3, FiniteSum, Point3, Tolerance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupFrameOrigin {
    #[default]
    BoundingBoxCenter,
    ObjectFrameAverage,
}
impl GroupFrameOrigin {
    pub const fn name(self) -> &'static str {
        match self {
            Self::BoundingBoxCenter => "BoundingBoxCenter",
            Self::ObjectFrameAverage => "ObjectFrameAverage",
        }
    }
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "boundingboxcenter" => Some(Self::BoundingBoxCenter),
            "objectframeaverage" => Some(Self::ObjectFrameAverage),
            _ => None,
        }
    }
    pub const fn toggled(self) -> Self {
        match self {
            Self::BoundingBoxCenter => Self::ObjectFrameAverage,
            Self::ObjectFrameAverage => Self::BoundingBoxCenter,
        }
    }
}
/// Only selected members contribute. Duplicate parents/grip owners count once.
/// ObjectFrameAverage uses the individual tight-box centers, independently of
/// an object's stored custom frame. Public native command witnesses distinguish
/// the average of member centers from the center of their union.
pub fn group_origins(
    doc: &Document,
    context: impl IntoIterator<Item = ObjectId>,
    mode: GroupFrameOrigin,
    tolerance: Tolerance,
) -> Result<BTreeMap<GroupId, Point3>, CommandError> {
    let mut bounds = BTreeMap::<GroupId, BoundingBox3>::new();
    let mut means = BTreeMap::<GroupId, [FiniteSum; 3]>::new();
    for id in context.into_iter().collect::<BTreeSet<_>>() {
        let object = doc
            .object(id)
            .ok_or(viboceros_document::DocumentError::ObjectNotFound(id))?;
        let Some(group) = object.top_group() else {
            continue;
        };
        match mode {
            GroupFrameOrigin::BoundingBoxCenter => {
                let b = object.geometry().tight_bounds(tolerance)?;
                bounds.insert(
                    group,
                    match bounds.get(&group) {
                        Some(prior) => prior.union(b)?,
                        None => b,
                    },
                );
            }
            GroupFrameOrigin::ObjectFrameAverage => {
                let p = object
                    .geometry()
                    .tight_bounds(tolerance)?
                    .center()?
                    .to_array();
                let sums = means.entry(group).or_default();
                for (axis, sum) in sums.iter_mut().enumerate() {
                    sum.add(p[axis])?;
                }
            }
        }
    }
    if mode == GroupFrameOrigin::BoundingBoxCenter {
        bounds
            .into_iter()
            .map(|(g, b)| Ok((g, b.center()?)))
            .collect()
    } else {
        means
            .into_iter()
            .map(|(g, s)| {
                Ok((
                    g,
                    Point3::try_new(s[0].mean()?, s[1].mean()?, s[2].mean()?)?,
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::Geometry;
    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    #[test]
    fn group_origins_use_member_boxes_and_deduplicate_selected_context() {
        let mut doc = Document::default();
        let line = viboceros_geometry::LineSegment::try_new(
            p(0., 0., 0.),
            p(10., 0., 0.),
            doc.tolerance(),
        )
        .unwrap();
        let ids = [
            Geometry::Line(line),
            Geometry::Point(p(20., 10., 2.)),
            Geometry::Point(p(1e6, 1e6, 1e6)),
        ]
        .map(|g| doc.add_geometry(g).unwrap());
        let group = doc.add_group(None, ids).unwrap();
        let context = [ids[0], ids[1], ids[0]];
        let average = group_origins(
            &doc,
            context,
            GroupFrameOrigin::ObjectFrameAverage,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert_eq!(average[&group], p(12.5, 5., 1.));
        let bounds = group_origins(
            &doc,
            context,
            GroupFrameOrigin::BoundingBoxCenter,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert_eq!(bounds[&group], p(10., 5., 1.));
        let single = group_origins(
            &doc,
            [ids[0]],
            GroupFrameOrigin::ObjectFrameAverage,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert_eq!(single[&group], p(5., 0., 0.));
    }
    #[test]
    fn group_average_is_unweighted_order_independent_and_overflow_safe() {
        let mut doc = Document::default();
        let tiny = f64::from_bits(1);
        let ids = [
            p(f64::MAX, tiny, 0.),
            p(f64::MAX, tiny, 0.),
            p(-f64::MAX, 0., 0.),
        ]
        .map(|p| doc.add_geometry(Geometry::Point(p)).unwrap());
        let group = doc.add_group(None, ids).unwrap();
        let a = group_origins(
            &doc,
            ids,
            GroupFrameOrigin::ObjectFrameAverage,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        let b = group_origins(
            &doc,
            ids.into_iter().rev(),
            GroupFrameOrigin::ObjectFrameAverage,
            Tolerance::NUMERICAL_VALIDATION,
        )
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a[&group], p(f64::MAX / 3., tiny, 0.));
    }
}
