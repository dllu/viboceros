//! Compare the finite cutter with the complete section of its carrier.
use super::*;
use viboceros_geometry::BrepLoop;

pub(in crate::tapered_boundary::oblique) enum Coverage {
    Complete(Brep),
    MissedRim,
    Unresolved,
}

pub(in crate::tapered_boundary::oblique) fn recover(
    sweep: &Shape,
    stock: &Brep,
    source: &Brep,
    cutter: &Brep,
    displacement: Vector3,
    cap: bool,
    tolerance: Tolerance,
) -> Result<Coverage, CommandError> {
    let face = &cutter.faces()[0];
    let plane = face.surface().plane(tolerance)?;
    let support = match plane {
        Some(plane) => support::planar(stock, plane, displacement, tolerance),
        None => support::warped(stock, face.surface(), displacement, tolerance),
    };
    let section = support.and_then(|support| {
        clip(
            sweep,
            stock,
            source,
            &support,
            displacement,
            true,
            tolerance,
        )
    });
    let section = match section {
        Ok(section) => section,
        Err(CommandError::SurfaceExtrusionBoundaryRejected) => return Ok(Coverage::Unresolved),
        Err(error) => return Err(error),
    };
    let cap_index = section.faces().len() - 1;
    let end = &section.faces()[cap_index];
    let outer = &face.loops()[0];
    for trim in outer.trims() {
        let edge = cutter.edges()[trim.edge().ok_or(REJECTED)?].curve();
        for rim in end.loops().iter().flat_map(|rim| rim.trims()) {
            let rim = section.edges()[rim.edge().ok_or(REJECTED)?].curve();
            if !crate::curve_intersections::events(edge, rim, tolerance)?.is_empty() {
                return Ok(Coverage::Unresolved);
            }
        }
    }
    // The rims share a carrier and are disjoint. One point on each rim
    // determines whether the cutter contains that entire rim.
    if !contains(face, loop_start(end, &end.loops()[0])?, tolerance)? {
        for inner in &end.loops()[1..] {
            if contains(face, loop_start(end, inner)?, tolerance)? {
                return Ok(Coverage::Unresolved);
            }
        }
        return Ok(Coverage::MissedRim);
    }

    if !cap {
        let base = source.faces()[0]
            .surface()
            .plane(tolerance)?
            .ok_or(REJECTED)?;
        let walls = (0..cap_index)
            .filter(|&i| !on_plane(section.faces()[i].surface(), base, tolerance))
            .collect::<Vec<_>>();
        return Ok(Coverage::Complete(section.sub_brep(&walls, tolerance)?));
    }
    let restored = if plane.is_some() {
        mount_planar_cap(section, cap_index, face.surface(), tolerance)?
    } else {
        let stock = section.sub_brep(&(0..cap_index).collect::<Vec<_>>(), tolerance)?;
        let Some(restored) = stock.try_cap_on_surface(face.surface(), tolerance)? else {
            return Ok(Coverage::Unresolved);
        };
        restored
    };
    Ok(Coverage::Complete(restored))
}

fn loop_start(face: &BrepFace, rim: &BrepLoop) -> Result<Point3, GeometryError> {
    let curve = rim.trims()[0].curve();
    let uv = curve.evaluate(*curve.domain().start())?;
    face.surface().evaluate(uv.x(), uv.y())
}

fn contains(face: &BrepFace, point: Point3, tolerance: Tolerance) -> Result<bool, GeometryError> {
    let (u, v) = face.surface().closest_parameters(point, tolerance)?;
    Ok(face.surface().evaluate(u, v)?.is_near(point, tolerance)
        && face.contains_parameters(u, v, tolerance)?)
}
