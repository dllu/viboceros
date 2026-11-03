//! Smooth planar trim loops, analytic offsets and shared NURBS fitting.
use super::*;
use crate::{
    Curve3, CurveOffsetCornerStyle, CurveSegment3, ParameterSide, PolyCurve3, WeightedPoint3,
};

pub(super) fn joined(brep: &Brep, trims: &[BrepTrim]) -> Result<NurbsCurve, GeometryError> {
    let segments = trims
        .iter()
        .map(|trim| {
            let edge = trim
                .edge
                .ok_or_else(|| invalid("inset does not support poles"))?;
            if !matches!(trim.trim_type, BrepTrimType::Boundary | BrepTrimType::Mated) {
                return Err(invalid("inset does not support seam trims"));
            }
            let curve = if trim.reversed_3d {
                brep.edges[edge].curve.reversed()?
            } else {
                brep.edges[edge].curve.clone()
            };
            Ok(CurveSegment3::NurbsCurve(curve))
        })
        .collect::<Result<Vec<_>, GeometryError>>()?;
    PolyCurve3::try_new(segments)?.to_nurbs()
}

fn gap(a: UnitVector3, b: UnitVector3) -> Result<Real, GeometryError> {
    Ok(a.as_vector()
        .cross(b.as_vector())?
        .length()?
        .atan2(a.as_vector().dot(b.as_vector())?))
}

fn shifted_start(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
) -> Result<Point3, GeometryError> {
    let t = *curve.domain().start();
    let tangent = curve.tangent_at_on_side(t, ParameterSide::Right)?;
    curve.evaluate(t)?.translated(
        normal
            .as_vector()
            .cross(tangent.as_vector())?
            .scaled(distance)?,
    )
}

fn analytic_offset(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
) -> Result<Option<NurbsCurve>, GeometryError> {
    let tight = Tolerance::try_new(
        tolerance.absolute() * 0.01,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    let transform = if matches!(curve.analytic_form(tight)?, Some(CurveSegment3::Line(_))) {
        let t = *curve.domain().start();
        let shift = normal
            .as_vector()
            .cross(
                curve
                    .tangent_at_on_side(t, ParameterSide::Right)?
                    .as_vector(),
            )?
            .scaled(distance)?;
        crate::AffineTransform3::from_translation(shift)
    } else if let Some(center) = curve.circular_center(tight)? {
        let p = curve.evaluate(*curve.domain().start())?;
        let radius = center.distance_to(p)?;
        let target = shifted_start(curve, normal, distance)?;
        let left = p.vector_to(target)?;
        let radial = center.vector_to(p)?.normalized_nonzero()?.as_vector();
        let new_radius = radius + left.dot(radial)?;
        if new_radius <= tolerance.absolute() {
            return Err(invalid("inset collapses a circular boundary"));
        }
        crate::AffineTransform3::try_uniform_scale(center, new_radius / radius)?
    } else {
        return Ok(None);
    };
    let candidate = curve.transformed(transform)?;
    // Recognition bounds positions. Match the shared offset fitter's normal-
    // locus sampling policy too, before accepting an analytic shortcut.
    for (a, b) in curve.spans() {
        for i in 0..=16 {
            let t = a * (1. - i as Real / 16.) + b * (i as Real / 16.);
            let side = if i == 16 {
                ParameterSide::Left
            } else {
                ParameterSide::Right
            };
            let expected = curve.evaluate(t)?.translated(
                normal
                    .as_vector()
                    .cross(curve.tangent_at_on_side(t, side)?.as_vector())?
                    .scaled(distance)?,
            )?;
            if candidate.evaluate(t)?.distance_to(expected)? > tolerance.absolute() * 0.25 {
                return Ok(None);
            }
        }
    }
    Ok(Some(candidate))
}

fn offset(
    curve: &NurbsCurve,
    normal: UnitVector3,
    distance: Real,
    tolerance: Tolerance,
    operations: Option<CurveOffsetOperations>,
) -> Result<NurbsCurve, GeometryError> {
    curve.tight_bounds(tolerance)?;
    if let Some(result) = analytic_offset(curve, normal, distance, tolerance)? {
        return Ok(result);
    }
    let fit = Tolerance::try_new(
        tolerance.absolute() * 0.25,
        tolerance.relative(),
        tolerance.angular(),
    )?;
    Curve3::NurbsCurve(curve.clone())
        .try_offset_with_operations(
            distance,
            normal,
            fit,
            CurveOffsetCornerStyle::Sharp,
            operations,
        )?
        .as_ref()
        .to_nurbs()
}

fn aligned(
    curve: NurbsCurve,
    a: Point3,
    b: Point3,
    tolerance: Tolerance,
) -> Result<NurbsCurve, GeometryError> {
    let curve = curve.clamped_to_active_domain()?;
    if curve.evaluate(*curve.domain().start())?.distance_to(a)? > tolerance.absolute() * 0.5
        || curve.evaluate(*curve.domain().end())?.distance_to(b)? > tolerance.absolute() * 0.5
    {
        return Err(invalid(
            "smooth inset endpoint correspondence is unresolved",
        ));
    }
    let mut controls = curve.control_points().to_vec();
    let last = controls.len() - 1;
    controls[0] = WeightedPoint3::try_new(a, controls[0].weight())?;
    controls[last] = WeightedPoint3::try_new(b, controls[last].weight())?;
    NurbsCurve::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
}

impl Brep {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn inset_smooth_boundary(
        &mut self,
        face: &BrepFace,
        boundary: &BrepLoop,
        normal: UnitVector3,
        distance: Real,
        crease: bool,
        tolerance: Tolerance,
        operations: Option<CurveOffsetOperations>,
        work: &mut Work,
    ) -> Result<Boundary, GeometryError> {
        let old = &boundary.trims;
        let mut sources = Vec::with_capacity(old.len());
        for trim in old {
            sources.push(joined(self, std::slice::from_ref(trim))?);
        }
        if sources.is_empty() {
            return Err(invalid("inset requires closed trim loops"));
        }
        for (j, curve) in sources.iter().enumerate() {
            let next = &sources[(j + 1) % sources.len()];
            if curve.has_full_multiplicity_kink(tolerance)?
                || gap(
                    curve.tangent_at_on_side(*curve.domain().end(), ParameterSide::Left)?,
                    next.tangent_at_on_side(*next.domain().start(), ParameterSide::Right)?,
                )? > tolerance.angular()
            {
                return Err(invalid(
                    "curved inset currently requires smooth boundary joins",
                ));
            }
        }
        let goals = sources
            .iter()
            .map(|c| shifted_start(c, normal, distance))
            .collect::<Result<Vec<_>, _>>()?;
        let mut offsets = Vec::with_capacity(sources.len());
        for (j, source) in sources.iter().enumerate() {
            let curve = aligned(
                offset(source, normal, distance, tolerance, operations)?,
                goals[j],
                goals[(j + 1) % goals.len()],
                tolerance,
            )?;
            work.charge_curve(&curve)?;
            offsets.push(curve);
        }
        let strips = crease && old.len() > 1;
        if !strips && offsets.len() > 1 {
            let joined = PolyCurve3::try_new(
                offsets
                    .into_iter()
                    .map(CurveSegment3::NurbsCurve)
                    .collect::<Vec<_>>(),
            )?
            .to_nurbs()?;
            work.charge_curve(&joined)?;
            offsets = vec![joined];
        }
        let vertex_start = self.vertices.len();
        for curve in &offsets {
            self.vertices.push(BrepVertex::try_new(
                curve.evaluate(*curve.domain().start())?,
                0.,
            )?);
        }
        let mut inset = Vec::with_capacity(offsets.len());
        let count = offsets.len();
        for (j, curve) in offsets.into_iter().enumerate() {
            let uv = face.surface.try_pullback_affine_curve(&curve, tolerance)?;
            let vertices = [vertex_start + j, vertex_start + (j + 1) % count];
            let edge = self.edges.len();
            self.edges.push(BrepEdge::try_new(vertices, curve, 0.)?);
            let iso = trim_iso::classify(&uv, &face.surface);
            inset.push(BrepTrim::try_new(
                vertices,
                Some(edge),
                false,
                uv,
                BrepTrimType::Mated,
                iso,
                [0.; 2],
            )?);
        }
        let mut borders = Vec::new();
        if strips {
            let mut connectors = Vec::with_capacity(old.len());
            for (j, trim) in old.iter().enumerate() {
                connectors.push(self.inset_line_trim(
                    &face.surface,
                    [trim.vertices[0], vertex_start + j],
                    [self.vertices[trim.vertices[0]].point, goals[j]],
                    tolerance,
                )?);
            }
            for j in 0..old.len() {
                let k = (j + 1) % old.len();
                let trims = vec![
                    old[j].clone(),
                    connectors[k].clone(),
                    reverse_trim(&inset[j])?,
                    reverse_trim(&connectors[j])?,
                ];
                borders.push(BrepFace::try_new(
                    face.surface.clone(),
                    face.reversed,
                    vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
                )?);
            }
        } else {
            let reversed = inset
                .iter()
                .rev()
                .map(reverse_trim)
                .collect::<Result<Vec<_>, _>>()?;
            let (outer, inner) = if boundary.loop_type == BrepLoopType::Outer {
                (old.clone(), reversed)
            } else {
                (reversed, old.clone())
            };
            borders.push(BrepFace::try_new(
                face.surface.clone(),
                face.reversed,
                vec![
                    BrepLoop::try_new(BrepLoopType::Outer, outer)?,
                    BrepLoop::try_new(BrepLoopType::Inner, inner)?,
                ],
            )?);
        }
        Ok(Boundary { inset, borders })
    }
}
