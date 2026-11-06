use super::*;
use crate::{Circle3, Polyline3};
fn p(x: Real, y: Real) -> Point3 {
    Point3::try_new(x, y, 0.).unwrap()
}
fn frame() -> Frame3 {
    Frame3::try_from_points(p(0., 0.), p(1., 0.), p(0., 1.), Tolerance::DEFAULT).unwrap()
}
fn path(points: &[[Real; 2]]) -> NurbsCurve {
    Polyline3::try_new(
        points.iter().map(|v| p(v[0], v[1])).collect(),
        Tolerance::DEFAULT,
    )
    .unwrap()
    .to_native_nurbs()
    .unwrap()
}
fn rect(a: Real, b: Real, c: Real, d: Real) -> NurbsCurve {
    path(&[[a, b], [c, b], [c, d], [a, d], [a, b]])
}
fn areas(regions: &[PlanarCurveRegion]) -> Vec<Real> {
    let mut areas = regions
        .iter()
        .map(|r| r.brep.area(Tolerance::DEFAULT).unwrap())
        .collect::<Vec<_>>();
    areas.sort_by(Real::total_cmp);
    areas
}
fn close(actual: &[Real], expected: &[Real]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!((a - b).abs() < 1e-8, "{actual:?} != {expected:?}");
    }
}
#[test]
fn picks_and_shared_edge_cancellation_keep_exact_rectangle_cells() {
    let paths = [rect(0., 0., 6., 6.), rect(4., 2., 10., 8.)];
    let old = paths.clone();
    let a = PlanarCurveArrangement::try_new(&paths, frame(), Tolerance::DEFAULT).unwrap();
    close(&areas(a.cells()), &[8., 28., 28.]);
    let mut picked = vec![false; a.cells().len()];
    for point in [p(1., 1.), p(5., 3.)] {
        picked[a.cell_at(point).unwrap().unwrap()] = true;
    }
    close(&areas(&a.selected(&picked, false).unwrap()), &[8., 28.]);
    close(&areas(&a.selected(&picked, true).unwrap()), &[36.]);
    close(
        &areas(&a.selected(&vec![true; a.cells().len()], true).unwrap()),
        &[64.],
    );
    assert_eq!(a.cell_at(p(-1., 0.)).unwrap(), None);
    assert_eq!(paths, old);
}
#[test]
fn holes_bridges_and_dangling_edges_preserve_bounded_material() {
    let outer = rect(0., 0., 10., 10.);
    let inner = rect(2., 2., 8., 8.);
    for bridge in [false, true] {
        let mut paths = vec![outer.clone(), inner.clone()];
        if bridge {
            paths.push(path(&[[0., 5.], [2., 5.]]));
        }
        let a = PlanarCurveArrangement::try_new(&paths, frame(), Tolerance::DEFAULT).unwrap();
        close(&areas(a.cells()), &[36., 64.]);
        close(
            &areas(&a.selected(&vec![true; a.cells().len()], true).unwrap()),
            &[100.],
        );
        let annulus = &a.cells()[a.cell_at(p(1., 1.)).unwrap().unwrap()];
        assert_eq!(
            annulus.brep.faces()[0].loops().len(),
            if bridge { 1 } else { 2 }
        );

        if bridge {
            assert_eq!(annulus.brep.vertices().len(), 12);
            assert_eq!(annulus.brep.edges().len(), 12);
            assert!(
                annulus.brep.faces()[0].loops()[0]
                    .trims()
                    .iter()
                    .all(|t| t.trim_type() == BrepTrimType::Boundary)
            );
        }
    }
    let a = PlanarCurveArrangement::try_new(
        &[outer, path(&[[0., 5.], [5., 5.]])],
        frame(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    close(&areas(a.cells()), &[100.]);
}
#[test]
fn circles_remain_rational_and_point_contacts_do_not_merge_cells() {
    let circle = |x| {
        Circle3::try_from_center_point(
            p(x, 0.),
            p(x + 5., 0.),
            frame().z_axis(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let a = PlanarCurveArrangement::try_new(&[circle(0.), circle(4.)], frame(), Tolerance::DEFAULT)
        .unwrap();
    let overlap = 50. * (0.4_f64.acos()) - 2. * 84_f64.sqrt();
    close(
        &areas(a.cells()),
        &[
            25. * std::f64::consts::PI - overlap,
            25. * std::f64::consts::PI - overlap,
            overlap,
        ],
    );
    close(
        &areas(&a.selected(&vec![true; a.cells().len()], true).unwrap()),
        &[50. * std::f64::consts::PI - overlap],
    );
    let a = PlanarCurveArrangement::try_new(
        &[rect(0., 0., 4., 4.), rect(4., 4., 8., 8.)],
        frame(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    close(
        &areas(&a.selected(&[true, true], true).unwrap()),
        &[16., 16.],
    );
}
