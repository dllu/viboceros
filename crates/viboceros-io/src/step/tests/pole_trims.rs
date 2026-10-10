use super::*;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Vector3};

fn frame() -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn closed_circle_edges_are_not_imported_as_singular_trims() {
    let source = Brep::try_cylinder(frame(), 2., 0., 5., Tolerance::DEFAULT).unwrap();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    let decoded = read_step_native_instances(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    let result = &decoded.instances[0].brep;
    assert_eq!(result.edges().len(), source.edges().len());
    let circles = result
        .edges()
        .iter()
        .filter(|edge| {
            edge.vertices()[0] == edge.vertices()[1]
                && edge.curve().length(Tolerance::DEFAULT).unwrap() > 10.
        })
        .count();
    assert_eq!(circles, 2);
    let singular = |brep: &Brep| {
        brep.faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .filter(|trim| trim.edge().is_none())
            .count()
    };
    assert_eq!(singular(result), singular(&source));
}

#[test]
fn collapsed_support_with_a_noncollapsed_surface_image_is_rejected() {
    let source = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    // Keep both UV endpoints at the south pole but make the interior leave it.
    // This passes UV-loop closure and defeats endpoint-only collapse tests.
    let point_id = |coordinates: &str| {
        text.lines()
            .find(|line| line.contains(coordinates))
            .unwrap()
            .split_once(" = ")
            .unwrap()
            .0
    };
    let first = point_id("CARTESIAN_POINT('', (0.0, -1.5707963267948966));");
    let last = point_id("CARTESIAN_POINT('', (6.283185307179586, -1.5707963267948966));");
    let controls = format!("B_SPLINE_CURVE(1, ({first}, {last}),");
    let position = text.find(&controls).unwrap();
    let start = text[..position].rfind("\n#").unwrap();
    let end = position + text[position..].find("\n);").unwrap() + 3;
    let original = &text[start..end];
    let changed = original
        .replace(
            &controls,
            &format!("B_SPLINE_CURVE(2, ({first}, #1000000, {last}),"),
        )
        .replace(
            "B_SPLINE_CURVE_WITH_KNOTS((2, 2)",
            "B_SPLINE_CURVE_WITH_KNOTS((3, 3)",
        )
        .replace(
            "RATIONAL_B_SPLINE_CURVE((1.0, 1.0))",
            "RATIONAL_B_SPLINE_CURVE((1.0, 1.0, 1.0))",
        );
    assert_ne!(original, changed);
    let invalid = text.replacen(original, &changed, 1).replacen(
        "ENDSEC;\nEND-ISO-10303-21;",
        "#1000000 = CARTESIAN_POINT('', (3.141592653589793, 0.0));\nENDSEC;\nEND-ISO-10303-21;",
        1,
    );
    let error = read_step_native_instances(Cursor::new(invalid), Tolerance::DEFAULT).unwrap_err();
    assert!(
        matches!(
            error,
            StepError::UnsupportedNativeShell {
                reason: "collapsed STEP edge does not certify a singular surface boundary",
                ..
            }
        ),
        "{error:?}"
    );
}
#[test]
fn sphere_pole_trims_round_trip_as_exact_singular_boundaries() {
    let source = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    let decoded = read_step_native_instances(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.instances.len(), 1);
    let result = &decoded.instances[0].brep;
    assert!(result.is_solid());
    assert_eq!(result.edges().len(), source.edges().len());
    let singular = result
        .faces()
        .iter()
        .flat_map(|f| f.loops())
        .flat_map(|l| l.trims())
        .filter(|t| t.edge().is_none())
        .count();
    assert_eq!(singular, 2);
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap() - 32. * std::f64::consts::PI / 3.).abs()
            < 1e-8
    );
}
#[test]
fn cone_apex_has_a_certified_pole_trim_after_step_transfer() {
    let source = Brep::try_cone(frame(), 2., 5., Tolerance::DEFAULT).unwrap();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    let decoded = read_step_native_instances(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.instances.len(), 1);
    let result = &decoded.instances[0].brep;
    assert!(result.is_solid());
    assert!(
        result
            .faces()
            .iter()
            .flat_map(|f| f.loops())
            .flat_map(|l| l.trims())
            .any(|t| t.edge().is_none())
    );
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap() - 20. * std::f64::consts::PI / 3.).abs()
            < 1e-8
    );
}

#[test]
fn reversed_sphere_poles_keep_face_sense_and_oriented_volume() {
    let mut source = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    source.reverse_orientation();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    let decoded = read_step_native_instances(Cursor::new(bytes), Tolerance::DEFAULT).unwrap();
    assert_eq!(decoded.instances.len(), 1);
    let result = &decoded.instances[0].brep;
    assert!(result.is_solid());
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap() + 32. * std::f64::consts::PI / 3.).abs()
            < 1e-8
    );
}

#[test]
fn pole_export_respects_source_units_without_rescaling_uv_geometry() {
    let source = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(), 0.002).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let mut bytes = Vec::new();
    write_step_nurbs_breps_in_units(
        &mut bytes,
        [&source],
        &LengthUnitSystem::Meters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let decoded = read_step_native_instances_in_units(
        Cursor::new(bytes),
        &LengthUnitSystem::Meters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(decoded.instances.len(), 1);
    let result = &decoded.instances[0].brep;
    assert!(result.is_solid());
    assert!(
        (result.signed_volume(Tolerance::DEFAULT).unwrap()
            - source.signed_volume(Tolerance::DEFAULT).unwrap())
        .abs()
            < 1e-14
    );
}
