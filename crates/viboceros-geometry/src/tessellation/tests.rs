use super::*;
use crate::{Brep, Frame3, Point3, WeightedPoint3};

fn surface(points: &[[f64; 3]], count_u: usize, knots_u: Vec<f64>) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        count_u,
        2,
        points
            .iter()
            .map(|&p| WeightedPoint3::try_new(Point3::try_from(p).unwrap(), 1.).unwrap())
            .collect(),
        knots_u,
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}

#[test]
fn normals_match_derivatives_and_uv_samples_without_changing_geometry() {
    let s = surface(
        &[[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 1.]],
        2,
        vec![0., 0., 1., 1.],
    );
    let output = s.tessellate_with_normals(4, Tolerance::DEFAULT).unwrap();
    assert_eq!(output.mesh, s.tessellate(4, Tolerance::DEFAULT).unwrap());
    assert_eq!(output.corners.len(), output.mesh.triangles().len());
    for (triangle, corners) in output.mesh.triangles().iter().zip(&output.corners) {
        for (&index, sample) in triangle.iter().zip(corners) {
            let p = s.evaluate(sample.uv.x(), sample.uv.y()).unwrap();
            assert_eq!(p, output.mesh.vertices()[index as usize]);
            assert_eq!(sample.face, None);
            assert_eq!(sample.normal_source, TessellationNormalSource::Analytic);
            let (_, su, sv) = s
                .evaluate_with_derivatives(sample.uv.x(), sample.uv.y())
                .unwrap();
            let expected = su.cross(sv).unwrap().normalized_nonzero().unwrap();
            assert!(
                expected
                    .as_vector()
                    .dot(sample.normal.unwrap().as_vector())
                    .unwrap()
                    > 1. - 1e-12
            );
        }
    }
}

#[test]
fn internal_knot_corners_keep_both_one_sided_normals() {
    let s = surface(
        &[
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 1.],
            [0., 1., 0.],
            [1., 1., 0.],
            [2., 1., 1.],
        ],
        3,
        vec![0., 0., 1., 2., 2.],
    );
    assert!(
        s.normal_at_on_sides(1., 0.5, ParameterSide::Left, ParameterSide::Right)
            .unwrap()
            .z()
            > 0.999
    );
    let output = s.tessellate_with_normals(2, Tolerance::DEFAULT).unwrap();
    let seam = output
        .corners
        .iter()
        .flatten()
        .filter(|c| (c.uv.x() - 1.).abs() < 1e-12)
        .map(|c| c.normal.unwrap())
        .collect::<Vec<_>>();
    assert!(seam.iter().any(|n| n.x().abs() < 1e-12 && n.z() > 0.999));
    assert!(seam.iter().any(|n| n.x() < -0.7 && n.z() > 0.7));
}

#[test]
fn singular_corner_uses_local_limit_and_keeps_regular_channels() {
    let s = surface(
        &[[0., 0., 0.], [1., 0., 0.], [0., 0., 0.], [1., 1., 0.]],
        2,
        vec![0., 0., 1., 1.],
    );
    let output = s.tessellate_with_normals(4, Tolerance::DEFAULT).unwrap();
    assert!(
        output
            .corners
            .iter()
            .flatten()
            .any(|c| c.normal_source == TessellationNormalSource::Analytic)
    );
    let singular = output
        .corners
        .iter()
        .flatten()
        .filter(|c| c.uv.x() == 0.)
        .collect::<Vec<_>>();
    assert!(!singular.is_empty());
    assert!(singular.iter().all(
        |c| c.normal_source == TessellationNormalSource::SampledLimit
            && c.normal.unwrap().z() > 0.999
    ));
}

#[test]
fn brep_face_sources_and_reversal_preserve_hard_edges() {
    let mut b = Brep::try_box(
        Frame3::try_from_directions(
            Point3::try_new(0., 0., 0.).unwrap(),
            crate::Vector3::try_new(1., 0., 0.).unwrap(),
            crate::Vector3::try_new(0., 1., 0.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        [[0., 1.]; 3],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let forward = b.tessellate_with_normals(2, Tolerance::DEFAULT).unwrap();
    assert_eq!(forward.mesh, b.tessellate(2, Tolerance::DEFAULT).unwrap());
    let mut faces = std::collections::BTreeSet::new();
    for (triangle, corners) in forward.mesh.triangles().iter().zip(&forward.corners) {
        let normal = forward
            .mesh
            .face_normal(
                forward
                    .mesh
                    .triangles()
                    .iter()
                    .position(|t| t == triangle)
                    .unwrap(),
            )
            .unwrap();
        for c in corners {
            faces.insert(c.face.unwrap());
            assert!(
                normal
                    .as_vector()
                    .dot(c.normal.unwrap().as_vector())
                    .unwrap()
                    > 0.999
            );
            assert_eq!(
                b.faces()[c.face.unwrap()]
                    .surface()
                    .evaluate(c.uv.x(), c.uv.y())
                    .unwrap(),
                forward.mesh.vertices()
                    [triangle[corners.iter().position(|x| x == c).unwrap()] as usize]
            );
        }
    }
    assert_eq!(faces.len(), 6);
    b.reverse_orientation();
    let reverse = b.tessellate_with_normals(2, Tolerance::DEFAULT).unwrap();
    for (a, b) in forward.corners.iter().zip(reverse.corners) {
        assert!(
            a[0].normal
                .unwrap()
                .as_vector()
                .dot(b[0].normal.unwrap().as_vector())
                .unwrap()
                < -0.999
        );
    }
}
