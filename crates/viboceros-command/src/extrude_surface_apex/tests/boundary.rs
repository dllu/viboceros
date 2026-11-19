use super::*;

fn double_section_cutter(left: f64, right: f64, center_y: f64, reverse: bool) -> NurbsSurface {
    let width = right - left;
    let points = [-1., 4.]
        .into_iter()
        .flat_map(|y| {
            let height = |x| (x - 2.8) * (x - 4.4) + (y - center_y) * (0.08 + 0.01 * (x - 3.6));
            let slope = 2. * left - 7.2 + 0.01 * (y - center_y);
            let mut row = [
                p(left, y, height(left)),
                p(left + 0.5 * width, y, height(left) + 0.5 * width * slope),
                p(right, y, height(right)),
            ];
            if reverse {
                row.reverse();
            }
            row
        })
        .collect();
    NurbsSurface::try_new(
        2,
        1,
        3,
        2,
        points,
        vec![0., 0., 0., 1., 1., 1.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

fn document_with_folded_cutter(
    left: f64,
    right: f64,
    center_y: f64,
    reverse: bool,
) -> (Document, ObjectId, ObjectId) {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let mut doc = Document::new(tolerance);
    let source = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
    let boundary = doc
        .add_geometry(Geometry::NurbsSurface(double_section_cutter(
            left, right, center_y, reverse,
        )))
        .unwrap();
    doc.clear_history().unwrap();
    (doc, source, boundary)
}

#[test]
fn surface_apex_ignores_flat_sections_that_miss_the_sweep() {
    for (left, right, center_y, native_vertices) in [
        (
            2.4,
            4.4,
            2.65,
            [
                p(2.700271288843304, 0.262601733316239, 0.),
                p(2.716678053757396, 0.679169513439349, 0.),
                p(2.802216350315363, 2.6991688686317383, 0.),
                p(2.784604640488288, 2.3038488398779284, 0.),
            ],
        ),
        (
            2.8,
            4.8,
            2.9,
            [
                p(4.4933112489671085, 1.1233278122417767, 0.),
                p(4.50256699425501, 0.9384626228456288, 0.),
                p(4.443924243644748, 2.0835284086332195, 0.),
                p(4.454250176083497, 1.886437455979126, 0.),
            ],
        ),
    ] {
        let (mut doc, source, boundary) = document_with_folded_cutter(left, right, center_y, false);
        let registry = CommandRegistry::with_builtins();
        let command = format!(
            "ExtrudeSrfToPoint 6,1.5,0 ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        );
        registry.execute(&mut doc, &command).unwrap();
        assert_cut_rim(&doc, &native_vertices, false);
        assert!(doc.object(source).is_none());
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.object(source).is_some());
    }
}

#[test]
fn surface_apex_raised_sweep_selects_the_nearest_complete_section() {
    let near_at_quarter = [
        p(2.721009359342296, 0.27037850975336075, 0.04506308495889353),
        p(2.6994134512323598, 0.6748533628080899, 0.11247556046801499),
        p(2.821986034587447, 2.6917552370297075, 0.05137412716171534),
        p(2.7649788582261645, 2.308755285443459, 0.1152074524260902),
    ];
    for (height, right, reverse, native_vertices) in [
        (0.25, 4.8, false, near_at_quarter),
        (0.25, 4.8, true, near_at_quarter),
        (0.25, 4.4, false, near_at_quarter),
        (
            0.01,
            4.8,
            false,
            [
                p(
                    2.746148567965311,
                    0.27980571298699175,
                    0.0018653714199132255,
                ),
                p(2.761798734069421, 0.6904496835173553, 0.004602997890115729),
                p(2.8539735517776847, 2.679759918083368, 0.0021349338794442044),
                p(2.833540098788743, 2.2916149753028145, 0.0047225668313145225),
            ],
        ),
    ] {
        let (mut doc, source, boundary) = document_with_folded_cutter(2.4, right, 1.5, reverse);
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint 6,1.5,{height} ToBoundary={boundary} FaceRefs={source}:0"
                ),
            )
            .unwrap();
        assert_cut_rim(&doc, &native_vertices, true);
        assert!(doc.object(source).is_some());
    }
}

#[test]
fn surface_apex_rejects_ambiguous_flat_cuts_and_nearer_partial_sections_atomically() {
    for (height, left, reverse) in [(0., 2.4, false), (0., 2.4, true), (0.25, 2.8, false)] {
        let (mut doc, source, boundary) = document_with_folded_cutter(left, 4.8, 1.5, reverse);
        let before = format!("{doc:?}");
        let command = format!(
            "ExtrudeSrfToPoint 6,1.5,{height} ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        );
        assert!(
            CommandRegistry::with_builtins()
                .execute(&mut doc, &command)
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
    }
}

fn assert_cut_rim(doc: &Document, expected: &[Point3], raised: bool) {
    let Geometry::Brep(cut) = doc.selected_objects().next().unwrap().geometry() else {
        panic!("expected a boundary-cut B-rep")
    };
    assert_eq!(
        [cut.faces().len(), cut.edges().len(), cut.vertices().len()],
        [6, 12, 8]
    );
    assert!(cut.is_solid());
    let volume = cut.signed_volume(doc.tolerance()).unwrap();
    assert_eq!(volume > 0., raised);
    for &point in expected {
        assert!(
            cut.vertices()
                .iter()
                .any(|v| v.point().is_near(point, doc.tolerance())),
            "{point:?}"
        );
    }
}

#[test]
fn surface_apex_planar_boundaries_cut_flat_and_shallow_sweeps() {
    for (height, normal, native_vertices) in [
        (
            0.,
            [1., -0.25, 0.4],
            [
                p(3.333333333333333, 0.833333333333333, 0.),
                p(3.2413793103448265, 0.4655172413793099, 0.),
                p(3.714285714285714, 2.3571428571428577, 0.),
                p(3.6470588235294112, 2.0882352941176463, 0.),
            ],
        ),
        (
            1e-8,
            [1., 0., 0.],
            [
                p(3.5, 0.875, 5.8333333333333335e-9),
                p(3.5, 0.5625, 3.75e-9),
                p(3.5, 2.4375, 3.75e-9),
                p(3.5, 2.125, 5.8333333333333335e-9),
            ],
        ),
        (
            0.,
            [1., 0., 0.],
            [
                p(3.5, 0.875, 0.),
                p(3.5, 0.5625, 0.),
                p(3.5, 2.4375, 0.),
                p(3.5, 2.125, 0.),
            ],
        ),
    ] {
        let mut doc = Document::default();
        let registry = CommandRegistry::with_builtins();
        let t = doc.tolerance();
        let source = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
        let normal = UnitVector3::try_new(normal[0], normal[1], normal[2], t).unwrap();
        let frame = Frame3::try_from_unit_normal(p(3.5, 1.5, 0.), normal).unwrap();
        let surface = |extent: f64| {
            let corners = [
                [-extent, -extent],
                [extent, -extent],
                [extent, extent],
                [-extent, extent],
            ]
            .map(|[x, y]| frame.point_at([x, y, 0.]).unwrap());
            Geometry::NurbsSurface(NurbsSurface::try_bilinear(corners).unwrap())
        };
        let boundary = doc.add_geometry(surface(10.)).unwrap();
        let narrow = doc.add_geometry(surface(0.3)).unwrap();
        doc.clear_history().unwrap();
        let before = format!("{doc:?}");
        assert!(
            registry
                .execute(
                    &mut doc,
                    &format!("ExtrudeSrfToPoint 6,1.5,{height} FaceRefs={source}:0 ToBoundary={narrow} DeleteInput=Yes"),
                )
                .is_err()
        );
        assert_eq!(format!("{doc:?}"), before);
        registry
            .execute(
                &mut doc,
                &format!("ExtrudeSrfToPoint 6,1.5,{height} FaceRefs={source}:0 ToBoundary={boundary} DeleteInput=Yes"),
            )
            .unwrap();
        let Geometry::Brep(brep) = doc.selected_objects().next().unwrap().geometry() else {
            panic!("expected truncated sweep")
        };
        assert_eq!(
            [
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ],
            [6, 12, 8]
        );
        assert!(brep.is_closed());
        for point in native_vertices {
            assert!(
                brep.vertices()
                    .iter()
                    .any(|v| v.point().distance_to(point).unwrap() < 1e-12),
                "{point:?}"
            );
        }
        assert!(doc.object(source).is_none());
        assert!(doc.object(boundary).is_some());
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.object(source).is_some());
        registry.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn surface_apex_flat_boundary_keeps_rational_source_loops_and_checks_picked_holes() {
    let mut doc = Document::default();
    let t = doc.tolerance();
    let circle = |center, radius, normal| {
        Circle3::try_new(center, radius, normal, t)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let source_normal = UnitVector3::try_new(0., 0., 1., t).unwrap();
    let outer = circle(p(0., 0., 0.), 3., source_normal);
    let inner = circle(p(0., 0., 0.), 1., source_normal);
    let base = Brep::try_planar_face_with_holes(&outer, &[inner], t).unwrap();
    let source = doc.add_geometry(Geometry::Brep(base)).unwrap();
    let normal = UnitVector3::try_new(1., -0.2, 0.6, t).unwrap();
    let frame = Frame3::try_from_unit_normal(p(4., 0., 0.), normal).unwrap();
    let outer = circle(frame.origin(), 10., normal);
    let make_cutter = |hole_center| {
        let hole = circle(hole_center, 0.2, normal);
        Geometry::Brep(Brep::try_planar_face_with_holes(&outer, &[hole], t).unwrap())
    };
    let crossing = doc.add_geometry(make_cutter(frame.origin())).unwrap();
    let clear = doc
        .add_geometry(make_cutter(frame.point_at([0., 5., 0.]).unwrap()))
        .unwrap();
    doc.clear_history().unwrap();
    let before = format!("{doc:?}");
    let registry = CommandRegistry::with_builtins();
    assert!(
        registry
            .execute(
                &mut doc,
                &format!("ExtrudeSrfToPoint 8,0,0 FaceRefs={source}:0 ToBoundary={crossing}"),
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    registry
        .execute(
            &mut doc,
            &format!("ExtrudeSrfToPoint 8,0,0 FaceRefs={source}:0 ToBoundary={clear}"),
        )
        .unwrap();
    let Geometry::Brep(brep) = doc.selected_objects().next().unwrap().geometry() else {
        panic!("expected a truncated rational sweep")
    };
    assert!(brep.is_closed());
    assert_eq!(brep.faces().len(), 4);
    assert_eq!(brep.faces()[0].loops().len(), 2);
    assert_eq!(brep.faces()[3].loops().len(), 2);
    assert!(brep.faces()[1..3].iter().all(|face| {
        face.surface()
            .control_points()
            .iter()
            .any(|control| control.weight() != 1.)
    }));
}

#[test]
fn surface_apex_flat_sweeps_follow_warped_boundary_sections() {
    let cases = [
        (
            [
                [3.1, -4., -4.],
                [4.6, 7., -3.],
                [3.9, 7., 5.],
                [3.8, -4., 3.],
            ],
            [
                p(3.7877185737886157, 0.6703944651707304, 0.),
                p(3.8085141977875874, 0.9521285494468964, 0.),
                p(3.911651924821384, 2.2831305281919816, 0.),
                p(3.891203240771333, 2.0271991898071664, 0.),
            ],
        ),
        (
            [
                [2.8, -4., -4.],
                [4.8, 7., -3.],
                [3.3, 7., 5.],
                [4.5, -4., 3.],
            ],
            [
                p(3.8138891844508818, 0.6802084441690806, 0.),
                p(3.82412509014911, 0.9560312725372776, 0.),
                p(3.885023721591392, 2.2931161044032278, 0.),
                p(3.8716893964377883, 2.0320776508905523, 0.),
            ],
        ),
    ];
    for ((corners, native_rim), tilted) in
        cases.into_iter().zip([false; 2]).chain([(cases[1], true)])
    {
        let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
        let mut doc = Document::new(tolerance);
        let placed = |point: Point3| {
            let [x, y, z] = point.to_array();
            if tilted {
                p(
                    1024. + x,
                    -32. + 0.6 * y - 0.8 * z,
                    256. + 0.8 * y + 0.6 * z,
                )
            } else {
                point
            }
        };
        let surface = |corners: [[f64; 3]; 4]| {
            NurbsSurface::try_bilinear(corners.map(|[x, y, z]| placed(p(x, y, z)))).unwrap()
        };
        let source = doc
            .add_geometry(Geometry::NurbsSurface(surface([
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 3., 0.],
                [0., 3., 0.],
            ])))
            .unwrap();
        let cutter = surface(corners);
        let boundary = doc
            .add_geometry(Geometry::NurbsSurface(cutter.clone()))
            .unwrap();
        let narrow = doc
            .add_geometry(Geometry::NurbsSurface(surface([
                [3.4, 1.4, -4.],
                [3.6, 1.6, -3.],
                [3.9, 1.6, 5.],
                [3.8, 1.4, 3.],
            ])))
            .unwrap();
        doc.clear_history().unwrap();
        let before = format!("{doc:?}");
        let registry = CommandRegistry::with_builtins();
        let [x, y, z] = placed(p(6., 1.5, 0.)).to_array();
        assert!(registry.execute(&mut doc,&format!(
            "ExtrudeSrfToPoint {x},{y},{z} ToBoundary={narrow} FaceRefs={source}:0 DeleteInput=Yes"
        )).is_err());
        assert_eq!(format!("{doc:?}"), before);
        registry.execute(&mut doc,&format!(
            "ExtrudeSrfToPoint {x},{y},{z} ToBoundary={boundary} FaceRefs={source}:0 DeleteInput=Yes"
        )).unwrap();
        let Geometry::Brep(cut) = doc.selected_objects().next().unwrap().geometry() else {
            panic!("expected boundary-cut B-rep");
        };
        assert_eq!(
            [cut.faces().len(), cut.edges().len(), cut.vertices().len()],
            [6, 12, 8]
        );
        assert!(cut.is_closed());
        assert!(cut.signed_volume(tolerance).unwrap().abs() < 1e-10);
        for point in native_rim.map(placed) {
            assert!(
                cut.vertices()
                    .iter()
                    .any(|v| v.point().is_near(point, tolerance)),
                "{point:?}"
            );
        }
        let mut rim_count = 0;
        for edge in cut.edges().iter().filter(|edge| {
            edge.vertices()
                .iter()
                .all(|&v| cut.vertices()[v].point().x() > placed(p(3., 0., 0.)).x())
        }) {
            rim_count += 1;
            for fraction in [0., 0.25, 0.5, 0.75, 1.] {
                let curve = edge.curve();
                let point = curve
                    .evaluate(curve.parameter_at(fraction).unwrap())
                    .unwrap();
                let (u, v) = cutter.closest_parameters(point, tolerance).unwrap();
                assert!(
                    cutter.evaluate(u, v).unwrap().is_near(point, tolerance),
                    "{point:?}"
                );
            }
        }
        assert_eq!(rim_count, 4);
        assert!(doc.object(source).is_none());
        assert!(doc.object(boundary).is_some());
        registry.execute(&mut doc, "Undo").unwrap();
        assert!(doc.object(source).is_some());
    }
}

#[test]
fn surface_apex_later_boundary_miss_preserves_batch_and_redo() {
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let s = doc.add_geometry(planar_rectangle(0., 0.)).unwrap();
    let other = doc.add_geometry(planar_rectangle(20., 0.)).unwrap();
    let boundary = doc
        .add_geometry(Geometry::NurbsSurface(
            NurbsSurface::try_bilinear([
                p(-1., -1., 6.),
                p(3., -1., 6.),
                p(3., 4., 6.),
                p(-1., 4., 6.),
            ])
            .unwrap(),
        ))
        .unwrap();
    registry.execute(&mut doc, "Point 50,50,50").unwrap();
    registry.execute(&mut doc, "Undo").unwrap();
    let before = format!("{doc:?}");
    assert!(
        registry
            .execute(
                &mut doc,
                &format!(
                    "ExtrudeSrfToPoint 1,1.5,12 ToBoundary={boundary} FaceRefs={s}:0,{other}:0 DeleteInput=Yes"
                )
            )
            .is_err()
    );
    assert_eq!(format!("{doc:?}"), before);
    assert!(doc.can_redo());
}
