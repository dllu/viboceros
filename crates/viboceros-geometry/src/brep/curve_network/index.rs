use super::*;
use rstar::{
    AABB, RTree,
    primitives::{GeomWithData, Rectangle},
};

// Keep R-tree volume arithmetic finite. Extreme coordinates and rational
// curves without a convex control hull use a conservative exhaustive fallback.
const INDEX_COORDINATE_LIMIT: Real = 1.0e90;

fn envelope<const N: usize>(low: [Real; N], high: [Real; N]) -> Option<AABB<[Real; N]>> {
    low.into_iter()
        .chain(high)
        .all(|x| x.is_finite() && x.abs() < INDEX_COORDINATE_LIMIT)
        .then(|| AABB::from_corners(low, high))
}

#[derive(Default)]
pub(super) struct HullIndex {
    tree: RTree<GeomWithData<Rectangle<[Real; 3]>, usize>>,
    fallback: Vec<usize>,
    count: usize,
    scale: Real,
}

impl HullIndex {
    pub fn hull(curve: &NurbsCurve) -> Option<BoundingBox3> {
        let sign = curve.control_points()[0].weight().is_sign_positive();
        curve
            .control_points()
            .iter()
            .all(|p| p.weight().is_sign_positive() == sign)
            .then(|| curve.control_point_bounds())
    }

    pub fn insert(&mut self, hull: Option<BoundingBox3>) {
        let bounds = hull.and_then(|b| envelope(b.min().to_array(), b.max().to_array()));
        if let Some(bounds) = bounds {
            self.scale = bounds
                .lower()
                .into_iter()
                .chain(bounds.upper())
                .fold(self.scale.max(1.), |scale, x| scale.max(x.abs()));
            self.tree.insert(GeomWithData::new(
                Rectangle::from_corners(bounds.lower(), bounds.upper()),
                self.count,
            ));
        } else {
            self.fallback.push(self.count);
        }
        self.count += 1;
    }

    pub fn candidates(&self, hull: Option<BoundingBox3>, tolerance: Tolerance) -> Vec<usize> {
        let bounds = hull.and_then(|b| {
            // Outward roundoff padding keeps the search conservative at a
            // tolerance boundary; the intersection solver makes the decision.
            let scale = b
                .min()
                .to_array()
                .into_iter()
                .chain(b.max().to_array())
                .fold(self.scale.max(1.), |scale, x| scale.max(x.abs()));
            let allowed = tolerance.absolute().max(tolerance.relative() * scale);
            let padding = |x: Real| allowed + x.abs().max(1.) * Real::EPSILON * 8.;
            envelope(
                b.min().to_array().map(|x| x - padding(x)),
                b.max().to_array().map(|x| x + padding(x)),
            )
        });
        let Some(bounds) = bounds else {
            return (0..self.count).collect();
        };
        let mut result = self
            .tree
            .locate_in_envelope_intersecting(bounds)
            .map(|entry| entry.data)
            .chain(self.fallback.iter().copied())
            .collect::<Vec<_>>();
        // Preserve source order, including topology tie-breaks at coincidences.
        result.sort_unstable();
        result
    }
}

#[derive(Default)]
pub(super) struct NodeIndex {
    tree: RTree<GeomWithData<[Real; 2], usize>>,
    fallback: Vec<usize>,
}

impl NodeIndex {
    pub fn new(nodes: &[SurfaceCutArrangementNode]) -> Self {
        let mut result = Self::default();
        for (index, node) in nodes.iter().enumerate() {
            result.insert(node.parameter, index);
        }
        result
    }

    fn insert(&mut self, point: Point2, index: usize) {
        let p = point.to_array();
        if envelope(p, p).is_some() {
            self.tree.insert(GeomWithData::new(p, index));
        } else {
            self.fallback.push(index);
        }
    }

    pub fn node(
        &mut self,
        nodes: &mut Vec<SurfaceCutArrangementNode>,
        parameter: Point2,
        point: Point3,
        tolerance: [Real; 2],
        maximum: usize,
    ) -> Result<usize, GeometryError> {
        let p = parameter.to_array();
        let padding = std::array::from_fn::<_, 2, _>(|i| {
            tolerance[i] + p[i].abs().max(1.) * Real::EPSILON * 8.
        });
        let bounds = envelope(
            std::array::from_fn(|i| p[i] - padding[i]),
            std::array::from_fn(|i| p[i] + padding[i]),
        );
        let near = |i: &usize| parameter_points_near(nodes[*i].parameter, parameter, tolerance);
        let existing = if let Some(bounds) = bounds {
            self.tree
                .locate_in_envelope_intersecting(bounds)
                .map(|entry| entry.data)
                .chain(self.fallback.iter().copied())
                .filter(near)
                .min()
        } else {
            (0..nodes.len()).find(near)
        };
        if let Some(index) = existing {
            return Ok(index);
        }
        if nodes.len() >= maximum {
            return invalid("a surface cutting arrangement exceeds its work limit");
        }
        let index = nodes.len();
        nodes.push(SurfaceCutArrangementNode { parameter, point });
        self.insert(parameter, index);
        Ok(index)
    }
}
