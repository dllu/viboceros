use super::*;
use viboceros_geometry::{Brep, Frame3, NurbsSurface, Vector3};

fn frame(center: [f64; 3]) -> Frame3 {
    Frame3::try_from_normal(
        Point3::try_new(center[0], center[1], center[2]).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn sphere(center: [f64; 3], radius: f64, inward: bool) -> Brep {
    let mut result = Brep::try_surface_grid(
        &NurbsSurface::try_sphere(frame(center), radius).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap();
    if inward {
        result.reverse_orientation();
    }
    result
}
fn torus() -> Brep {
    Brep::try_surface_grid(
        &NurbsSurface::try_torus(frame([0.; 3]), 4., 1.).unwrap(),
        &[],
        &[],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn export(parts: Vec<Brep>) -> (Brep, String, StepNativeImport) {
    let source = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
    let before = source.clone();
    let mut bytes = Vec::new();
    write_step_nurbs_breps(&mut bytes, [&source]).unwrap();
    assert_eq!(
        source, before,
        "native classification changed source geometry"
    );
    let text = String::from_utf8(bytes).unwrap();
    let imported = read_step_native_instances(Cursor::new(&text), Tolerance::DEFAULT).unwrap();
    let volume = imported
        .instances
        .iter()
        .map(|i| i.brep.signed_volume(Tolerance::DEFAULT).unwrap())
        .sum::<f64>();
    let expected = source.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (volume - expected).abs() <= 1e-8 * expected.abs(),
        "signed material volume changed"
    );
    assert!(imported.instances.iter().all(|i| i.brep.is_solid()));
    (source, text, imported)
}

#[test]
fn curved_cavity_shells_keep_one_shape_in_any_component_order() {
    for reverse in [false, true] {
        let mut parts = vec![sphere([0.; 3], 2., true), sphere([0.; 3], 4., false)];
        if reverse {
            parts.reverse();
        }
        let (_, text, imported) = export(parts);
        assert_eq!(text.matches("BREP_WITH_VOIDS(").count(), 1);
        assert!(
            text.lines()
                .filter(|l| l.contains("ORIENTED_CLOSED_SHELL("))
                .all(|line| line.ends_with(".F.);"))
        );
        assert!(!text.contains("SHELL_BASED_SURFACE_MODEL("));
        assert_eq!(imported.instances.len(), 2);
        assert_eq!(
            imported.instances[0].source_shape_id,
            imported.instances[1].source_shape_id
        );
        assert!(
            imported.instances[0]
                .brep
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                > 0.
        );
        assert!(
            imported.instances[1]
                .brep
                .signed_volume(Tolerance::DEFAULT)
                .unwrap()
                < 0.
        );
    }
}

#[test]
fn nested_material_island_is_a_separate_solid_inside_a_curved_void() {
    let (_, text, imported) = export(vec![
        sphere([0.; 3], 0.5, false),
        sphere([0.; 3], 2., true),
        sphere([0.; 3], 4., false),
    ]);
    assert_eq!(text.matches("BREP_WITH_VOIDS(").count(), 1);
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 1);
    assert_eq!(imported.instances.len(), 3);
    let shapes = imported
        .instances
        .iter()
        .map(|i| i.source_shape_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(shapes.len(), 2);
}

#[test]
fn nonconvex_torus_tube_preserves_a_spherical_cavity() {
    let (_, text, imported) = export(vec![sphere([4., 0., 0.], 0.25, true), torus()]);
    assert_eq!(text.matches("BREP_WITH_VOIDS(").count(), 1);
    assert_eq!(imported.instances.len(), 2);
}

#[test]
fn sphere_in_torus_hole_is_not_mistaken_for_a_contained_shell() {
    let (_, text, imported) = export(vec![sphere([0.; 3], 0.5, false), torus()]);
    assert!(!text.contains("BREP_WITH_VOIDS("));
    assert_eq!(text.matches("MANIFOLD_SOLID_BREP(").count(), 2);
    assert_ne!(
        imported.instances[0].source_shape_id,
        imported.instances[1].source_shape_id
    );
}

#[test]
fn touching_crossing_and_wrong_sense_shells_fail_before_writing() {
    for parts in [
        vec![sphere([0.; 3], 2., false), sphere([4., 0., 0.], 2., false)],
        vec![sphere([0.; 3], 2., false), sphere([3., 0., 0.], 2., false)],
        vec![sphere([0.; 3], 4., false), sphere([0.; 3], 2., false)],
    ] {
        let source = Brep::try_combine(parts, Tolerance::DEFAULT).unwrap();
        let mut output = Vec::new();
        assert!(matches!(
            write_step_nurbs_breps(&mut output, [&source]),
            Err(StepError::NativeShellClassification { .. })
        ));
        assert!(output.is_empty());
    }
}

#[test]
fn curved_void_groups_and_oriented_volume_survive_unit_conversion() {
    let source = Brep::try_combine(
        vec![sphere([0.; 3], 2., true), sphere([0.; 3], 4., false)],
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
    let imported = read_step_native_instances_in_units(
        Cursor::new(bytes),
        &LengthUnitSystem::Meters,
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(imported.instances.len(), 2);
    assert_eq!(
        imported.instances[0].source_shape_id,
        imported.instances[1].source_shape_id
    );
    let volume = imported
        .instances
        .iter()
        .map(|i| i.brep.signed_volume(Tolerance::DEFAULT).unwrap())
        .sum::<f64>();
    assert!((volume - source.signed_volume(Tolerance::DEFAULT).unwrap()).abs() < 1e-8);
}
