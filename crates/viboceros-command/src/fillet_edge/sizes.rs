//! Apply independently sized edges together to preserve shared corner topology.
use super::*;
use crate::chamfer_edge::ChamferRailType;
use viboceros_smlib::{EdgeBlendProfile, EdgeBlendRailType, Solid};

pub(super) fn native_rail(rail: ChamferRailType) -> EdgeBlendRailType {
    match rail {
        ChamferRailType::DistanceBetweenRails => EdgeBlendRailType::DistanceBetweenRails,
        ChamferRailType::RollingBall => EdgeBlendRailType::RollingBall,
    }
}

pub(super) fn blend_with_sizes(
    brep: &Brep,
    edges: &[(usize, Real)],
    kind: EdgeBlendKind,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    let profile = match kind {
        EdgeBlendKind::Fillet(FilletCorner::Automatic) => EdgeBlendProfile::Fillet,
        EdgeBlendKind::Fillet(FilletCorner::Ball) => {
            return Err(CommandError::Usage(
                "Corner=Ball requires a common radius on each solid",
            ));
        }
        EdgeBlendKind::Chamfer(rail) => EdgeBlendProfile::Chamfer(native_rail(rail)),
        EdgeBlendKind::Curvature(rail) => EdgeBlendProfile::Curvature(native_rail(rail)),
    };
    let (local, to_world) = local_blend_source(brep, tolerance)?;
    let primary = Solid::blend_brep_edge_sizes(&local, edges, profile, tolerance, false);
    let blended = if matches!(profile, EdgeBlendProfile::Fillet) {
        primary.or_else(|error| {
            Solid::blend_brep_edge_sizes(&local, edges, profile, tolerance, true)
                .ok()
                .and_then(validate_local_fillet)
                .ok_or(error)
        })?
    } else {
        primary?
    };
    Ok(blended.transformed(to_world, tolerance)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_edge_blend_sizes_are_solved_on_the_same_original_solid() {
        let mut doc = Document::default();
        let tol = doc.tolerance();
        let frame = CommandContext::default().construction_plane;
        let source = Brep::try_box(frame, [[0., 10.]; 3], tol).unwrap();
        let edge = |y, z| {
            source
                .edges()
                .iter()
                .position(|edge| {
                    edge.vertices().iter().all(|&i| {
                        let point = source.vertices()[i].point();
                        point.y() == y && point.z() == z
                    })
                })
                .unwrap()
        };
        let [first, second] = [edge(0., 0.), edge(10., 10.)];
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        let rail = ChamferRailType::RollingBall;
        for (kind, section) in [
            (
                EdgeBlendKind::Fillet(FilletCorner::Automatic),
                1. - std::f64::consts::PI / 4.,
            ),
            (EdgeBlendKind::Chamfer(rail), 0.5),
            (
                EdgeBlendKind::Curvature(rail),
                3. / 5. - 23. * 2_f64.sqrt() / 70.,
            ),
        ] {
            let plan =
                EdgeBlendPlan::capture_sizes(&doc, [(id, first, 1.), (id, second, 2.)], kind)
                    .unwrap();
            let solved = plan.solve().unwrap();
            let Geometry::Brep(result) = &solved.replacements()[0].1 else {
                panic!("expected a solid")
            };
            let volume = result.signed_volume(tol).unwrap();
            assert!(
                (volume - (1000. - 50. * section)).abs() < 1e-6,
                "{kind:?}: {volume}"
            );
        }
    }
}
