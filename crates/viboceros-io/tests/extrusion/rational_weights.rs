//! Native extrusion archives retain their original weight gauge across files.
use super::*;

#[test]
fn scaled_rational_extrusions_round_trip_their_weights_caps_and_tight_bounds() {
    use viboceros_geometry::{Circle3, NurbsCurve, WeightedPoint3};
    let circle = Circle3::try_new(
        Point3::try_new(0., 0., 0.).unwrap(),
        2.,
        Vector3::try_new(0., 0., 1.)
            .unwrap()
            .normalized_nonzero()
            .unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_nurbs()
    .unwrap();
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    for (index, scale) in [2., 7., 1e-100, 1e100].into_iter().enumerate() {
        let profile = NurbsCurve::try_new_rational(
            circle.degree(),
            circle
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(p.point(), p.weight() * scale).unwrap())
                .collect(),
            circle.knots().to_vec(),
        )
        .unwrap();
        let source =
            Brep::try_planar_profile_extrusion(&profile, frame, 3., true, Tolerance::DEFAULT)
                .unwrap();
        let object = ThreeDmObject::new(ThreeDmGeometry::Brep(source.clone()), 0);
        let id = object.id;
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Rational profiles".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![object],
        );
        let path = directory.path().join(format!("weight-{index}.3dm"));
        assert_eq!(
            write_3dm_file(&path, &model).unwrap().written_object_count,
            1
        );
        let imported = read_3dm_file(&path, Tolerance::DEFAULT).unwrap();
        assert_eq!(imported.unsupported_object_count(), 0);
        assert_eq!(imported.objects.len(), 1);
        assert_eq!(imported.objects[0].id, id);
        let ThreeDmGeometry::Brep(actual) = &imported.objects[0].geometry else {
            panic!("missing extrusion")
        };
        assert!(actual.is_extrusion());
        assert!(actual.is_solid());
        let form = actual.extrusion().unwrap().nurbs_form().unwrap();
        assert_eq!(
            form.surface,
            source.extrusion().unwrap().nurbs_form().unwrap().surface
        );
        assert_eq!(
            form.surface
                .control_points()
                .iter()
                .map(|p| p.weight())
                .fold(0f64, f64::max),
            scale
        );
        assert_eq!(actual.explicit_brep(), source.explicit_brep());
        let bounds = actual.tight_bounds(Tolerance::DEFAULT).unwrap();
        assert!(
            bounds
                .min()
                .distance_to(Point3::try_new(-2., -2., 0.).unwrap())
                .unwrap()
                < 1e-6
        );
        assert!(
            bounds
                .max()
                .distance_to(Point3::try_new(2., 2., 3.).unwrap())
                .unwrap()
                < 1e-6
        );
    }
}
