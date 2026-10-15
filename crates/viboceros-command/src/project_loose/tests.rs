use super::*;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn plane() -> Geometry {
    Geometry::NurbsSurface(
        NurbsSurface::try_bilinear([
            p(0., 0., 0.),
            p(10., 0., 0.),
            p(10., 10., 0.),
            p(0., 10., 0.),
        ])
        .unwrap(),
    )
}
fn setup(source: Geometry, target: Geometry) -> (Document, CommandRegistry, ObjectId, ObjectId) {
    let mut d = Document::default();
    let a = d.add_geometry(source).unwrap();
    let b = d.add_geometry(target).unwrap();
    d.clear_history().unwrap();
    (d, CommandRegistry::with_builtins(), a, b)
}
#[test]
fn loose_moves_edit_points_without_clipping_between_them() {
    for command in ["Project", "Pull"] {
        let source = NurbsCurve::try_new(
            3,
            vec![
                p(5., 0., 3.),
                p(5., 100. / 3., 3.),
                p(5., -70. / 3., 3.),
                p(5., 10., 3.),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let (mut d, r, a, b) = setup(Geometry::NurbsCurve(source.clone()), plane());
        r.execute(
            &mut d,
            &format!("{command} Sources={a} Targets={b} Loose=Yes"),
        )
        .unwrap();
        let output = d.selected_objects().next().unwrap();
        let Geometry::NurbsCurve(c) = output.geometry() else {
            panic!()
        };
        assert_eq!(c.knots(), source.knots());
        assert_eq!(c.control_points().len(), 4);
        assert!(c.evaluate(0.25).unwrap().y() > 10.5);
        for e in c.edit_points().unwrap() {
            assert!(e.point.z().abs() < 1e-8);
            assert!(e.point.y() >= -1e-8 && e.point.y() <= 10. + 1e-8);
        }
    }
}
#[test]
fn a_missed_edit_point_preserves_input_even_with_delete_input() {
    for command in ["Project", "Pull"] {
        let (mut d, r, a, b) = setup(
            Geometry::Line(
                LineSegment::try_new(p(-1., 5., 3.), p(9., 5., 3.), Tolerance::DEFAULT).unwrap(),
            ),
            plane(),
        );
        r.execute(
            &mut d,
            &format!("{command} Sources={a} Targets={b} Loose=Yes DeleteInput=Yes"),
        )
        .unwrap();
        assert_eq!(d.objects().len(), 2);
        assert!(d.object(a).is_some());
        assert!(d.undo_label().is_none());
    }
}
#[test]
fn mesh_pull_returns_original_curve_structure_and_projects_its_edit_points() {
    let mesh = TriangleMesh::try_new(
        vec![
            p(-4., 0., 0.),
            p(0., -4., 0.),
            p(0., 4., 0.),
            p(4., 0., -4.),
        ],
        vec![[0, 1, 2], [1, 3, 2]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = NurbsCurve::try_new(
        3,
        vec![p(-1., 0., 2.), p(0., 0., 2.), p(3., 0., 2.), p(4., 0., 2.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let expected = source
        .edit_points()
        .unwrap()
        .iter()
        .map(|e| {
            viboceros_geometry::MeshProjectionIndex::new(&mesh)
                .unwrap()
                .closest_point(e.point)
        })
        .collect::<Vec<_>>();
    let (mut d, r, a, b) = setup(Geometry::NurbsCurve(source.clone()), Geometry::Mesh(mesh));
    r.execute(
        &mut d,
        &format!("Pull Sources={a} Targets={b} Loose=Yes DeleteInput=Yes OutputLayer=TargetObject"),
    )
    .unwrap();
    let Geometry::NurbsCurve(c) = d.selected_objects().next().unwrap().geometry() else {
        panic!()
    };
    assert_eq!(c.knots(), source.knots());
    assert_eq!(c.control_points().len(), 4);
    for (actual, expected) in c.edit_points().unwrap().iter().zip(expected) {
        assert!(
            actual
                .point
                .distance_to(expected.unwrap().unwrap())
                .unwrap()
                < 1e-8
        );
    }
    assert!(d.object(a).is_none());
    assert_eq!(d.undo_label(), Some("Pull"));
    d.undo().unwrap();
    assert!(d.object(a).is_some());
    d.redo().unwrap();
    assert!(d.object(a).is_none());
}
#[test]
fn unsupported_later_source_rolls_back_previously_staged_loose_output() {
    let (mut d, r, a, b) = setup(
        Geometry::Line(
            LineSegment::try_new(p(1., 5., 3.), p(9., 5., 3.), Tolerance::DEFAULT).unwrap(),
        ),
        plane(),
    );
    let bad = NurbsCurve::try_new(
        2,
        vec![p(1., 2., 3.), p(2., 3., 3.), p(3., 2., 3.)],
        vec![-2., -1., 0., 1., 2., 3.],
    )
    .unwrap();
    let c = d.add_geometry(Geometry::NurbsCurve(bad)).unwrap();
    d.clear_history().unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    assert!(
        r.execute(
            &mut d,
            &format!("Project Sources={a},{c} Targets={b} Loose=Yes DeleteInput=Yes")
        )
        .is_err()
    );
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert!(d.undo_label().is_none());
}

#[test]
fn mesh_projection_keeps_two_complete_sheets_and_declines_facing_changes() {
    let vertices = vec![
        p(0., 0., 0.),
        p(10., 0., 0.),
        p(0., 10., 0.),
        p(0., 0., -2.),
        p(10., 0., -2.),
        p(0., 10., -2.),
    ];
    let mesh =
        TriangleMesh::try_new(vertices, vec![[0, 1, 2], [3, 4, 5]], Tolerance::DEFAULT).unwrap();
    let (mut d, r, a, b) = setup(
        Geometry::Line(
            LineSegment::try_new(p(1., 2., 3.), p(5., 2., 3.), Tolerance::DEFAULT).unwrap(),
        ),
        Geometry::Mesh(mesh),
    );
    r.execute(
        &mut d,
        &format!("Project Sources={a} Targets={b} Loose=Yes"),
    )
    .unwrap();
    assert_eq!(d.selected_object_count(), 2);
    let mesh = TriangleMesh::try_new(
        vec![p(-4., 0., 0.), p(0., -4., 0.), p(0., 4., 0.), p(4., 0., 0.)],
        vec![[0, 1, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, r, a, b) = setup(
        Geometry::Line(
            LineSegment::try_new(p(-1., 0., 2.), p(1., 0., 2.), Tolerance::DEFAULT).unwrap(),
        ),
        Geometry::Mesh(mesh),
    );
    r.execute(
        &mut d,
        &format!("Project Sources={a} Targets={b} Loose=Yes DeleteInput=Yes"),
    )
    .unwrap();
    assert_eq!(d.objects().len(), 2);
    assert!(d.object(a).is_some());
}

#[test]
fn a_greville_point_in_a_trim_hole_misses_the_complete_loose_curve() {
    let rect = |lo, hi| {
        viboceros_geometry::Polyline3::try_new(
            vec![
                p(lo, lo, 0.),
                p(hi, lo, 0.),
                p(hi, hi, 0.),
                p(lo, hi, 0.),
                p(lo, lo, 0.),
            ],
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let target =
        Brep::try_planar_face_with_holes(&rect(0., 10.), &[rect(4., 6.)], Tolerance::DEFAULT)
            .unwrap();
    for command in ["Project", "Pull"] {
        let source = NurbsCurve::try_new(
            2,
            vec![p(1., 5., 3.), p(5., 5., 3.), p(9., 5., 3.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let (mut d, r, a, b) = setup(Geometry::NurbsCurve(source), Geometry::Brep(target.clone()));
        r.execute(
            &mut d,
            &format!("{command} Sources={a} Targets={b} Loose=Yes DeleteInput=Yes"),
        )
        .unwrap();
        assert_eq!(d.objects().len(), 2);
        assert!(d.object(a).is_some());
    }
}
