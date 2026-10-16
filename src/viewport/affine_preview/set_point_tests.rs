use super::*;
use viboceros_command::set_point::SetPointOptions;
use viboceros_document::{ControlPointId, CopyGroupPolicy};

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn options() -> SetPointOptions {
    SetPointOptions {
        axes: [false, false, true],
        ..Default::default()
    }
}

#[test]
fn set_point_cplane_preview_uses_the_destination_view_and_matches_command_execution() {
    for (kind, expected) in [
        (ViewKind::Top, p(1., 8., 3.)),
        (ViewKind::Front, p(1., 2., 9.)),
    ] {
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
        doc.select_objects_direct([id], SelectionMode::Add).unwrap();
        let sources = [id];
        let view = Viewport::new(kind);
        let options = SetPointOptions {
            axes: [false, true, false],
            align_to_cplane: true,
            ..Default::default()
        };
        let preview = AffinePreview {
            sources: &sources,
            grips: &[],
            definition: PointTransform::SetPoint(options).into(),
            frame: None,
            copy: false,
            last_transform: None,
            rigid_layout: None,
        };
        let before = format!("{doc:?}");
        let map = view
            .resolve_affine_preview(preview, Some(p(7., 8., 9.)), &doc)
            .0
            .unwrap()
            .transform;
        assert!(
            map.transform_point(p(1., 2., 3.))
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-10
        );
        assert_eq!(format!("{doc:?}"), before);
        viboceros_command::CommandRegistry::with_builtins()
            .execute_in_context(
                &mut doc,
                "SetPt 7,8,9 XSet=No YSet=Yes ZSet=No Alignment=CPlane",
                viboceros_command::CommandContext {
                    construction_plane: view.construction_plane(),
                },
            )
            .unwrap();
        let Geometry::Point(result) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(result.distance_to(expected).unwrap() < 1e-10);
    }
}

#[test]
fn set_point_singular_maps_accept_points_and_reject_a_whole_invalid_batch() {
    let mut doc = Document::default();
    let point = doc.add_geometry(Geometry::Point(p(1., 2., 3.))).unwrap();
    let line = doc
        .add_geometry(Geometry::Line(
            viboceros_geometry::LineSegment::try_new(p(0., 0., 0.), p(1., 0., 1.), doc.tolerance())
                .unwrap(),
        ))
        .unwrap();
    let sources = [point, line];
    let view = Viewport::new(ViewKind::Top);
    let preview = AffinePreview {
        sources: &sources,
        grips: &[],
        definition: PointTransform::SetPoint(SetPointOptions::default()).into(),
        frame: None,
        copy: false,
        last_transform: None,
        rigid_layout: None,
    };
    let before = format!("{doc:?}");
    let (objects, update) = view.resolve_affine_preview(preview, Some(p(9., 8., 7.)), &doc);
    assert!(objects.is_none());
    assert_eq!(update, Some(None));
    let (objects, update) = view.resolve_affine_preview(
        AffinePreview {
            sources: &sources[..1],
            ..preview
        },
        Some(p(9., 8., 7.)),
        &doc,
    );
    let map = objects.unwrap().transform;
    assert!(map.orientation_reversing().is_err());
    assert_eq!(map.transform_point(p(1., 2., 3.)).unwrap(), p(9., 8., 7.));
    assert_eq!(update, Some(Some(map)));
    assert_eq!(format!("{doc:?}"), before);
}

#[test]
fn set_point_edit_preview_retains_last_target_when_reconstruction_overflows() {
    let mut doc = Document::default();
    let source =
        NurbsCurve::try_clamped_uniform(2, vec![p(0., 0., 0.), p(1., 2., 0.), p(2., 0., 0.)])
            .unwrap();
    let id = doc.add_geometry(Geometry::NurbsCurve(source)).unwrap();
    doc.enable_edit_points([id]).unwrap();
    let grips = [ControlPointId {
        object: id,
        index: 1,
    }];
    let view = Viewport::new(ViewKind::Top);
    let preview = AffinePreview {
        sources: &[],
        grips: &grips,
        definition: PointTransform::SetPoint(options()).into(),
        frame: None,
        copy: false,
        last_transform: None,
        rigid_layout: None,
    };
    let before = format!("{doc:?}");
    let valid = view
        .resolve_affine_preview(preview, Some(p(0., 0., 5.)), &doc)
        .0
        .unwrap()
        .transform;
    let preview = AffinePreview {
        last_transform: Some(valid),
        ..preview
    };
    let (held, update) = view.resolve_affine_preview(preview, None, &doc);
    assert_eq!(held.unwrap().transform, valid);
    assert_eq!(update, None);
    let target = p(0., 0., 1e308);
    let (held, update) = view.resolve_affine_preview(preview, Some(target), &doc);
    assert_eq!(held.unwrap().transform, valid);
    assert_eq!(update, Some(Some(valid)));
    let bad = options()
        .transform_at(target, view.construction_plane())
        .unwrap();
    assert!(
        doc.transform_objects_and_grips([], grips, bad, false, CopyGroupPolicy::Preserve)
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
}
