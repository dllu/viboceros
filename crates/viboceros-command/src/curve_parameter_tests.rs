use super::*;
use viboceros_document::SelectionMode;

#[test]
fn flip_scale_conversion_and_export_preserve_circle_and_ellipse_domains() {
    let tolerance = Tolerance::DEFAULT;
    let origin = Point3::try_new(1.0, 2.0, 3.0).unwrap();
    let x = UnitVector3::try_new(1.0, 0.0, 0.0, tolerance).unwrap();
    let y = UnitVector3::try_new(0.0, 1.0, 0.0, tolerance).unwrap();
    let z = UnitVector3::try_new(0.0, 0.0, 1.0, tolerance).unwrap();
    let inputs = [
        Geometry::Circle(
            Circle3::try_new(origin, 3.0, z, tolerance)
                .unwrap()
                .try_reparameterized(-7.0..=13.0)
                .unwrap(),
        ),
        Geometry::Ellipse(
            Ellipse3::try_new(origin, 5.0, 2.0, x, y, tolerance)
                .unwrap()
                .try_reparameterized(-7.0..=13.0)
                .unwrap(),
        ),
    ];
    let registry = CommandRegistry::with_builtins();
    for input in inputs {
        let mut document = Document::default();
        let source = document.add_geometry(input.clone()).unwrap();
        document
            .select_object(source, SelectionMode::Replace)
            .unwrap();
        registry.execute(&mut document, "Flip").unwrap();
        let reversed = document.object(source).unwrap().geometry().clone();
        assert_eq!(reversed.curve_ref().unwrap().domain(), -13.0..=7.0);
        for i in 0..=32 {
            let t = input
                .curve_ref()
                .unwrap()
                .parameter_at(i as f64 / 32.0)
                .unwrap();
            assert!(
                input
                    .curve_ref()
                    .unwrap()
                    .evaluate(t)
                    .unwrap()
                    .distance_to(reversed.curve_ref().unwrap().evaluate(-t).unwrap())
                    .unwrap()
                    < 2e-12
            );
        }
        registry.execute(&mut document, "Scale 0,0,0 2").unwrap();
        let scaled = document.object(source).unwrap().geometry().clone();
        assert_eq!(scaled.curve_ref().unwrap().domain(), -13.0..=7.0);
        let exported = geometry_to_3dm(&scaled).unwrap();
        let ThreeDmGeometry::NurbsCurve(exported) = exported else {
            panic!("circle/ellipse interchange is rational")
        };
        assert_eq!(exported.domain(), -13.0..=7.0);
        registry
            .execute(&mut document, "ToNURBS DeleteInput=Yes")
            .unwrap();
        assert_eq!(document.objects().count(), 1);
        let Geometry::NurbsCurve(converted) = document.objects().next().unwrap().geometry() else {
            panic!("conversion did not produce NURBS")
        };
        assert_eq!(converted, &exported);
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.object(source).unwrap().geometry(), &scaled);
    }
}
