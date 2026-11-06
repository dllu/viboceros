//! Physical corner charts and exact triangular trim construction.
use crate::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex,
    GeometryError, NurbsCurve, NurbsCurve2, NurbsSurface, Point2, Point3, SurfaceIso, Tolerance,
};
#[path = "../../../third_party/opennurbs_rust/length.rs"]
mod native_length;
fn distance(a: Point3, b: Point3) -> Result<f64, GeometryError> {
    let d = a.vector_to(b)?;
    let length = native_length::length3d(d.to_array());
    if length.is_finite() {
        Ok(length)
    } else {
        Err(GeometryError::NonFinite {
            context: "corner surface length",
        })
    }
}

impl NurbsSurface {
    /// Bilinear perimeter corners with each parameter extent equal to the
    /// longest of its two opposite edges. Singular sides remain explicit.
    pub fn try_corner_patch(p: [Point3; 4]) -> Result<Self, GeometryError> {
        if (0..4).filter(|&i| !p[..i].contains(&p[i])).count() < 3 {
            return Err(GeometryError::Degenerate {
                context: "corner surface requires three distinct corners",
            });
        }
        let u = distance(p[0], p[1])?.max(distance(p[3], p[2])?);
        let v = distance(p[0], p[3])?.max(distance(p[1], p[2])?);
        Self::try_bilinear(p)?.try_reparameterized(0. ..=u, 0. ..=v)
    }
}
impl Brep {
    /// An exact three-edge triangle on its affine parallelogram surface.
    pub fn try_three_corner_face(
        p: [Point3; 3],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        const ZERO: f64 = 2.3283064365386963e-10;
        for i in 0..3 {
            for j in i + 1..3 {
                if p[i]
                    .to_array()
                    .into_iter()
                    .zip(p[j].to_array())
                    .all(|(a, b)| (a - b).abs() <= ZERO)
                {
                    return Err(GeometryError::Degenerate {
                        context: "triangular surface corners coincide",
                    });
                }
            }
        }
        let [a, b, c] = p.map(Point3::to_array);
        let opposite = Point3::try_from(std::array::from_fn(|i| c[i] + (b[i] - a[i])))?;
        let surface = NurbsSurface::try_corner_patch([p[0], p[1], opposite, p[2]])?;
        let u = *surface.domain_u().end();
        let v = *surface.domain_v().end();
        let uv = [
            Point2::try_new(0., 0.)?,
            Point2::try_new(u, 0.)?,
            Point2::try_new(0., v)?,
        ];
        let vertices = p
            .into_iter()
            .map(|p| BrepVertex::try_new(p, 0.))
            .collect::<Result<_, _>>()?;
        let mut edges = Vec::with_capacity(3);
        let mut trims = Vec::with_capacity(3);
        for i in 0..3 {
            let j = (i + 1) % 3;
            let d = distance(p[i], p[j])?;
            let curve = NurbsCurve::try_new(1, vec![p[i], p[j]], vec![0., 0., d, d])?;
            edges.push(BrepEdge::try_new([i, j], curve, 0.)?);
            let d = uv[i].distance_to(uv[j])?;
            let curve = NurbsCurve2::try_new(1, vec![uv[i], uv[j]], vec![0., 0., d, d])?;
            trims.push(BrepTrim::try_new(
                [i, j],
                Some(i),
                false,
                curve,
                BrepTrimType::Boundary,
                [SurfaceIso::South, SurfaceIso::NotIso, SurfaceIso::West][i],
                [0.; 2],
            )?);
        }
        Self::try_new(
            vertices,
            edges,
            vec![BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
            )?],
            tolerance,
        )
    }
}
