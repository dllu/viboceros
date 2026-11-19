//! Truncate the conical sweep, retaining the picked support and its trim holes.
use super::*;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use viboceros_smlib::{BooleanOperation, Shape, Solid};

pub(super) const NO_SECTION: &str =
    "The boundary must span the complete extrusion between the source and the apex";

pub(super) fn truncate(
    cone: &Brep,
    boundary: &Brep,
    apex: Point3,
    tolerance: Tolerance,
) -> Result<Brep, CommandError> {
    // The radial cutter shares the apex, so its side walls do not clip a
    // wider source base when the finite boundary covers the cut section.
    let cutter = Shape::from_brep(boundary, tolerance)?
        .extruded_faces_to_point(apex, true, tolerance)?
        .to_brep(tolerance)?;
    let cone_native = Solid::from_brep(cone, tolerance)?;
    let cutter_native = Solid::from_brep(&cutter, tolerance)?;
    let kept = cone_native.boolean_with_postprocess(
        &cutter_native,
        BooleanOperation::Difference,
        false,
    )?;
    if kept.is_empty()? {
        return Err(CommandError::Usage(NO_SECTION));
    }
    let output = kept.to_brep(tolerance)?;

    // Compare the carriers against the imported operands. Import can
    // standardize closed seams and orientation while retaining the bases.
    let cone = cone_native.to_brep(tolerance)?;
    let cutter = cutter_native.to_brep(tolerance)?;
    let cap = |brep: &Brep| {
        brep.faces()
            .iter()
            .position(|face| !reaches_apex(brep, face, apex, tolerance))
            .ok_or(CommandError::Usage(NO_SECTION))
    };
    let base_index = cap(&cone)?;
    let boundary_surface = cutter.faces()[cap(&cutter)?].surface();
    let has_holes = boundary.faces().iter().any(|face| face.loops().len() > 1);
    let mut by_support = HashMap::<u64, Vec<usize>>::new();
    for (index, face) in cone.faces().iter().enumerate() {
        by_support
            .entry(surface_key(face.surface()))
            .or_default()
            .push(index);
    }

    let mut faces = Vec::new();
    let mut base_pieces = Vec::new();
    let mut cap_count = 0;
    for (index, face) in output.faces().iter().enumerate() {
        let reaches_apex = reaches_apex(&output, face, apex, tolerance);
        if let Some(matches) = by_support.get(&surface_key(face.surface()))
            && let Some(&source) = matches
                .iter()
                .find(|&&source| cone.faces()[source].surface() == face.surface())
        {
            if reaches_apex {
                return Err(CommandError::Usage(NO_SECTION));
            }
            if source == base_index {
                base_pieces.push(index);
            }
        } else if face.surface() == boundary_surface && !reaches_apex {
            cap_count += 1;
        } else if reaches_apex && has_holes {
            // Material surviving above a hole is a spike terminating at the
            // shared apex. Discard its radial faces and keep the opening in
            // the copied boundary cap, as Rhino does.
            continue;
        } else {
            return Err(CommandError::Usage(NO_SECTION));
        }
        faces.push(index);
    }
    if cap_count == 0 || base_pieces.is_empty() {
        return Err(CommandError::Usage(NO_SECTION));
    }
    let base = cone.duplicate_faces(&[base_index], tolerance)?;
    let area = base.area(tolerance)?;
    let retained_area = output.sub_brep(&base_pieces, tolerance)?.area(tolerance)?;
    let width = base.bounds().min().distance_to(base.bounds().max())?;
    let area_tolerance = 4. * width * tolerance.absolute() + area * tolerance.relative();
    if (area - retained_area).abs() > area_tolerance {
        return Err(CommandError::Usage(NO_SECTION));
    }
    if faces.len() == output.faces().len() {
        Ok(output)
    } else {
        Ok(output.sub_brep(&faces, tolerance)?)
    }
}

fn reaches_apex(
    brep: &Brep,
    face: &viboceros_geometry::BrepFace,
    apex: Point3,
    tolerance: Tolerance,
) -> bool {
    face.loops()
        .iter()
        .flat_map(|boundary| boundary.trims())
        .flat_map(|trim| trim.vertices())
        .any(|index| brep.vertices()[index].point().is_near(apex, tolerance))
}

fn surface_key(surface: &NurbsSurface) -> u64 {
    // Use geometric data instead of face order for provenance. Confirm exact
    // carrier equality after lookup to guard against hash collisions.
    let mut key = DefaultHasher::new();
    surface.degree_u().hash(&mut key);
    surface.degree_v().hash(&mut key);
    let mut real = |number: f64| {
        (if number == 0. { 0 } else { number.to_bits() }).hash(&mut key);
    };
    for &knot in surface.knots_u().iter().chain(surface.knots_v()) {
        real(knot);
    }
    for point in surface.control_points() {
        for coordinate in point.point().to_array() {
            real(coordinate);
        }
        real(point.weight());
    }
    key.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64, z: f64) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    #[test]
    fn surface_apex_boundary_cut_reuses_the_planar_or_warped_cap() {
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let face = |points| {
            Brep::try_surface_face(NurbsSurface::try_bilinear(points).unwrap(), tol).unwrap()
        };
        let base = face([p(0., 0., 0.), p(2., 0., 0.), p(2., 3., 0.), p(0., 3., 0.)]);
        let apex = p(1., 1.5, 12.);
        let cone = Shape::from_brep(&base, tol)
            .unwrap()
            .extruded_faces_to_point(apex, true, tol)
            .unwrap()
            .to_brep(tol)
            .unwrap();
        for (corners, rim, volume) in [
            (
                [
                    p(-1., -1., 6.),
                    p(3., -1., 6.),
                    p(3., 4., 6.),
                    p(-1., 4., 6.),
                ],
                [
                    p(0.5, 0.75, 6.),
                    p(1.5, 0.75, 6.),
                    p(1.5, 2.25, 6.),
                    p(0.5, 2.25, 6.),
                ],
                21.,
            ),
            (
                [
                    p(-1., -1., 4.75),
                    p(3., -1., 4.75),
                    p(3., 4., 7.25),
                    p(-1., 4., 7.25),
                ],
                [
                    p(0.466666666666661, 0.7, 5.6),
                    p(1.533333333333339, 0.7, 5.6),
                    p(1.470588235294118, 2.205882352941174, 6.352941176470587),
                    p(0.529411764705884, 2.205882352941174, 6.352941176470587),
                ],
                20.976427994149816,
            ),
            (
                [
                    p(-5., -5., 5.),
                    p(7., -5., 6.),
                    p(7., 8., 8.),
                    p(-5., 8., 6.),
                ],
                [
                    p(1.481280475378906, 0.778079286931643, 6.224634295453122),
                    p(0.508826008126782, 0.763239012190169, 6.105912097521372),
                    p(1.467381981300669, 2.201072971951, 6.391416224391973),
                    p(0.522563491125297, 2.216154763312062, 6.270761893503529),
                ],
                21.357920757977315,
            ),
        ] {
            let boundary = face(corners);
            let clipped = truncate(&cone, &boundary, apex, tol).unwrap();
            assert!(clipped.is_solid());
            assert_eq!(clipped.faces().len(), 6);
            assert_eq!(clipped.vertices().len(), 8);
            for point in rim {
                assert!(
                    clipped
                        .vertices()
                        .iter()
                        .any(|vertex| vertex.point().is_near(point, tol)),
                    "{point:?}"
                );
            }
            let target = boundary.faces()[0].surface();
            for edge in clipped.edges().iter().filter(|edge| {
                edge.vertices()
                    .into_iter()
                    .all(|index| clipped.vertices()[index].point().z() > 1.)
            }) {
                let curve = edge.curve();
                let domain = curve.domain();
                for fraction in [0.25, 0.5, 0.75] {
                    let point = curve
                        .evaluate((1. - fraction) * domain.start() + fraction * domain.end())
                        .unwrap();
                    let (u, v) = target.closest_parameters(point, tol).unwrap();
                    assert!(
                        target.evaluate(u, v).unwrap().is_near(point, tol),
                        "{point:?}"
                    );
                }
            }
            let actual = clipped.signed_volume(tol).unwrap();
            // Convert the model-space tolerance to a volume tolerance for
            // the 2-by-3 cross section before comparing integrated values.
            assert!(
                (actual - volume).abs() < 6. * tol.absolute(),
                "{corners:?}: {actual} != {volume}"
            );
        }
    }

    #[test]
    fn surface_apex_boundary_retains_inner_openings_and_declines_partial_coverage() {
        let tol = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let rect = |w: f64, h: f64, z| [p(-w, -h, z), p(w, -h, z), p(w, h, z), p(-w, h, z)];
        let face = |corners| {
            Brep::try_surface_face(NurbsSurface::try_bilinear(corners).unwrap(), tol).unwrap()
        };
        let base = face(rect(1., 1.5, 0.));
        let apex = p(0., 0., 12.);
        let cone = Shape::from_brep(&base, tol)
            .unwrap()
            .extruded_faces_to_point(apex, true, tol)
            .unwrap()
            .to_brep(tol)
            .unwrap();
        let rim = rect(4., 4., 6.);
        let rim = Polyline3::try_new(rim.into_iter().chain([rim[0]]).collect(), tol)
            .unwrap()
            .to_nurbs()
            .unwrap();
        let hole = Circle3::try_new(
            p(0., 0., 6.),
            0.25,
            UnitVector3::try_new(0., 0., 1., tol).unwrap(),
            tol,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let boundary = Brep::try_planar_face_with_holes(&rim, &[hole], tol).unwrap();
        let output = truncate(&cone, &boundary, apex, tol).unwrap();
        assert!(!output.is_solid());
        assert_eq!(output.faces().len(), 6);
        assert!(output.faces().iter().any(|face| face.loops().len() == 2));
        assert!(
            output
                .vertices()
                .iter()
                .all(|vertex| vertex.point().z() <= 6. + 1e-6)
        );
        assert!(truncate(&cone, &face(rect(0.3, 0.4, 6.)), apex, tol).is_err());
        assert!(truncate(&cone, &face(rect(4., 4., 13.)), apex, tol).is_err());
    }
}
