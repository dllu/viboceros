//! Conforming triangle/quad pieces for the possible split-edge masks.
//! The unchanged logical face is retained as an n-gon over these pieces.
use super::*;
pub(super) fn count(n: usize, m: &[Option<u32>; 4], center: bool) -> usize {
    let k = m.iter().filter(|i| i.is_some()).count();
    if center {
        n
    } else {
        match (n, k) {
            (3, 1 | 2) => 2,
            (4, 1) => 3,
            (4, 2) if m[0].is_some() == m[2].is_some() => 2,
            (4, 2 | 3) => 4,
            (_, 0) => 1,
            _ => unreachable!("valid split mask"),
        }
    }
}
pub(super) fn pieces(face: MeshFace, m: &[Option<u32>; 4], center: Option<u32>) -> Vec<MeshFace> {
    let f = face.indices();
    let n = f.len();
    let v = |i: usize| f[i % n];
    let e = |i: usize| m[i % n].unwrap();
    let t = |a, b, c| MeshFace::Triangle([a, b, c]);
    let q = |a, b, c, d| MeshFace::Quad([a, b, c, d]);
    if let Some(center) = center {
        return (0..n)
            .map(|i| q(v(i), e(i), center, e(i + n - 1)))
            .collect();
    }
    let k = m.iter().filter(|i| i.is_some()).count();
    match (n, k) {
        (_, 0) => vec![face],
        (3, 1) => {
            let s = m.iter().position(Option::is_some).unwrap();
            vec![t(v(s), e(s), v(s + 2)), t(e(s), v(s + 1), v(s + 2))]
        }
        (3, 2) => {
            let shared = (0..3)
                .find(|&i| m[(i + 2) % 3].is_some() && m[i].is_some())
                .unwrap();
            vec![
                q(v(shared + 2), e(shared + 2), e(shared), v(shared + 1)),
                t(v(shared), e(shared), e(shared + 2)),
            ]
        }
        (4, 1) => {
            let s = m.iter().position(Option::is_some).unwrap();
            vec![
                t(v(s), e(s), v(s + 3)),
                t(v(s + 2), v(s + 3), e(s)),
                t(v(s + 1), v(s + 2), e(s)),
            ]
        }
        (4, 2) if m[0].is_some() == m[2].is_some() => {
            let s = m.iter().position(Option::is_some).unwrap();
            vec![
                q(v(s), e(s), e(s + 2), v(s + 3)),
                q(v(s + 1), v(s + 2), e(s + 2), e(s)),
            ]
        }
        (4, 2) => {
            let s = (0..4)
                .find(|&i| m[i].is_some() && m[(i + 1) % 4].is_some())
                .unwrap();
            vec![
                t(v(s), e(s), v(s + 3)),
                t(v(s + 3), e(s), e(s + 1)),
                t(v(s + 1), e(s + 1), e(s)),
                t(v(s + 2), v(s + 3), e(s + 1)),
            ]
        }
        (4, 3) => {
            let missing = m.iter().position(Option::is_none).unwrap();
            let s = (missing + 1) % 4;
            vec![
                q(v(s), e(s), e(s + 2), v(s + 3)),
                t(e(s), e(s + 1), e(s + 2)),
                t(v(s + 1), e(s + 1), e(s)),
                t(v(s + 2), e(s + 2), e(s + 1)),
            ]
        }
        _ => unreachable!("fully split faces have a center"),
    }
}
