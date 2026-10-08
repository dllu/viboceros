use super::super::super::tests::cube;
use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane(lo: f64, hi: f64) -> Brep {
    Brep::try_surface_face(
        NurbsSurface::try_bilinear([p(1., lo, -1.), p(1., hi, -1.), p(1., hi, 3.), p(1., lo, 3.)])
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
#[test]
fn open_target_normal_selects_the_shared_solid_half_and_keeps_open_remainder() {
    let target = plane(-1., 3.);
    let cutter = cube([[0., 2.]; 3]);
    for (target, expected_x) in [(target.clone(), 0.5), (target.reversed(), 1.5)] {
        let before = (target.clone(), cutter.clone());
        let pieces = split_open_polyhedral_brep(&target, &[&cutter], Tolerance::DEFAULT).unwrap();
        assert_eq!(pieces.len(), 2);
        let solid = pieces.iter().find(|p| p.brep.is_solid()).unwrap();
        let mass = solid
            .brep
            .volume_mass_properties(Tolerance::DEFAULT)
            .unwrap();
        assert!((mass.signed_volume().unwrap() - 4.).abs() < 1e-10);
        assert!((mass.centroid().unwrap().x() - expected_x).abs() < 1e-10);
        let open = pieces.iter().find(|p| !p.brep.is_solid()).unwrap();
        assert!(
            open.brep
                .faces()
                .iter()
                .flat_map(|f| f.loops())
                .flat_map(|l| l.trims())
                .any(|t| t.trim_type() == BrepTrimType::Boundary)
        );
        assert!((open.brep.area(Tolerance::DEFAULT).unwrap() - 24.).abs() < 1e-9);
        assert_eq!((target, cutter.clone()), before);
    }
}
#[test]
fn open_target_finite_coverage_rejects_short_sheet_and_preserves_closed_contract() {
    let partial = plane(-1., 1.);
    let cube = cube([[0., 2.]; 3]);
    assert!(
        split_open_polyhedral_brep(&partial, &[&cube], Tolerance::DEFAULT)
            .unwrap()
            .is_empty()
    );
    let full = plane(-1., 3.);
    assert!(matches!(
        split_polyhedral_brep_with_surfaces(&full, &[&cube], Tolerance::DEFAULT),
        Err(GeometryError::UnsupportedPolyhedralBrepBoolean { .. })
    ));
    assert!(split_open_polyhedral_brep(&cube, &[&full], Tolerance::DEFAULT).is_err());
}
