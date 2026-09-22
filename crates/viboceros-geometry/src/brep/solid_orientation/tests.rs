use super::*;
use BrepSolidOrientation::*;

fn point(p: [Real; 3]) -> Point3 {
    Point3::try_from(p).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_normal(
        point([0.; 3]),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn cube(center: [Real; 3], size: Real) -> Brep {
    Brep::try_box(
        frame(),
        center.map(|c| [c - size, c + size]),
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn combine(parts: Vec<Brep>) -> Brep {
    Brep::try_combine(parts, Tolerance::DEFAULT).unwrap()
}

#[test]
fn boxes_distinguish_spatial_sense_topological_solidity_and_inconsistent_faces() {
    let outward = cube([0.; 3], 1.);
    let before = outward.clone();
    assert_eq!(outward.solid_orientation().unwrap(), Outward);
    assert_eq!(outward.reversed().solid_orientation().unwrap(), Inward);
    assert_eq!(outward, before);
    assert_eq!(
        outward
            .sub_brep(&[0, 1, 2, 3, 4], Tolerance::DEFAULT)
            .unwrap()
            .solid_orientation()
            .unwrap(),
        NotSolid
    );
    let mut inconsistent = outward.clone();
    inconsistent.faces[0].reversed = !inconsistent.faces[0].reversed;
    assert!(inconsistent.is_closed());
    assert_eq!(inconsistent.solid_orientation().unwrap(), NotSolid);
    assert_eq!(outward.solid_orientation_with_budget(0).unwrap(), Unknown);
}

#[test]
fn disjoint_shells_follow_minimum_x_not_signed_volume_size_or_table_order() {
    for axis in 0..3 {
        for low_size in [1., 2.] {
            for reverse in [false, true] {
                for reverse_order in [false, true] {
                    let mut low = [0.; 3];
                    low[axis] = -10.;
                    let mut high = [0.; 3];
                    high[axis] = 10.;
                    let low = cube(low, low_size);
                    let high = cube(high, 3. - low_size);
                    let mut parts = if reverse {
                        vec![low.reversed(), high]
                    } else {
                        vec![low, high.reversed()]
                    };
                    if reverse_order {
                        parts.reverse();
                    }
                    let brep = combine(parts);
                    let low_selected = axis == 0 || low_size == 2.;
                    assert_eq!(
                        brep.solid_orientation().unwrap(),
                        if reverse == low_selected {
                            Inward
                        } else {
                            Outward
                        }
                    );
                }
            }
        }
    }
    let opposed = combine(vec![
        cube([-10., 0., 0.], 1.),
        cube([10., 0., 0.], 2.).reversed(),
    ]);
    assert!(opposed.signed_volume(Tolerance::DEFAULT).unwrap() < -55.);
    assert_eq!(opposed.solid_orientation().unwrap(), Outward);
}

#[test]
fn nested_shells_use_the_outer_support_and_conflicting_ties_remain_unknown() {
    for inner_reversed in [false, true] {
        for outer_reversed in [false, true] {
            for swap in [false, true] {
                let outer = cube([0.; 3], 4.);
                let inner = cube([0.; 3], 1.);
                let mut parts = vec![
                    if outer_reversed {
                        outer.reversed()
                    } else {
                        outer
                    },
                    if inner_reversed {
                        inner.reversed()
                    } else {
                        inner
                    },
                ];
                if swap {
                    parts.reverse();
                }
                assert_eq!(
                    combine(parts).solid_orientation().unwrap(),
                    if outer_reversed { Inward } else { Outward }
                );
            }
        }
    }
    let b = cube([0.; 3], 1.);
    assert_eq!(
        combine(vec![b.clone(), b.clone()])
            .solid_orientation()
            .unwrap(),
        Outward
    );
    for parts in [vec![b.clone(), b.reversed()], vec![b.reversed(), b.clone()]] {
        assert_eq!(combine(parts).solid_orientation().unwrap(), Unknown);
    }
}

#[test]
fn natural_spheres_and_capped_cylinders_have_regular_exact_support_witnesses() {
    for surface in [
        NurbsSurface::try_sphere(frame(), 2.).unwrap(),
        NurbsSurface::try_cylinder(frame(), 2., -1., 3.).unwrap(),
    ] {
        let face = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
        let closed = if face.is_solid() {
            face
        } else {
            face.try_cap_planar_holes(Tolerance::DEFAULT)
                .unwrap()
                .unwrap()
        };
        assert!(closed.is_solid());
        assert_eq!(closed.solid_orientation().unwrap(), Outward);
        assert_eq!(closed.reversed().solid_orientation().unwrap(), Inward);
    }
}

#[test]
fn a_corner_normal_component_is_not_an_orientation_certificate() {
    // All non-origin vertices have positive X. An outward face through the
    // minimum-X vertex nevertheless has normal C×B=(15,-2,7), pointing +X.
    let vertices = [[0., 0., 0.], [1., 10., 0.], [1., 11., 1.], [2., 15., 0.]]
        .map(point)
        .to_vec();
    let mesh = TriangleMesh::try_new(
        vertices,
        vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
        Tolerance::DEFAULT,
    )
    .unwrap();
    for trimmed in [false, true] {
        let brep = Brep::try_from_mesh(&mesh, trimmed, Tolerance::DEFAULT).unwrap();
        assert!(brep.is_solid());
        assert!(brep.signed_volume(Tolerance::DEFAULT).unwrap() > 0.8);
        assert_eq!(brep.solid_orientation().unwrap(), Unknown);
        assert_eq!(brep.reversed().solid_orientation().unwrap(), Unknown);
    }
}

#[test]
fn common_negative_weights_preserve_orientation_but_mixed_weights_cannot_use_the_hull() {
    let sphere = NurbsSurface::try_sphere(frame(), 2.).unwrap();
    for mixed in [false, true] {
        let mut controls = sphere
            .control_points()
            .iter()
            .map(|c| WeightedPoint3::try_new(c.point(), -c.weight()).unwrap())
            .collect::<Vec<_>>();
        if mixed {
            let i = sphere.control_point_count_u() + 1;
            controls[i] = WeightedPoint3::try_new(controls[i].point(), 1e-6).unwrap();
        }
        let surface = NurbsSurface::try_new_rational(
            sphere.degree_u(),
            sphere.degree_v(),
            sphere.control_point_count_u(),
            sphere.control_point_count_v(),
            controls,
            sphere.knots_u().to_vec(),
            sphere.knots_v().to_vec(),
        )
        .unwrap();
        let brep = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
        assert!(brep.is_solid());
        assert_eq!(
            brep.solid_orientation().unwrap(),
            if mixed { Unknown } else { Outward }
        );
    }
}

#[test]
fn rectangular_trim_proof_requires_exact_closed_continuous_boundary() {
    use crate::{Point2, WeightedPoint2};
    let original = cube([0.; 3], 1.).faces[0].clone();
    let expected = rectangle::bounds(&original).unwrap();
    let controls = original.loops[0].trims[0].curve.control_points();
    let a = controls[0].point().to_array();
    let b = controls[1].point().to_array();
    let fixed = usize::from(a[0] != b[0]);
    let interpolate =
        |t: Real| Point2::try_new(a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])).unwrap();
    for kind in ["positive", "negative", "mixed", "bulge", "jump", "gap"] {
        let mut points = [0., 0.75, 0.25, 1.]
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                WeightedPoint2::try_new(
                    interpolate(t),
                    if kind == "negative" || kind == "mixed" && i == 1 {
                        -1.
                    } else {
                        1.
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        if kind == "bulge" || kind == "gap" {
            let i = if kind == "gap" { 3 } else { 1 };
            let mut p = points[i].point().to_array();
            p[fixed] += 1e-14;
            points[i] = WeightedPoint2::try_new(Point2::try_new(p[0], p[1]).unwrap(), 1.).unwrap();
        }
        let mut face = original.clone();
        face.loops[0].trims[0].curve = NurbsCurve2::try_new_rational(
            1,
            points,
            if kind == "jump" {
                vec![0., 0., 0.5, 0.5, 1., 1.]
            } else {
                vec![0., 0., 0.25, 0.75, 1., 1.]
            },
        )
        .unwrap();
        assert_eq!(
            rectangle::bounds(&face),
            if matches!(kind, "positive" | "negative") {
                Some(expected)
            } else {
                None
            },
            "{kind}"
        );
    }
    let mut hole = original.clone();
    let mut inner = hole.loops[0].clone();
    inner.loop_type = BrepLoopType::Inner;
    hole.loops.push(inner);
    assert_eq!(rectangle::bounds(&hole), None);
    let mut shuffled = original.clone();
    shuffled.loops[0].trims.swap(0, 1);
    assert_eq!(rectangle::bounds(&shuffled), None);
}

#[test]
fn unsupported_but_equivalent_quadratic_trims_return_unknown_without_modification() {
    use crate::Point2;
    let mut brep = cube([0.; 3], 1.);
    for face in &mut brep.faces {
        for boundary in &mut face.loops {
            for trim in &mut boundary.trims {
                let points = trim.curve.control_points();
                let a = points[0].point();
                let b = points[1].point();
                trim.curve = NurbsCurve2::try_new(
                    2,
                    vec![
                        a,
                        Point2::try_new((a.x() + b.x()) * 0.5, (a.y() + b.y()) * 0.5).unwrap(),
                        b,
                    ],
                    vec![0., 0., 0., 1., 1., 1.],
                )
                .unwrap();
            }
        }
    }
    let brep = Brep::try_new(brep.vertices, brep.edges, brep.faces, Tolerance::DEFAULT).unwrap();
    assert!(brep.is_solid());
    let before = brep.clone();
    assert_eq!(brep.solid_orientation().unwrap(), Unknown);
    assert_eq!(brep, before);
}
