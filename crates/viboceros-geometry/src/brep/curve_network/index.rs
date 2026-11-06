use super::*;
pub(super) use crate::curve_hull_index::HullIndex;
use crate::curve_hull_index::envelope;
use rstar::{RTree, primitives::GeomWithData};

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

// A complete overlap of simple open curves pairs their endpoint sets. Use
// model-space acceptance, including relative scale, just like the narrow phase.
// Closed curves can have different seams and still duplicate the same loop.
pub(super) fn may_duplicate(
    a: &SurfaceCutArrangementCurve,
    b: &SurfaceCutArrangementCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    if a.closed || b.closed {
        return Ok(true);
    }
    let scale = a
        .spatial
        .control_points()
        .iter()
        .chain(b.spatial.control_points())
        .flat_map(|p| p.point().to_array())
        .fold(1_f64, |s, x| s.max(x.abs()));
    let allowed = tolerance.absolute().max(tolerance.relative() * scale) * 8.;
    let ends = |c: &NurbsCurve| {
        Ok::<_, GeometryError>([
            c.evaluate(*c.domain().start())?,
            c.evaluate(*c.domain().end())?,
        ])
    };
    let [p, q] = ends(&a.spatial)?;
    let [r, s] = ends(&b.spatial)?;
    Ok(
        (p.distance_to(r)? <= allowed && q.distance_to(s)? <= allowed)
            || (p.distance_to(s)? <= allowed && q.distance_to(r)? <= allowed),
    )
}

// Two noncollinear straight segments sharing an exact endpoint have no other
// contact. Their endpoint node is already registered. A dimensional margin
// keeps nearly coincident rays on the general intersection path.
pub(super) fn only_common_endpoint(
    a: &NurbsCurve,
    b: &NurbsCurve,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let linear = |c: &NurbsCurve| {
        c.degree() == 1
            && c.control_points().len() == 2
            && c.control_points()[0].weight().is_sign_positive()
                == c.control_points()[1].weight().is_sign_positive()
    };
    if !linear(a) || !linear(b) {
        return Ok(false);
    }
    let ends = |c: &NurbsCurve| [c.control_points()[0].point(), c.control_points()[1].point()];
    let x = ends(a);
    let y = ends(b);
    for i in 0..2 {
        for j in 0..2 {
            if x[i] == y[j] {
                let u = x[i].vector_to(x[1 - i])?;
                let v = y[j].vector_to(y[1 - j])?;
                let lu = u.length()?;
                let lv = v.length()?;
                let scale = x
                    .into_iter()
                    .chain(y)
                    .flat_map(Point3::to_array)
                    .map(Real::abs)
                    .fold(1., Real::max);
                let allowed = tolerance.absolute().max(tolerance.relative() * scale);
                let threshold = (8. * allowed * lu.max(lv)).max(512. * Real::EPSILON * lu * lv);
                return Ok(u.cross(v)?.length()? > threshold);
            }
        }
    }
    Ok(false)
}
