use super::*;

#[test]
fn reparameterized_circle_edits_use_native_parameters_and_retain_document_state() {
    let registry = CommandRegistry::with_builtins();
    for command in [
        "SubCrv Parameter=-4,9",
        "Split Parameter=-4,9",
        "CrvSeam Parameter=-4",
    ] {
        let mut document = Document::default();
        registry.execute(&mut document, "Circle 0,0 3").unwrap();
        let id = document.objects().next().unwrap().id();
        document.select_object(id, SelectionMode::Replace).unwrap();
        registry
            .execute(&mut document, "Group NativeEdits")
            .unwrap();
        registry
            .execute(&mut document, "SetObjectName NativeCircle")
            .unwrap();
        registry
            .execute(&mut document, "Reparameterize -7 13")
            .unwrap();
        let before = document.object(id).unwrap().clone();
        let Geometry::Circle(circle) = before.geometry() else {
            panic!("reparameterization must preserve circles")
        };
        assert_eq!(circle.domain(), -7.0..=13.0);
        registry.execute(&mut document, command).unwrap();
        let curve = document.object(id).unwrap().geometry().curve_ref().unwrap();
        assert_eq!(*curve.domain().start(), -4.0);
        assert!(
            curve
                .start_point()
                .unwrap()
                .distance_to(circle.evaluate(-4.0).unwrap())
                .unwrap()
                < 1e-12
        );
        if command.starts_with("CrvSeam") {
            assert!(matches!(
                document.object(id).unwrap().geometry(),
                Geometry::Circle(_)
            ));
            assert_eq!(curve.domain(), -4.0..=16.0);
        } else {
            assert!(matches!(
                document.object(id).unwrap().geometry(),
                Geometry::Arc(_)
            ));
            assert_eq!(curve.domain(), -4.0..=9.0);
            for i in 0..=64 {
                let t = curve.parameter_at(i as Real / 64.0).unwrap();
                assert!(
                    curve
                        .evaluate(t)
                        .unwrap()
                        .distance_to(circle.evaluate(t).unwrap())
                        .unwrap()
                        < 1e-12
                );
            }
        }
        for object in document.objects() {
            assert_eq!(object.attributes(), before.attributes());
            assert!(
                document
                    .group_by_name("NativeEdits")
                    .unwrap()
                    .members()
                    .any(|member| member == object.id())
            );
        }
        registry.execute(&mut document, "Undo").unwrap();
        assert_eq!(document.objects().count(), 1);
        assert_eq!(document.object(id).unwrap(), &before);
    }
}

#[test]
fn failed_native_edits_are_atomic_and_make_no_history_entry() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Circle 0,0 3").unwrap();
    let id = document.objects().next().unwrap().id();
    document.select_object(id, SelectionMode::Replace).unwrap();
    registry
        .execute(&mut document, "Reparameterize -7,13")
        .unwrap();
    let before = document.object(id).unwrap().clone();
    let history = document.undo_label().map(str::to_owned);
    for command in [
        "Split Parameter=-7",
        "Split Parameter=1,1",
        "SubCrv Parameter=-8,2",
        "CrvSeam Parameter=NaN",
        "Reparameterize 2,2",
    ] {
        assert!(registry.execute(&mut document, command).is_err());
        assert_eq!(document.objects().count(), 1);
        assert_eq!(document.object(id).unwrap(), &before);
        assert_eq!(document.undo_label(), history.as_deref());
    }
}
