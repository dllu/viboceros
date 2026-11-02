use super::*;
use crate::Polyline3;
#[test]
fn face_chart_edits_preserve_spatial_geometry_normals_holes_and_seams() {
    let t = Tolerance::DEFAULT;
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let frame = Frame3::try_from_directions(
        p(0., 0., 0.),
        Vector3::try_new(1., 0., 0.).unwrap(),
        Vector3::try_new(0., 1., 0.).unwrap(),
        t,
    )
    .unwrap();
    let polygon = |vertices| Polyline3::try_new(vertices, t).unwrap().to_nurbs().unwrap();
    let outer = polygon(vec![
        p(0., 0., 0.),
        p(8., 0., 0.),
        p(8., 6., 0.),
        p(0., 6., 0.),
        p(0., 0., 0.),
    ]);
    let hole = polygon(vec![
        p(2., 2., 0.),
        p(2., 4., 0.),
        p(5., 4., 0.),
        p(5., 2., 0.),
        p(2., 2., 0.),
    ]);
    let planar = Brep::try_planar_face_with_holes(&outer, &[hole], t).unwrap();
    let cylinder =
        Brep::try_surface_face(NurbsSurface::try_cylinder(frame, 2., -3., 5.).unwrap(), t).unwrap();
    let controls = [
        [0., 0., 0.],
        [2., 3., 1.],
        [5., 0., 2.],
        [0., 4., 1.],
        [2., 7., 3.],
        [5., 4., 4.],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, p)| {
        WeightedPoint3::try_new(
            Point3::try_from(p).unwrap(),
            if i % 3 == 1 { 0.5 } else { 1. },
        )
        .unwrap()
    })
    .collect();
    let warped = Brep::try_surface_face(
        NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![-4., -4., -4., 2., 2., 2.],
            vec![3., 3., 9., 9.],
        )
        .unwrap(),
        t,
    )
    .unwrap();
    let sphere = Brep::try_surface_face(NurbsSurface::try_sphere(frame, 2.).unwrap(), t).unwrap();
    for mut source in [planar, cylinder, warped, sphere] {
        for trim in source
            .faces
            .iter_mut()
            .flat_map(|f| &mut f.loops)
            .flat_map(|l| &mut l.trims)
        {
            trim.tolerance = [1e-12, 2e-12];
        }
        source.validate(t).unwrap();
        for (source, edit) in [source.clone(), source.reversed()]
            .into_iter()
            .flat_map(|source| {
                [Edit::Swap, Edit::ReverseU, Edit::ReverseV].map(|edit| (source.clone(), edit))
            })
        {
            let mut output = source.clone();
            let vertex_storage = output.vertices.as_ptr();
            let edge_storage = output.edges.as_ptr();
            output
                .edit_face_charts(&(0..source.faces.len()).collect::<Vec<_>>(), edit)
                .unwrap();
            output.validate(t).unwrap();
            assert_eq!(output.vertices.as_ptr(), vertex_storage);
            assert_eq!(output.edges.as_ptr(), edge_storage);
            assert_eq!(source.vertices(), output.vertices());
            assert_eq!(source.edges(), output.edges());
            assert_eq!(source.is_solid(), output.is_solid());
            assert_eq!(source.is_manifold(), output.is_manifold());
            for (a, b) in source.faces().iter().zip(output.faces()) {
                assert_eq!(a, &b.edit_chart(edit).unwrap());
                for (u, v) in [(0.2, 0.3), (0.5, 0.5), (0.8, 0.7)] {
                    let u = a.surface().parameter_at_u(u).unwrap();
                    let v = a.surface().parameter_at_v(v).unwrap();
                    let uv = edit.point(Point2::try_new(u, v).unwrap()).unwrap();
                    assert!(
                        a.surface()
                            .evaluate(u, v)
                            .unwrap()
                            .distance_to(b.surface().evaluate(uv.x(), uv.y()).unwrap())
                            .unwrap()
                            < 1e-12
                    );
                    assert_eq!(
                        a.contains_parameters(u, v, t).unwrap(),
                        b.contains_parameters(uv.x(), uv.y(), t).unwrap()
                    );
                    let normal = |f: &BrepFace, u, v| {
                        let n = f.surface().normal_at(u, v).unwrap();
                        if f.is_reversed() { n.opposite() } else { n }
                    };
                    assert!(
                        normal(a, u, v)
                            .as_vector()
                            .dot(normal(b, uv.x(), uv.y()).as_vector())
                            .unwrap()
                            > 1. - 1e-12
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_face_uv_indices_are_atomic_and_empty_selection_is_exact() {
    let frame = Frame3::try_from_normal(
        Point3::try_new(0., 0., 0.).unwrap(),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = Brep::try_box(frame, [[0., 2.], [0., 3.], [0., 5.]], Tolerance::DEFAULT).unwrap();
    for (indices, edit) in [
        vec![source.faces.len()],
        vec![0, source.faces.len()],
        vec![0, 0],
        vec![usize::MAX],
    ]
    .into_iter()
    .flat_map(|indices| {
        [Edit::Swap, Edit::ReverseU, Edit::ReverseV].map(|edit| (indices.clone(), edit))
    }) {
        let mut output = source.clone();
        assert!(output.edit_face_charts(&indices, edit).is_err());
        assert_eq!(output, source);
    }
    for edit in [Edit::Swap, Edit::ReverseU, Edit::ReverseV] {
        let mut output = source.clone();
        output.edit_face_charts(&[], edit).unwrap();
        assert_eq!(output, source);
        output.edit_face_charts(&[1, 3, 5], edit).unwrap();
        output.validate(Tolerance::DEFAULT).unwrap();
        for i in [0, 2, 4] {
            assert_eq!(output.faces[i], source.faces[i]);
        }
        assert!(output.is_solid());
        output.edit_face_charts(&[1, 3, 5], edit).unwrap();
        assert_eq!(output, source);
    }
}
