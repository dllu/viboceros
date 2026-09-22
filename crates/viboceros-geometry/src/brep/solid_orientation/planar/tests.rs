use super::*;

fn exact(p: [Real; 3]) -> ExactPoint {
    p.map(rational)
}
fn yz_loop(points: &[[Real; 2]]) -> Vec<ExactPoint> {
    points.iter().map(|p| exact([0., p[0], p[1]])).collect()
}

#[test]
fn exact_winding_distinguishes_concavity_holes_and_boundary_hits() {
    let concave = PolygonFace::new(
        vec![yz_loop(&[
            [0., 0.],
            [4., 0.],
            [4., 1.],
            [1., 1.],
            [1., 4.],
            [0., 4.],
        ])],
        exact([1., 0., 0.]),
        false,
    )
    .unwrap();
    let holed = PolygonFace::new(
        vec![
            yz_loop(&[[0., 0.], [4., 0.], [4., 4.], [0., 4.]]),
            yz_loop(&[[1., 1.], [1., 3.], [3., 3.], [3., 1.]]),
        ],
        exact([1., 0., 0.]),
        false,
    )
    .unwrap();
    for face in [&concave, &holed] {
        assert_eq!(
            ray::contains(face, &exact([0., 0.5, 2.]), &mut 100),
            Some(true)
        );
        assert_eq!(
            ray::contains(face, &exact([0., 2., 2.]), &mut 100),
            Some(false)
        );
        assert_eq!(ray::contains(face, &exact([0., 1., 2.]), &mut 100), None);
        assert_eq!(ray::contains(face, &exact([0., 0.5, 2.]), &mut 0), None);
    }
}

#[test]
fn first_crossing_is_spatial_not_face_table_order_and_degenerate_rays_do_not_guess() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[-1., 1.]; 3], Tolerance::DEFAULT).unwrap();
    let mut faces = cube
        .faces
        .iter()
        .map(|f| face::extract(f, &mut 1000).unwrap())
        .collect::<Vec<_>>();
    for reverse in [false, true] {
        for swap in [false, true] {
            if swap {
                faces.reverse();
            }
            for f in &mut faces {
                f.reversed = reverse;
            }
            let faces = faces.iter().collect::<Vec<_>>();
            assert_eq!(
                ray::first_hit(&faces, &rational(0.), &rational(0.), &mut 1000),
                Some(reverse)
            );
            assert_eq!(
                ray::first_hit(&faces, &rational(2.), &rational(0.), &mut 1000),
                None
            );
            assert_eq!(
                ray::first_hit(&faces, &rational(-1.), &rational(0.), &mut 1000),
                None
            );
            assert_eq!(
                ray::first_hit(&faces, &rational(0.), &rational(0.), &mut 0),
                None
            );
        }
    }
}

#[test]
fn concave_sheared_prisms_and_reflections_do_not_need_axis_aligned_support_faces() {
    let outline = [[0., 0.], [4., 0.], [4., 1.], [1., 1.], [1., 4.], [0., 4.]];
    let transform = |[x, y, z]: [Real; 3], reflected: bool| {
        Point3::try_new(
            (x + 2. * y + 3. * z) * if reflected { -1. } else { 1. },
            3. * x - y + 2. * z,
            2. * x + 3. * y - z,
        )
        .unwrap()
    };
    for reflected in [false, true] {
        let vertices = [0., 2.]
            .into_iter()
            .flat_map(|z| outline.map(|[x, y]| transform([x, y, z], reflected)))
            .collect::<Vec<_>>();
        let mut faces = Vec::new();
        for [a, b, c] in [[0, 1, 3], [1, 2, 3], [0, 3, 5], [3, 4, 5]] {
            faces.push(MeshFace::Triangle([a, c, b]));
            faces.push(MeshFace::Triangle([a + 6, b + 6, c + 6]));
        }
        for i in 0..6 {
            let j = (i + 1) % 6;
            faces.push(MeshFace::Quad([i, j, j + 6, i + 6]));
        }
        for reorder in [false, true] {
            if reorder {
                faces.reverse();
            }
            let mesh =
                TriangleMesh::try_new_faces(vertices.clone(), faces.clone(), Tolerance::DEFAULT)
                    .unwrap();
            for trimmed in [false, true] {
                let brep = Brep::try_from_mesh(&mesh, trimmed, Tolerance::DEFAULT).unwrap();
                assert!(brep.is_solid());
                assert_eq!(
                    brep.solid_orientation().unwrap(),
                    if reflected {
                        BrepSolidOrientation::Inward
                    } else {
                        BrepSolidOrientation::Outward
                    }
                );
                assert_eq!(
                    brep.reversed().solid_orientation().unwrap(),
                    if reflected {
                        BrepSolidOrientation::Outward
                    } else {
                        BrepSolidOrientation::Inward
                    }
                );
            }
        }
    }
}

#[test]
fn extraction_accepts_exact_rational_polyline_images_but_not_nearly_planar_or_gapped_faces() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let cube = Brep::try_box(frame, [[-1., 1.]; 3], Tolerance::DEFAULT).unwrap();
    let original = &cube.faces[0];
    let expected = face::extract(original, &mut 1000).unwrap();
    let axis = expected.normal.iter().position(|n| !n.is_zero()).unwrap();
    for warp in [false, true] {
        let s = &original.surface;
        let mut controls = s.control_points().to_vec();
        for c in &mut controls {
            *c = WeightedPoint3::try_new(c.point(), -7.).unwrap();
        }
        if warp {
            let mut p = controls[3].point().to_array();
            p[axis] += 1e-14;
            controls[3] = WeightedPoint3::try_new(Point3::try_from(p).unwrap(), -7.).unwrap();
        }
        let mut f = original.clone();
        f.surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            controls,
            s.knots_u().to_vec(),
            s.knots_v().to_vec(),
        )
        .unwrap();
        assert!(f.surface.plane(Tolerance::DEFAULT).unwrap().is_some());
        assert_eq!(face::extract(&f, &mut 1000).is_some(), !warp);
    }
    for kind in ["positive", "negative", "mixed", "gap", "jump"] {
        let mut f = original.clone();
        let trim = &mut f.loops[0].trims[0];
        let a = trim.curve.control_points()[0].point();
        let b = trim.curve.control_points()[1].point();
        let midpoint = Point2::try_new((a.x() + b.x()) * 0.5, (a.y() + b.y()) * 0.5).unwrap();
        let mut points = vec![a, midpoint, b];
        let mut knots = vec![-7., -7., 3., 12., 12.];
        if kind == "jump" {
            points.insert(2, midpoint);
            knots.insert(3, 3.);
        }
        if kind == "gap" {
            points[2] = Point2::try_new(
                b.x() + (a.x() - b.x()) * 1e-14,
                b.y() + (a.y() - b.y()) * 1e-14,
            )
            .unwrap();
        }
        trim.curve = NurbsCurve2::try_new_rational(
            1,
            points
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    WeightedPoint2::try_new(
                        p,
                        if kind == "negative" || kind == "mixed" && i == 1 {
                            -(i as Real + 1.)
                        } else {
                            i as Real + 1.
                        },
                    )
                    .unwrap()
                })
                .collect(),
            knots,
        )
        .unwrap();
        assert_eq!(
            face::extract(&f, &mut 1000).is_some(),
            matches!(kind, "positive" | "negative"),
            "{kind}"
        );
    }
}

#[test]
fn real_affine_trimmed_face_retains_holes_through_exact_extraction() {
    let curve = |points: &[[Real; 2]]| {
        crate::Polyline3::try_new(
            points
                .iter()
                .map(|p| Point3::try_new(p[0], p[1], 0.).unwrap())
                .collect(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let outer = curve(&[[0., 0.], [4., 0.], [4., 4.], [0., 4.], [0., 0.]]);
    let hole = curve(&[[1., 1.], [3., 1.], [3., 3.], [1., 3.], [1., 1.]]);
    let brep = Brep::try_planar_face_with_holes(&outer, &[hole], Tolerance::DEFAULT).unwrap();
    let face = face::extract(&brep.faces[0], &mut 1000).unwrap();
    assert_eq!(face.loops.len(), 2);
    assert_eq!(
        ray::contains(&face, &exact([0.5, 2., 0.]), &mut 100),
        Some(true)
    );
    assert_eq!(
        ray::contains(&face, &exact([2., 2., 0.]), &mut 100),
        Some(false)
    );
    assert_eq!(ray::contains(&face, &exact([1., 2., 0.]), &mut 100), None);
}
