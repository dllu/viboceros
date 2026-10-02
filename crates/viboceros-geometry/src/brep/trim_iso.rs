//! Isoparametric trim classification from the complete rational control net.
use super::*;

pub(super) fn classify(curve: &NurbsCurve2, surface: &NurbsSurface) -> SurfaceIso {
    let first = curve.control_points()[0].point();
    let u = surface.domain_u();
    let v = surface.domain_v();
    if curve
        .control_points()
        .iter()
        .all(|p| p.point().x() == first.x())
    {
        if first.x() == *u.start() {
            SurfaceIso::West
        } else if first.x() == *u.end() {
            SurfaceIso::East
        } else {
            SurfaceIso::InteriorUConstant
        }
    } else if curve
        .control_points()
        .iter()
        .all(|p| p.point().y() == first.y())
    {
        if first.y() == *v.start() {
            SurfaceIso::South
        } else if first.y() == *v.end() {
            SurfaceIso::North
        } else {
            SurfaceIso::InteriorVConstant
        }
    } else {
        SurfaceIso::NotIso
    }
}
