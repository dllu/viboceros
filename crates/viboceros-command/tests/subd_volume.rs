use viboceros_command::{CommandContext, CommandError, CommandRegistry, ObjectSelectionFilter};
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{
    Point3, SubD, SubDSolidOrientation, Tolerance, VolumeBoundary, VolumeMassProperties,
};
fn sharp_box() -> SubD {
    let points = [
        [0., 0., 0.],
        [4., 0., 0.],
        [4., 3., 0.],
        [0., 3., 0.],
        [0., 0., 5.],
        [4., 0., 5.],
        [4., 3., 5.],
        [0., 3., 5.],
    ]
    .into_iter()
    .map(|p| Point3::try_from(p).unwrap())
    .collect();
    let s = SubD::try_from_polygons(
        points,
        vec![
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ],
    )
    .unwrap();
    let tags = (0..s.edges().len()).map(|i| (i, 2)).collect::<Vec<_>>();
    s.try_with_edge_tags(&tags)
        .unwrap()
        .try_with_vertex_tags(&(0..8).map(|i| (i, 3)).collect::<Vec<_>>())
        .unwrap()
}
#[test]
fn immutable_subd_orientation_and_document_admission_preserve_native_ids_and_source() {
    let outward = sharp_box();
    assert_eq!(outward.solid_orientation(), SubDSolidOrientation::Outward);
    let ids = outward.component_ids().unwrap();
    let bytes = outward.archive().to_vec();
    let inward = outward.reversed().unwrap();
    assert_eq!(inward.solid_orientation(), SubDSolidOrientation::Inward);
    assert_eq!(inward.component_ids().unwrap(), ids);
    assert_eq!(outward.archive(), bytes);
    let mut doc = Document::default();
    let key = doc.add_geometry(Geometry::SubD(inward.clone())).unwrap();
    let Geometry::SubD(stored) = doc.object(key).unwrap().geometry() else {
        panic!()
    };
    assert_eq!(stored.solid_orientation(), SubDSolidOrientation::Outward);
    assert_eq!(stored.faces(), outward.faces());
    assert_eq!(inward.solid_orientation(), SubDSolidOrientation::Inward);
    for s in [&outward, &inward] {
        let mass =
            VolumeMassProperties::from_boundaries(&[VolumeBoundary::SubD(s)], Tolerance::DEFAULT)
                .unwrap();
        assert!(
            (mass.signed_volume().unwrap()
                - if s.solid_orientation() == SubDSolidOrientation::Inward {
                    -60.
                } else {
                    60.
                })
            .abs()
                < 1e-10
        );
        assert!(
            mass.centroid()
                .unwrap()
                .distance_to(Point3::try_new(2., 1.5, 2.5).unwrap())
                .unwrap()
                < 1e-10
        );
    }
}
#[test]
fn analytic_subd_volume_display_units_and_centroid_keep_sources_and_history() {
    let mut doc = Document::default();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut doc, "Units Millimeters Scale=No").unwrap();
    let key = doc.add_geometry(Geometry::SubD(sharp_box())).unwrap();
    doc.clear_history().unwrap();
    r.execute(&mut doc, "Point 40,50,60").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([key], SelectionMode::Replace)
        .unwrap();
    let source = doc.object(key).unwrap().clone();
    assert!(ObjectSelectionFilter::Volume.accepts_object(&source));
    let report = r.execute(&mut doc, "Volume Units=Centimeter").unwrap();
    let n = report
        .split_once("total volume ")
        .unwrap()
        .1
        .split_whitespace()
        .next()
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((n - 0.06).abs() < 1e-12);
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    assert!(doc.is_selected(key));
    r.execute_postselected(&mut doc, "VolumeCentroid", CommandContext::default())
        .unwrap();
    assert_eq!(doc.selected_object_count(), 0);
    assert_eq!(doc.object(key), Some(&source));
    assert_eq!(doc.undo_label(), Some("VolumeCentroid"));
    let Geometry::Point(p) = doc.objects().last().unwrap().geometry() else {
        panic!()
    };
    assert!(
        p.distance_to(Point3::try_new(2., 1.5, 2.5).unwrap())
            .unwrap()
            < 1e-10
    );
    r.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.objects().len(), 1);
    r.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.objects().len(), 2);
}
#[test]
fn open_subd_warning_rejects_implicit_or_declined_queries_without_consuming_redo() {
    let s = sharp_box();
    let mut faces = s.faces().to_vec();
    faces.remove(1);
    let open = SubD::try_from_polygons(s.control_points().to_vec(), faces).unwrap();
    assert_eq!(open.solid_orientation(), SubDSolidOrientation::NotSolid);
    let mut doc = Document::default();
    let source = Geometry::SubD(open.clone());
    let key = doc.add_geometry(source.clone()).unwrap();
    let r = CommandRegistry::with_builtins();
    doc.clear_history().unwrap();
    r.execute(&mut doc, "Point 40,50,60").unwrap();
    r.execute(&mut doc, "Undo").unwrap();
    doc.select_objects_direct([key], SelectionMode::Replace)
        .unwrap();
    for command in ["Volume", "VolumeCentroid"] {
        assert!(matches!(
            r.execute(&mut doc, command),
            Err(CommandError::OpenVolumeConfirmationRequired)
        ));
        assert!(matches!(
            r.execute(&mut doc, &format!("{command} Continue=No")),
            Err(CommandError::OperationDeclined)
        ));
        assert_eq!(doc.object(key).unwrap().geometry(), &source);
        assert_eq!(doc.objects().len(), 1);
        assert!(doc.can_redo());
        assert!(!doc.can_undo());
    }
    let reversed = open.reversed().unwrap();
    let new = doc.add_geometry(Geometry::SubD(reversed.clone())).unwrap();
    assert_eq!(
        doc.object(new).unwrap().geometry(),
        &Geometry::SubD(reversed)
    );
}
