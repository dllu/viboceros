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
    edges: &[(usize, EdgeBlendSizes)],
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
    let primary = Solid::blend_brep_edge_profiles(&local, edges, profile, tolerance, false);
    let blended = if matches!(profile, EdgeBlendProfile::Fillet) {
        primary.or_else(|error| {
            Solid::blend_brep_edge_profiles(&local, edges, profile, tolerance, true)
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

    #[test]
    fn edge_blend_end_sizes_are_applied_at_the_original_curve_endpoints() {
        let mut doc = Document::default();
        let tol = doc.tolerance();
        let frame = CommandContext::default().construction_plane;
        let source = Brep::try_box(frame, [[0., 10.]; 3], tol).unwrap();
        let index = source
            .edges()
            .iter()
            .position(|edge| {
                edge.vertices().iter().all(|&i| {
                    let point = source.vertices()[i].point();
                    point.y() == 0. && point.z() == 0.
                })
            })
            .unwrap();
        let curve = source.edges()[index].curve();
        let x = [*curve.domain().start(), *curve.domain().end()]
            .map(|t| curve.evaluate(t).unwrap().x());
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        let rail = ChamferRailType::RollingBall;
        for kind in [
            EdgeBlendKind::Fillet(FilletCorner::Automatic),
            EdgeBlendKind::Chamfer(rail),
            EdgeBlendKind::Curvature(rail),
        ] {
            let solved = EdgeBlendPlan::capture_end_sizes(&doc, [(id, index, [1., 2.])], kind)
                .unwrap()
                .solve()
                .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            let Geometry::Brep(result) = &solved.replacements()[0].1 else {
                panic!("expected a solid")
            };
            assert!(result.is_solid());
            for (x, size) in x.into_iter().zip([1., 2.]) {
                for [y, z] in [[size, 0.], [0., size]] {
                    let expected = Point3::try_new(x, y, z).unwrap();
                    assert!(
                        result.vertices().iter().any(|vertex| vertex
                            .point()
                            .distance_to(expected)
                            .unwrap()
                            < 1e-6),
                        "{kind:?}: missing {expected:?}"
                    );
                }
            }
            for [y, z] in [[1.5, 0.], [0., 1.5]] {
                let expected = Point3::try_new(x[0].midpoint(x[1]), y, z).unwrap();
                assert!(
                    result.edges().iter().any(|edge| {
                        let curve = edge.curve();
                        curve
                            .closest_parameter(expected, tol)
                            .and_then(|parameter| curve.evaluate(parameter))
                            .and_then(|point| point.distance_to(expected))
                            .is_ok_and(|gap| gap < 1e-6)
                    }),
                    "{kind:?}: missing midpoint setback {expected:?}"
                );
            }
        }
        let width = EdgeBlendKind::Chamfer(ChamferRailType::DistanceBetweenRails);
        let result = EdgeBlendPlan::capture_end_sizes(&doc, [(id, index, [1., 2.])], width)
            .unwrap()
            .solve();
        assert!(result.unwrap_err().to_string().contains("RollingBall"));
    }

    #[test]
    fn edge_blend_interior_sizes_are_applied_in_the_original_curve_domain() {
        let mut doc = Document::default();
        let tol = doc.tolerance();
        let frame = CommandContext::default().construction_plane;
        let source = Brep::try_box(frame, [[0., 10.]; 3], tol).unwrap();
        let index = source
            .edges()
            .iter()
            .position(|edge| {
                edge.vertices().iter().all(|&i| {
                    let point = source.vertices()[i].point();
                    point.y() == 0. && point.z() == 0.
                })
            })
            .unwrap();
        let profile = EdgeBlendSizes {
            ends: [1., 1.],
            interior: vec![(0.4, 2.)],
        };
        let curve = source.edges()[index].curve();
        let domain = curve.domain();
        let sections = profile
            .stations()
            .map(|(fraction, size)| {
                let parameter = (domain.end() - domain.start()).mul_add(fraction, *domain.start());
                let x = curve.evaluate(parameter).unwrap().x();
                [
                    Point3::try_new(x, size, 0.).unwrap(),
                    Point3::try_new(x, 0., size).unwrap(),
                ]
            })
            .collect::<Vec<_>>();
        let id = doc.add_geometry(Geometry::Brep(source)).unwrap();
        let rail = ChamferRailType::RollingBall;
        for kind in [
            EdgeBlendKind::Fillet(FilletCorner::Automatic),
            EdgeBlendKind::Chamfer(rail),
            EdgeBlendKind::Curvature(rail),
        ] {
            let solved =
                EdgeBlendPlan::capture_profiles(&doc, [(id, index, profile.clone())], kind)
                    .unwrap()
                    .solve()
                    .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            let Geometry::Brep(result) = &solved.replacements()[0].1 else {
                panic!("expected a solid")
            };
            assert!(result.is_solid());
            for expected in sections.iter().flatten() {
                assert!(
                    result.edges().iter().any(|edge| {
                        let curve = edge.curve();
                        curve
                            .closest_parameter(*expected, tol)
                            .and_then(|parameter| curve.evaluate(parameter))
                            .and_then(|point| point.distance_to(*expected))
                            .is_ok_and(|gap| gap < 1e-6)
                    }),
                    "{kind:?}: missing setback {expected:?}"
                );
            }
        }
    }
}
