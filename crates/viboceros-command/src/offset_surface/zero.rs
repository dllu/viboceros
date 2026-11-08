//! Zero-thickness walls remain separate caps and collapsed ruled sheets.
use super::*;

pub(super) fn wall(
    geometry: &Geometry,
    both_sides: bool,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if both_sides {
        let first = match geometry {
            Geometry::NurbsSurface(s) => Brep::try_surface_face(s.clone(), tolerance)?,
            Geometry::Brep(b) if b.faces().len() == 1 => b.clone(),
            _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
        };
        return Ok(vec![Geometry::Brep(Brep::try_disjoint_union(
            vec![first.clone(), first.reversed()],
            tolerance,
        )?)]);
    }
    let loops = match geometry {
        Geometry::NurbsSurface(s) => s.natural_boundary_curve_loops()?,
        Geometry::Brep(b) if b.faces().len() == 1 => b.face_boundary_curve_components(0)?,
        _ => return Err(CommandError::OffsetSurfaceRequiresPlanarFaces),
    };
    let mut result = vec![geometry.clone(), geometry.clone()];
    for curve in loops.into_iter().flatten() {
        let controls = curve
            .control_points()
            .iter()
            .copied()
            .cycle()
            .take(2 * curve.control_points().len())
            .collect();
        let surface = NurbsSurface::try_new_rational(
            curve.degree(),
            1,
            curve.control_points().len(),
            2,
            controls,
            curve.knots().to_vec(),
            vec![0., 0., 1., 1.],
        )?;
        result.push(Geometry::Brep(Brep::try_surface_face(surface, tolerance)?));
    }
    Ok(result)
}
