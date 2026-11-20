//! Straight swept faces with rectangular or exact rational miter trims.
use super::*;
use crate::{
    BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, NurbsCurve2,
    Point2, SurfaceIso, WeightedPoint2,
};

pub(super) struct Panel {
    pub brep: Brep,
    pub start: usize,
    pub end: usize,
    pub south: usize,
    pub north: usize,
}

pub(super) fn build(
    profile: &NurbsCurve,
    tangent: Vector3,
    origins: [Point3; 2],
    normals: [Option<UnitVector3>; 2],
    length: Real,
    untrimmed: bool,
    tolerance: Tolerance,
) -> Result<Panel, GeometryError> {
    let bounds = [0., length];
    let offsets = [
        offsets(profile, tangent, origins[0], normals[0], 0.)?,
        offsets(profile, tangent, origins[1], normals[1], length)?,
    ];
    // With common positive weights this coefficient check certifies that
    // the two trim graphs leave a nonempty strip at every profile parameter.
    if offsets[0]
        .iter()
        .zip(&offsets[1])
        .any(|(a, b)| b - a <= tolerance.absolute())
    {
        return Err(invalid("sweep miter boundaries overlap on a rail segment"));
    }
    let profiles = [
        shifted(profile, tangent, offsets[0].iter().copied())?,
        shifted(profile, tangent, offsets[1].iter().copied())?,
    ];
    let v = profile.domain();
    let v_bounds = [*v.start(), *v.end()];
    let (surface, trims, iso) = if untrimmed {
        (
            surface_between(&profiles, bounds)?,
            [iso_trim(0., v_bounds)?, iso_trim(length, v_bounds)?],
            [SurfaceIso::West, SurfaceIso::East],
        )
    } else {
        let limits = [
            offsets[0].iter().copied().fold(0., Real::min),
            offsets[1].iter().copied().fold(length, Real::max),
        ];
        let support = limits.map(|extent| {
            shifted(
                profile,
                tangent,
                std::iter::repeat_n(extent, profile.control_points().len()),
            )
        });
        let [low, high] = support;
        let [start_iso, end_iso] = [SurfaceIso::West, SurfaceIso::East];
        let start = if normals[0].is_some() {
            SurfaceIso::NotIso
        } else {
            start_iso
        };
        let end = if normals[1].is_some() {
            SurfaceIso::NotIso
        } else {
            end_iso
        };
        let trims = [
            if normals[0].is_some() {
                trim_graph(profile, &offsets[0])?
            } else {
                iso_trim(0., v_bounds)?
            },
            if normals[1].is_some() {
                trim_graph(profile, &offsets[1])?
            } else {
                iso_trim(length, v_bounds)?
            },
        ];
        (
            surface_between(&[low?, high?], limits)?,
            trims,
            [start, end],
        )
    };
    face(surface, profiles, trims, iso, tolerance)
}

fn offsets(
    profile: &NurbsCurve,
    tangent: Vector3,
    origin: Point3,
    normal: Option<UnitVector3>,
    fallback: Real,
) -> Result<Vec<Real>, GeometryError> {
    let Some(normal) = normal else {
        return Ok(vec![fallback; profile.control_points().len()]);
    };
    let normal = normal.as_vector();
    let divisor = tangent.dot(normal)?;
    if divisor <= 64. * Real::EPSILON {
        return Err(invalid("a reversing rail has no miter plane"));
    }
    profile
        .control_points()
        .iter()
        .map(|point| Ok(point.point().vector_to(origin)?.dot(normal)? / divisor))
        .collect()
}

fn shifted(
    profile: &NurbsCurve,
    tangent: Vector3,
    offsets: impl Iterator<Item = Real>,
) -> Result<NurbsCurve, GeometryError> {
    let controls = profile
        .control_points()
        .iter()
        .zip(offsets)
        .map(|(point, distance)| {
            WeightedPoint3::try_new(
                point.point().translated(tangent.scaled(distance)?)?,
                point.weight(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    NurbsCurve::try_new_rational(profile.degree(), controls, profile.knots().to_vec())
}

fn surface_between(
    profiles: &[NurbsCurve; 2],
    u: [Real; 2],
) -> Result<NurbsSurface, GeometryError> {
    let [start, end] = profiles;
    let mut controls = Vec::with_capacity(4 * start.control_points().len());
    for (a, b) in start.control_points().iter().zip(end.control_points()) {
        controls.push(*a);
        for fraction in [1. / 3., 2. / 3.] {
            let p = a
                .point()
                .translated(a.point().vector_to(b.point())?.scaled(fraction)?)?;
            controls.push(WeightedPoint3::try_new(p, a.weight())?);
        }
        controls.push(*b);
    }
    NurbsSurface::try_new_rational(
        3,
        start.degree(),
        4,
        start.control_points().len(),
        controls,
        u.into_iter().flat_map(|t| [t; 4]).collect(),
        start.knots().to_vec(),
    )
}

fn iso_trim(u: Real, v: [Real; 2]) -> Result<NurbsCurve2, GeometryError> {
    NurbsCurve2::try_line(Point2::try_new(u, v[0])?, Point2::try_new(u, v[1])?)
}

/// Lift the rational scalar offset a(v) to the trim (a(v), v). On each
/// Bézier span, elevate the first numerator and the denominator once and
/// multiply the original denominator by the linear parameter for the second.
fn trim_graph(profile: &NurbsCurve, offsets: &[Real]) -> Result<NurbsCurve2, GeometryError> {
    let controls = profile
        .control_points()
        .iter()
        .zip(offsets)
        .map(|(cp, &u)| WeightedPoint3::try_new(Point3::try_new(u, 0., 0.)?, cp.weight()))
        .collect::<Result<Vec<_>, _>>()?;
    let scalar =
        NurbsCurve::try_new_rational(profile.degree(), controls, profile.knots().to_vec())?;
    let degree = profile.degree() + 1;
    let mut points = Vec::new();
    let mut knots = vec![*scalar.domain().start(); degree + 1];
    for (index, span) in scalar.try_bezier_spans()?.iter().enumerate() {
        let domain = span.domain();
        if index > 0 {
            knots.extend(std::iter::repeat_n(*domain.start(), degree));
        }
        let controls = span.control_points();
        for k in usize::from(index > 0)..=degree {
            let a = controls[k.min(degree - 1)];
            let b = controls[k.saturating_sub(1)];
            let fraction = k as Real / degree as Real;
            let wa = (1. - fraction) * a.weight();
            let wb = fraction * b.weight();
            let weight = wa + wb;
            let point = Point2::try_new(
                (wa * a.point().x() + wb * b.point().x()) / weight,
                (wa * domain.start() + wb * domain.end()) / weight,
            )?;
            points.push(WeightedPoint2::try_new(point, weight)?);
        }
    }
    knots.extend(std::iter::repeat_n(*scalar.domain().end(), degree + 1));
    NurbsCurve2::try_new_rational(degree, points, knots)
}

fn face(
    surface: NurbsSurface,
    [start, end]: [NurbsCurve; 2],
    [west, east]: [NurbsCurve2; 2],
    iso: [SurfaceIso; 2],
    tolerance: Tolerance,
) -> Result<Panel, GeometryError> {
    let closed = start.is_closed()?;
    let v = surface.domain_v();
    let [[a, b], [d, c]] =
        [&start, &end].map(|curve| [curve.evaluate(*v.start()), curve.evaluate(*v.end())]);
    let [a, d, c, b] = [a?, d?, c?, b?];
    let mut vertices = vec![BrepVertex::try_new(a, 0.)?, BrepVertex::try_new(d, 0.)?];
    if !closed {
        vertices.extend([BrepVertex::try_new(c, 0.)?, BrepVertex::try_new(b, 0.)?]);
    }
    let [a_index, d_index, c_index, b_index] = if closed { [0, 1, 1, 0] } else { [0, 1, 2, 3] };
    let mut edges = vec![
        BrepEdge::try_new(
            [a_index, d_index],
            crate::LineSegment::try_new(a, d, tolerance)?.to_nurbs()?,
            0.,
        )?,
        BrepEdge::try_new([d_index, c_index], end, 0.)?,
        BrepEdge::try_new([a_index, b_index], start, 0.)?,
    ];
    let north = if closed {
        0
    } else {
        edges.push(BrepEdge::try_new(
            [b_index, c_index],
            crate::LineSegment::try_new(b, c, tolerance)?.to_nurbs()?,
            0.,
        )?);
        3
    };
    let w = west.domain();
    let e = east.domain();
    let uv = [
        west.evaluate(*w.start())?,
        east.evaluate(*e.start())?,
        east.evaluate(*e.end())?,
        west.evaluate(*w.end())?,
    ];
    let side = if closed {
        BrepTrimType::Seam
    } else {
        BrepTrimType::Boundary
    };
    let trims = vec![
        BrepTrim::try_new(
            [a_index, d_index],
            Some(0),
            false,
            NurbsCurve2::try_line(uv[0], uv[1])?,
            side,
            SurfaceIso::South,
            [0.; 2],
        )?,
        BrepTrim::try_new(
            [d_index, c_index],
            Some(1),
            false,
            east,
            BrepTrimType::Boundary,
            iso[1],
            [0.; 2],
        )?,
        BrepTrim::try_new(
            [c_index, b_index],
            Some(north),
            true,
            NurbsCurve2::try_line(uv[2], uv[3])?,
            side,
            SurfaceIso::North,
            [0.; 2],
        )?,
        BrepTrim::try_new(
            [b_index, a_index],
            Some(2),
            true,
            west.reversed()?,
            BrepTrimType::Boundary,
            iso[0],
            [0.; 2],
        )?,
    ];
    let face = BrepFace::try_new(
        surface,
        false,
        vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
    )?;
    let brep = Brep::try_new(vertices, edges, vec![face], tolerance)?;
    Ok(Panel {
        brep,
        start: 2,
        end: 1,
        south: 0,
        north,
    })
}
