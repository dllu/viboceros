//! Prepared endpoint connectivity for curve and surface-edge selection.
use crate::point_neighbors::PointTree;
use crate::{Curve3, GeometryError, ParameterSide, Point3, Real, UnitVector3, Vector3};
use std::collections::BTreeSet;
const MAX_CURVES: usize = 100_000;
const MAX_CHECKS: usize = 16_000_000;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChainContinuity {
    Position,
    #[default]
    Tangency,
    Curvature,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChainDirection {
    Forward,
    Backward,
    #[default]
    Both,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveChainOptions {
    pub continuity: ChainContinuity,
    pub direction: ChainDirection,
    pub gap_tolerance: Real,
    pub angle_tolerance: Real,
}
impl CurveChainOptions {
    pub fn validate(self) -> Result<(), GeometryError> {
        if !self.gap_tolerance.is_finite()
            || self.gap_tolerance < 0.
            || !self.angle_tolerance.is_finite()
            || !(0. ..=std::f64::consts::PI).contains(&self.angle_tolerance)
        {
            return Err(GeometryError::InvalidCurveChainOptions);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainContinuation {
    pub index: usize,
    pub reversed: bool,
    pub turning_angle: Real,
    pub gap: Real,
}
#[derive(Clone, Copy, Debug)]
struct Endpoint {
    curve: usize,
    start: bool,
    point: Point3,
    tangent: Option<UnitVector3>,
    curvature: Option<Vector3>,
}
/// Built once from immutable source geometry. Closed curves have no continuation.
#[derive(Clone, Debug)]
pub struct CurveChainIndex {
    endpoints: Vec<Endpoint>,
    ends: Vec<Option<[usize; 2]>>,
    tree: PointTree,
}
impl CurveChainIndex {
    pub fn new(curves: &[Curve3]) -> Result<Self, GeometryError> {
        if curves.len() > MAX_CURVES {
            return Err(limit("curves", MAX_CURVES));
        }
        let mut endpoints = Vec::new();
        let mut ends = Vec::with_capacity(curves.len());
        for (index, curve) in curves.iter().enumerate() {
            let curve = curve.as_ref();
            if curve.is_closed()? {
                ends.push(None);
                continue;
            }
            ends.push(Some([endpoints.len(), endpoints.len() + 1]));
            for start in [true, false] {
                let t = if start {
                    *curve.domain().start()
                } else {
                    *curve.domain().end()
                };
                let side = if start {
                    ParameterSide::Right
                } else {
                    ParameterSide::Left
                };
                let tangent = curve.evaluate_with_tangent_on_side(t, side).ok().map(|s| {
                    if start {
                        s.tangent().opposite()
                    } else {
                        s.tangent()
                    }
                });
                endpoints.push(Endpoint {
                    curve: index,
                    start,
                    point: curve.evaluate_on_side(t, side)?,
                    tangent,
                    curvature: curve.curvature_vector_on_side(t, side).ok(),
                });
            }
        }
        let tree = PointTree::new(endpoints.iter().map(|e| e.point).collect());
        Ok(Self {
            endpoints,
            ends,
            tree,
        })
    }
    /// Whether two open source ends close a chain within its gap tolerance.
    pub fn ends_meet(
        &self,
        a: (usize, bool),
        b: (usize, bool),
        gap: Real,
    ) -> Result<bool, GeometryError> {
        if !gap.is_finite() || gap < 0. {
            return Err(GeometryError::InvalidCurveChainOptions);
        }
        let endpoint = |(source, at_end): (usize, bool)| -> Result<Option<Point3>, GeometryError> {
            let ends = self
                .ends
                .get(source)
                .ok_or(GeometryError::InvalidCurveChainSeed)?;
            Ok(ends.map(|e| self.endpoints[e[usize::from(at_end)]].point))
        };
        let (Some(a), Some(b)) = (endpoint(a)?, endpoint(b)?) else {
            return Ok(false);
        };
        Ok((a.x() - b.x()).hypot(a.y() - b.y()).hypot(a.z() - b.z()) <= gap)
    }
    /// Compatible, unselected segments incident to the requested outgoing end.
    /// Newer sources precede older ones after source-kind preference.
    pub fn continuations(
        &self,
        source: usize,
        at_end: bool,
        options: CurveChainOptions,
        excluded: &BTreeSet<usize>,
        classes: &[u8],
    ) -> Result<Vec<ChainContinuation>, GeometryError> {
        options.validate()?;
        if source >= self.ends.len() || classes.len() != self.ends.len() {
            return Err(GeometryError::InvalidCurveChainSeed);
        }
        let Some(ends) = self.ends[source] else {
            return Ok(Vec::new());
        };
        let a = self.endpoints[ends[usize::from(at_end)]];
        let mut result = Vec::new();
        let mut checks = 0;
        let mut check = || {
            checks += 1;
            if checks > MAX_CHECKS {
                Err(limit("spatial comparisons", MAX_CHECKS))
            } else {
                Ok(())
            }
        };
        self.tree
            .visit_near(a.point, options.gap_tolerance, &mut check, &mut |i| {
                let b = self.endpoints[i];
                if a.curve == b.curve || excluded.contains(&b.curve) {
                    return Ok(());
                }
                let gap = (a.point.x() - b.point.x())
                    .hypot(a.point.y() - b.point.y())
                    .hypot(a.point.z() - b.point.z());
                if gap > options.gap_tolerance {
                    return Ok(());
                }
                let (angle, cosine) = match (a.tangent, b.tangent) {
                    (Some(a), Some(b)) => (
                        a.as_vector().angle_to(b.opposite().as_vector())?,
                        a.as_vector().dot(b.opposite().as_vector())?,
                    ),
                    _ => (Real::INFINITY, -1.),
                };
                if options.continuity != ChainContinuity::Position
                    && cosine.clamp(-1., 1.).acos() > options.angle_tolerance
                {
                    return Ok(());
                }
                if options.continuity == ChainContinuity::Curvature {
                    let Some((a, b)) = a.curvature.zip(b.curvature) else {
                        return Ok(());
                    };
                    let difference = (a.x() - b.x()).hypot(a.y() - b.y()).hypot(a.z() - b.z());
                    if difference > f64::EPSILON.sqrt() {
                        return Ok(());
                    }
                    if !viboceros_opennurbs::curvature_vectors_g2(
                        a.to_array(),
                        b.to_array(),
                        options.angle_tolerance.cos(),
                        f64::EPSILON.sqrt(),
                    )? {
                        return Ok(());
                    }
                }
                result.push((
                    classes[a.curve] != classes[b.curve],
                    angle,
                    gap,
                    b.curve,
                    !b.start,
                ));
                Ok(())
            })?;
        result.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| b.3.cmp(&a.3))
                .then_with(|| a.1.total_cmp(&b.1))
                .then_with(|| a.2.total_cmp(&b.2))
        });
        Ok(result
            .into_iter()
            .map(
                |(_, turning_angle, gap, index, reversed)| ChainContinuation {
                    index,
                    reversed,
                    turning_angle,
                    gap,
                },
            )
            .collect())
    }
}
fn limit(resource: &'static str, maximum: usize) -> GeometryError {
    GeometryError::CurveChainLimit { resource, maximum }
}
