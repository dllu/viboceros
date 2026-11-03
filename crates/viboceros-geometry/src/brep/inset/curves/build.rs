//! Cap edges and annular borders shared by ordinary and closed-edge offsets.
use super::*;

pub(super) fn cap(
    brep: &mut Brep,
    surface: &NurbsSurface,
    curves: Vec<NurbsCurve>,
    tolerance: Tolerance,
) -> Result<Vec<BrepTrim>, GeometryError> {
    if curves.is_empty() {
        return Err(invalid("inset produced no cap curves"));
    }
    let start = brep.vertices.len();
    for c in &curves {
        brep.vertices
            .push(BrepVertex::try_new(c.evaluate(*c.domain().start())?, 0.)?);
    }
    let count = curves.len();
    curves
        .into_iter()
        .enumerate()
        .map(|(i, curve)| {
            let uv = surface.try_pullback_affine_curve(&curve, tolerance)?;
            let vertices = [start + i, start + (i + 1) % count];
            let edge = brep.edges.len();
            brep.edges.push(BrepEdge::try_new(vertices, curve, 0.)?);
            let iso = trim_iso::classify(&uv, surface);
            BrepTrim::try_new(
                vertices,
                Some(edge),
                false,
                uv,
                BrepTrimType::Mated,
                iso,
                [0.; 2],
            )
        })
        .collect()
}

pub(super) fn annulus(
    face: &BrepFace,
    boundary: &BrepLoop,
    inset: &[BrepTrim],
) -> Result<BrepFace, GeometryError> {
    let reversed = inset
        .iter()
        .rev()
        .map(reverse_trim)
        .collect::<Result<Vec<_>, _>>()?;
    let (outer, inner) = if boundary.loop_type == BrepLoopType::Outer {
        (boundary.trims.clone(), reversed)
    } else {
        (reversed, boundary.trims.clone())
    };
    BrepFace::try_new(
        face.surface.clone(),
        face.reversed,
        vec![
            BrepLoop::try_new(BrepLoopType::Outer, outer)?,
            BrepLoop::try_new(BrepLoopType::Inner, inner)?,
        ],
    )
}
