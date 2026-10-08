//! Exact collapsed sides retain vertices and UV branches without spatial edges.
use super::*;

pub(super) struct Pole {
    pub point: Point3,
    pub parameter: Point2,
    axis: usize,
    coordinate: Real,
}

impl Pole {
    pub fn endpoint(&self, reference: [Real; 2]) -> Result<Point2, GeometryError> {
        let mut uv = reference;
        uv[self.axis] = self.coordinate;
        Point2::try_new(uv[0], uv[1])
    }
}

pub(super) fn prepare(
    face: &BrepFace,
    target: &NurbsSurface,
) -> Result<BTreeMap<usize, Pole>, GeometryError> {
    let mut poles = BTreeMap::<usize, Pole>::new();
    let domains = [face.surface.domain_u(), face.surface.domain_v()];
    for trim in face.loops.iter().flat_map(|l| &l.trims) {
        if trim.trim_type != BrepTrimType::Singular {
            continue;
        }
        if trim.edge.is_some() || trim.vertices[0] != trim.vertices[1] {
            return invalid("a singular retrim must retain one vertex without an edge");
        }
        let iso = trim_iso::classify(&trim.curve, &face.surface);
        let (axis, coordinate) = match iso {
            SurfaceIso::West => (0, 0.),
            SurfaceIso::East => (0, 1.),
            SurfaceIso::South => (1, 0.),
            SurfaceIso::North => (1, 1.),
            _ => return invalid("singular retrimming requires a natural collapsed side"),
        };
        let sign = trim.curve.control_points()[0].weight().is_sign_positive();
        // A coherent rational control hull proves the complete UV trim remains
        // on this side and inside the original free-coordinate interval.
        if trim.curve.control_points().iter().any(|p| {
            p.weight().is_sign_positive() != sign
                || !domains[0].contains(&p.point().x())
                || !domains[1].contains(&p.point().y())
        }) {
            return invalid("singular retrim must remain in its natural parameter chart");
        }
        let boundary = if axis == 0 {
            target.isocurve_v(coordinate)?
        } else {
            target.isocurve_u(coordinate)?
        };
        let point = boundary.control_points()[0].point();
        if boundary.control_points().iter().any(|p| p.point() != point) {
            return invalid("the target retrim side is not exactly collapsed");
        }
        let reference = trim.curve.start_point()?.to_array();
        let [u, v]: [Result<Real, GeometryError>; 2] = std::array::from_fn(|i| {
            crate::remap_scalar(
                reference[i],
                [*domains[i].start(), *domains[i].end()],
                [0., 1.],
            )
        });
        let parameter = Point2::try_new(u?, v?)?;
        if let Some(previous) = poles.get(&trim.vertices[0]) {
            if previous.point != point || previous.axis != axis || previous.coordinate != coordinate
            {
                return invalid("singular retrim vertex has conflicting collapsed sides");
            }
        } else {
            poles.insert(
                trim.vertices[0],
                Pole {
                    point,
                    parameter,
                    axis,
                    coordinate,
                },
            );
        }
    }
    Ok(poles)
}
