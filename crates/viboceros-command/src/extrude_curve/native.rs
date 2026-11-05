//! Native command regressions for curve conditioning and transactional failure.
use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclamped_and_negative_gauge_profiles_keep_their_active_locus_and_source() {
        for negative in [false, true] {
            let source = NurbsCurve::try_new_rational(
                2,
                (0..4)
                    .map(|i| {
                        WeightedPoint3::try_new(
                            Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap(),
                            if negative { -1. } else { 1. },
                        )
                        .unwrap()
                    })
                    .collect(),
                vec![0., 1., 2., 3., 4., 5., 6.],
            )
            .unwrap();
            let before = source.clone();
            let mut document = Document::default();
            let registry = CommandRegistry::with_builtins();
            registry
                .execute(&mut document, "UseExtrusions Polysurface")
                .unwrap();
            let source_id = document
                .add_geometry(Geometry::NurbsCurve(source.clone()))
                .unwrap();
            document
                .select_objects_direct([source_id], SelectionMode::Replace)
                .unwrap();
            registry
                .execute(&mut document, "ExtrudeCrv 0,0,0 0,3,4 BothSides=Yes")
                .unwrap();
            let Geometry::Brep(result) = document
                .objects()
                .find(|o| o.id() != source_id)
                .unwrap()
                .geometry()
            else {
                panic!("explicit B-rep")
            };
            assert!(!result.is_extrusion());
            assert_eq!(
                document.object(source_id).unwrap().geometry(),
                &Geometry::NurbsCurve(before.clone())
            );
            let mut covered = 0.;
            for face in result.faces() {
                let surface = face.surface();
                let interval = surface.domain_u();
                assert!(*interval.start() >= *source.domain().start());
                assert!(*interval.end() <= *source.domain().end());
                covered += interval.end() - interval.start();
                assert_eq!(surface.domain_v(), 0. ..=10.);
                for i in 0..=32 {
                    let u =
                        *interval.start() + (interval.end() - interval.start()) * i as f64 / 32.;
                    for j in 0..=8 {
                        let f = j as f64 / 8.;
                        let offset = Vector3::try_new(0., -3. + 6. * f, -4. + 8. * f).unwrap();
                        let expected = source.evaluate(u).unwrap().translated(offset).unwrap();
                        assert!(
                            surface
                                .evaluate(u, 10. * f)
                                .unwrap()
                                .distance_to(expected)
                                .unwrap()
                                < 1e-12
                        );
                    }
                }
            }
            assert_eq!(covered, source.domain().end() - source.domain().start());
            assert_eq!(source, before);
        }
    }

    #[test]
    fn later_native_profile_rejection_preserves_the_entire_batch_and_redo() {
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        let valid = document
            .add_geometry(Geometry::Line(
                LineSegment::try_new(
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(4., 0., 0.).unwrap(),
                    document.tolerance(),
                )
                .unwrap(),
            ))
            .unwrap();
        let mixed = NurbsCurve::try_new_rational(
            2,
            [1., -0.2, 1.]
                .into_iter()
                .enumerate()
                .map(|(i, w)| {
                    WeightedPoint3::try_new(
                        Point3::try_new(i as f64, (i % 2) as f64, 0.).unwrap(),
                        w,
                    )
                    .unwrap()
                })
                .collect(),
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let invalid = document.add_geometry(Geometry::NurbsCurve(mixed)).unwrap();
        let group = document.add_group(None, [valid, invalid]).unwrap();
        registry.execute(&mut document, "Point 30,0,0").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_objects_direct([valid, invalid], SelectionMode::Replace)
            .unwrap();
        let before = document.objects().cloned().collect::<Vec<_>>();
        let members_before = document.group(group).unwrap().members().collect::<Vec<_>>();
        let redo = document.redo_label().map(str::to_owned);
        assert_eq!(redo.as_deref(), Some("Point"));
        let error = registry
            .execute(&mut document, "ExtrudeCrv 5 DeleteInput=Yes")
            .unwrap_err();
        assert!(matches!(error, CommandError::NativeSmlib(_)), "{error:?}");
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(document.redo_label(), redo.as_deref());
        assert_eq!(
            document.group(group).unwrap().members().collect::<Vec<_>>(),
            members_before
        );
        assert!(document.is_selected(valid) && document.is_selected(invalid));
    }
}
