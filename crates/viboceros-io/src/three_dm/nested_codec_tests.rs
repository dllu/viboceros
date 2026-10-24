//! Exercise both sides of the private codec, including old payloads and rejects.
use super::*;
use viboceros_geometry::{BrepEdge, CurveSegment3};

fn native(object_type: c_int, bytes: &[u8]) -> (c_int, c_int) {
    let view = ffi::ViboWriteObject {
        object_type,
        geometry_data: bytes.as_ptr(),
        geometry_data_count: bytes.len(),
        ..Default::default()
    };
    let mut valid = -1;
    let mut message = [0 as c_char; 4096];
    // The view borrows bytes until this synchronous call returns.
    let result = unsafe {
        ffi::vibo_geometry_check(
            &view,
            std::ptr::null(),
            &mut valid,
            message.as_mut_ptr(),
            message.len(),
        )
    };
    (result, valid)
}
fn count(bytes: &[u8], start: usize) -> usize {
    u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap()) as usize
}
fn flag_after_curve(bytes: &[u8], start: usize) -> usize {
    start + 20 + count(bytes, start + 4) * 32 + count(bytes, start + 12) * 8
}
fn source() -> NurbsCurve {
    NurbsCurve::try_new(
        1,
        vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(2., 0., 0.).unwrap(),
        ],
        vec![-2., -2., 4., 4.],
    )
    .unwrap()
}
fn brep() -> Brep {
    Brep::try_box(
        Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
        [[0., 2.], [0., 3.], [0., 4.]],
        Tolerance::DEFAULT,
    )
    .unwrap()
}
fn brep_flags(bytes: &[u8]) -> Vec<usize> {
    let mut position = 36 + count(bytes, 12) * 32;
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    (0..count(bytes, 20))
        .map(|_| {
            let flag = flag_after_curve(bytes, position + 24);
            position = flag + if version >= 3 { 2 } else { 1 };
            flag
        })
        .collect()
}

#[test]
fn old_polycurve_versions_and_brep_v1_remain_readable_on_both_sides() {
    let p = PolyCurve3::try_new(vec![source()]).unwrap();
    let mut v2 = three_dm_geometry::encode_polycurve(&p).unwrap();
    assert_eq!(v2.pop(), Some(0)); // Last byte is the v3 curve flag.
    v2[8..12].copy_from_slice(&2u32.to_le_bytes());
    let mut v1 = v2.clone();
    assert_eq!(v1.remove(36), 4); // v1 had no typed segment tag.
    v1[8..12].copy_from_slice(&1u32.to_le_bytes());
    for bytes in [&v1, &v2] {
        assert_eq!(three_dm_geometry::decode_polycurve(bytes).unwrap(), p);
        assert_eq!(native(OBJECT_POLYCURVE, bytes), (1, 1));
    }
    let b = brep();
    let mut v2 = three_dm_geometry::encode_brep(&b).unwrap();
    for flag in brep_flags(&v2).into_iter().rev() {
        assert_eq!(v2.remove(flag + 1), 0); // v3 analytic-arc provenance.
    }
    v2[8..12].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(
        three_dm_geometry::decode_brep(&v2, Tolerance::DEFAULT).unwrap(),
        b
    );
    assert_eq!(native(OBJECT_BREP, &v2), (1, 1));
    let mut v1 = v2;
    for flag in brep_flags(&v1).into_iter().rev() {
        assert_eq!(v1.remove(flag), 0);
    }
    v1[8..12].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        three_dm_geometry::decode_brep(&v1, Tolerance::DEFAULT).unwrap(),
        b
    );
    assert_eq!(native(OBJECT_BREP, &v1), (1, 1));
}

#[test]
fn nested_flags_preserve_unit_weights_and_reject_ambiguous_encodings() {
    for rational in [false, true] {
        let c = if rational {
            source().with_rational_representation()
        } else {
            source()
        };
        let p = PolyCurve3::try_new(vec![c]).unwrap();
        let bytes = three_dm_geometry::encode_polycurve(&p).unwrap();
        assert_eq!(three_dm_geometry::decode_polycurve(&bytes).unwrap(), p);
        assert_eq!(native(OBJECT_POLYCURVE, &bytes), (1, 1));
        let mut bad = bytes.clone();
        *bad.last_mut().unwrap() = 2;
        assert!(three_dm_geometry::decode_polycurve(&bad).is_err());
        assert_eq!(native(OBJECT_POLYCURVE, &bad), (0, -1));
        for length in 0..bytes.len() {
            assert!(three_dm_geometry::decode_polycurve(&bytes[..length]).is_err());
            assert_eq!(native(OBJECT_POLYCURVE, &bytes[..length]), (0, -1));
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(three_dm_geometry::decode_polycurve(&trailing).is_err());
        assert_eq!(native(OBJECT_POLYCURVE, &trailing), (0, -1));
    }
    let c = source();
    let weighted = NurbsCurve::try_new_rational(
        c.degree(),
        c.control_points()
            .iter()
            .map(|p| WeightedPoint3::try_new(p.point(), 7.).unwrap())
            .collect(),
        c.knots().to_vec(),
    )
    .unwrap();
    let p = PolyCurve3::try_new(vec![CurveSegment3::NurbsCurve(weighted)]).unwrap();
    let mut bad = three_dm_geometry::encode_polycurve(&p).unwrap();
    assert_eq!(bad.pop(), Some(1));
    bad.push(0); // A polynomial encoding cannot carry a nonunit gauge.
    assert!(three_dm_geometry::decode_polycurve(&bad).is_err());
    assert_eq!(native(OBJECT_POLYCURVE, &bad), (0, -1));
}

#[test]
fn brep_edge_flags_are_independent_of_uv_and_surface_payloads() {
    let original = brep();
    let edges = original
        .edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            BrepEdge::try_new(
                e.vertices(),
                if i % 2 == 0 {
                    e.curve().clone().with_rational_representation()
                } else {
                    e.curve().clone()
                },
                e.tolerance(),
            )
            .unwrap()
        })
        .collect();
    let b = Brep::try_new(
        original.vertices().to_vec(),
        edges,
        original.faces().to_vec(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let bytes = three_dm_geometry::encode_brep(&b).unwrap();
    assert_eq!(
        three_dm_geometry::decode_brep(&bytes, Tolerance::DEFAULT).unwrap(),
        b
    );
    assert_eq!(native(OBJECT_BREP, &bytes), (1, 1));
    for flag in brep_flags(&bytes) {
        let mut bad = bytes.clone();
        bad[flag] = 255;
        assert!(three_dm_geometry::decode_brep(&bad, Tolerance::DEFAULT).is_err());
        assert_eq!(native(OBJECT_BREP, &bad), (0, -1));
        bad = bytes.clone();
        bad[flag + 1] = 255;
        assert!(three_dm_geometry::decode_brep(&bad, Tolerance::DEFAULT).is_err());
        assert_eq!(native(OBJECT_BREP, &bad), (0, -1));
        bad[flag + 1] = 1; // A line cannot claim analytic arc provenance.
        assert!(three_dm_geometry::decode_brep(&bad, Tolerance::DEFAULT).is_err());
        assert_eq!(native(OBJECT_BREP, &bad), (0, -1));
    }
    // Fail closed on missing flags, even if all geometry before them is valid.
    for flag in brep_flags(&bytes) {
        assert!(three_dm_geometry::decode_brep(&bytes[..flag], Tolerance::DEFAULT).is_err());
        assert_eq!(native(OBJECT_BREP, &bytes[..flag]), (0, -1));
    }
}
