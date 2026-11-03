//! Certify every native packed image by recovering its source-cell rectangles.
//! Native cell intervals are integer lattice intervals of length one or two;
//! source corner matches only select candidates, then complete basis bounds
//! verify their images. Coverage checks exclude gaps or duplicate rectangles.
use std::collections::{BTreeMap, HashSet};
use viboceros_geometry::{NurbsSurface, Point3};

#[derive(Clone, Copy)]
pub struct Placement {
    pub range: [f64; 4],
    pub rotation: u8,
}

pub struct NativeImage {
    surface: NurbsSurface,
    candidates: Vec<([i32; 4], [Point3; 4])>,
    covered: HashSet<(i32, i32)>,
    area: usize,
}
fn bound(a: &NurbsSurface, b: &NurbsSurface) -> f64 {
    let (a, b) = crate::basis::common_basis(a, b);
    crate::bound::complete_bound(&a, &b)
}
pub fn unrotate(surface: NurbsSurface, q: u8) -> NurbsSurface {
    let surface = match q {
        0 => surface,
        1 => surface.try_swapped_uv().unwrap().try_reversed_v().unwrap(),
        2 => surface.try_reversed_u().unwrap().try_reversed_v().unwrap(),
        3 => surface.try_swapped_uv().unwrap().try_reversed_u().unwrap(),
        _ => panic!("invalid chart rotation"),
    };
    surface.try_reparameterized(0. ..=1., 0. ..=1.).unwrap()
}
impl NativeImage {
    pub fn new(surface: NurbsSurface) -> Self {
        let domain = [
            *surface.domain_u().start(),
            *surface.domain_u().end(),
            *surface.domain_v().start(),
            *surface.domain_v().end(),
        ];
        let range = domain.map(|v| {
            assert!((v - v.round()).abs() < 1e-12);
            v.round() as i32
        });
        let [u0, u1, v0, v1] = range;
        let mut points = BTreeMap::new();
        for v in v0..=v1 {
            for u in u0..=u1 {
                points.insert((u, v), surface.evaluate(u as f64, v as f64).unwrap());
            }
        }
        let mut candidates = Vec::new();
        for v in v0..v1 {
            for u in u0..u1 {
                for dv in 1..=2 {
                    for du in 1..=2 {
                        if u + du > u1 || v + dv > v1 {
                            continue;
                        }
                        candidates.push((
                            [u, u + du, v, v + dv],
                            [
                                points[&(u, v)],
                                points[&(u + du, v)],
                                points[&(u + du, v + dv)],
                                points[&(u, v + dv)],
                            ],
                        ));
                    }
                }
            }
        }
        Self {
            surface,
            candidates,
            covered: HashSet::new(),
            area: ((u1 - u0) * (v1 - v0)) as usize,
        }
    }
    pub fn check(&mut self, local: &NurbsSurface, sector: bool) -> (f64, Placement) {
        if sector {
            let target = self
                .surface
                .try_reparameterized(0. ..=1., 0. ..=1.)
                .unwrap()
                .try_reversed_u()
                .unwrap()
                .try_reversed_v()
                .unwrap();
            let error = bound(local, &target);
            assert!(error < 1e-11, "native packed sector {error}");
            self.covered.insert((0, 0));
            assert_eq!(self.area, 1);
            return (
                error,
                Placement {
                    range: [
                        *self.surface.domain_u().start(),
                        *self.surface.domain_u().end(),
                        *self.surface.domain_v().start(),
                        *self.surface.domain_v().end(),
                    ],
                    rotation: 2,
                },
            );
        }
        let corners =
            [[0., 0.], [1., 0.], [1., 1.], [0., 1.]].map(|[u, v]| local.evaluate(u, v).unwrap());
        let mut best = f64::INFINITY;
        for &(range, points) in &self.candidates {
            for q in 0..4 {
                if (0..4).any(|k| corners[k].distance_to(points[(k + q) % 4]).unwrap() > 1e-11) {
                    continue;
                }
                let [u0, u1, v0, v1] = range;
                let part = self
                    .surface
                    .try_trimmed(u0 as f64..=u1 as f64, v0 as f64..=v1 as f64)
                    .unwrap();
                let error = bound(local, &unrotate(part, q as u8));
                best = best.min(error);
                if error >= 1e-11 {
                    continue;
                }
                for v in v0..v1 {
                    for u in u0..u1 {
                        assert!(self.covered.insert((u, v)), "duplicate native packed cell");
                    }
                }
                return (
                    error,
                    Placement {
                        range: range.map(|v| v as f64),
                        rotation: q as u8,
                    },
                );
            }
        }
        panic!("no matching native packed cell; best complete image bound {best}");
    }
    pub fn finish(&self) {
        assert_eq!(
            self.covered.len(),
            self.area,
            "incomplete native packed image"
        );
    }
}
