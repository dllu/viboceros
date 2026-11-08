use viboceros_smlib::Shape;
type SurfaceData = (
    usize,
    usize,
    usize,
    usize,
    Vec<([f64; 3], f64)>,
    Vec<f64>,
    Vec<f64>,
);
macro_rules! builder {
    ($name:ident,$geo:ident) => {
        mod $name {
            use $geo::*;
            pub fn source() -> NurbsSurface {
                let h = [
                    [0., 0.2, -0.1, 0.3],
                    [0.1, 0.8, 0.7, -0.2],
                    [0., 0.6, 1.1, 0.1],
                    [0.2, 0.1, 0., -0.2],
                ];
                let p = h
                    .into_iter()
                    .enumerate()
                    .flat_map(|(j, row)| {
                        row.into_iter().enumerate().map(move |(i, z)| {
                            Point3::try_new(i as f64 * 1.7, j as f64 * 1.2, z).unwrap()
                        })
                    })
                    .collect();
                let k = vec![0., 0., 0., 0., 1., 1., 1., 1.];
                NurbsSurface::try_new(3, 3, 4, 4, p, k.clone(), k).unwrap()
            }
            pub fn trimmed() -> Brep {
                let surface = source();
                let t = Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap();
                let mut vertices = Vec::new();
                let mut edges = Vec::new();
                let mut loops = Vec::new();
                for (i, (a, b)) in [(0.1, 0.9), (0.35, 0.65)].into_iter().enumerate() {
                    let points = if i == 0 {
                        [[a, a], [b, a], [b, b], [a, b], [a, a]]
                    } else {
                        [[a, a], [a, b], [b, b], [b, a], [a, a]]
                    };
                    let mut segments = Vec::new();
                    for (j, pair) in points.windows(2).enumerate() {
                        let (a, b) = (pair[0], pair[1]);
                        let (c, start, end) = if a[1] == b[1] {
                            (surface.isocurve_u(a[1]).unwrap(), a[0], b[0])
                        } else {
                            (surface.isocurve_v(a[0]).unwrap(), a[1], b[1])
                        };
                        let c = c.try_trimmed(start.min(end)..=start.max(end)).unwrap();
                        let c = if start > end {
                            c.reversed().unwrap()
                        } else {
                            c
                        };
                        segments.push(c.try_reparameterized(j as f64..=j as f64 + 1.).unwrap());
                    }
                    let curve =
                        PolyCurve3::try_with_segment_domains(segments, vec![0., 1., 2., 3., 4.])
                            .unwrap()
                            .to_nurbs()
                            .unwrap();
                    vertices.push(BrepVertex::try_new(curve.evaluate(0.).unwrap(), 0.).unwrap());
                    edges.push(BrepEdge::try_new([i, i], curve, 0.).unwrap());
                    let uv = NurbsCurve2::try_new_rational(
                        1,
                        points
                            .map(|p| {
                                WeightedPoint2::try_new(Point2::try_from(p).unwrap(), 1.).unwrap()
                            })
                            .to_vec(),
                        vec![0., 0., 1., 2., 3., 4., 4.],
                    )
                    .unwrap();
                    let trim = BrepTrim::try_new(
                        [i, i],
                        Some(i),
                        false,
                        uv,
                        BrepTrimType::Boundary,
                        SurfaceIso::NotIso,
                        [0.; 2],
                    )
                    .unwrap();
                    loops.push(
                        BrepLoop::try_new(
                            if i == 0 {
                                BrepLoopType::Outer
                            } else {
                                BrepLoopType::Inner
                            },
                            vec![trim],
                        )
                        .unwrap(),
                    );
                }
                Brep::try_new(
                    vertices,
                    edges,
                    vec![BrepFace::try_new(surface, false, loops).unwrap()],
                    t,
                )
                .unwrap()
            }
            pub fn edit(data: &super::SurfaceData, case: usize) -> String {
                let (du, dv, nu, nv, cp, ku, kv) = data;
                let points = cp
                    .iter()
                    .map(|(p, w)| {
                        WeightedPoint3::try_new(Point3::try_from(*p).unwrap(), *w).unwrap()
                    })
                    .collect();
                let surface = NurbsSurface::try_new_rational(
                    *du,
                    *dv,
                    *nu,
                    *nv,
                    points,
                    ku.clone(),
                    kv.clone(),
                )
                .unwrap();
                let source = trimmed();
                let start = std::time::Instant::now();
                let output = source
                    .try_with_edited_single_surface(
                        surface,
                        Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap(),
                    )
                    .unwrap();
                println!(
                    "CERTIFICATE_TIME {case} {} {}",
                    stringify!($name),
                    start.elapsed().as_secs_f64()
                );
                format!("{output:?}")
            }
        }
    };
}
builder!(baseline, baseline_geo);
builder!(candidate, current_geo);
fn main() {
    let t = current_geo::Tolerance::try_new(1e-6, 1e-9, 1e-6).unwrap();
    let directory =
        std::path::PathBuf::from(std::env::var_os("VIBOCEROS_CERTIFICATE_OUTPUT").unwrap());
    for (case, distance) in [-0.3, 0.5].into_iter().enumerate() {
        let input = current_geo::Brep::try_surface_face(candidate::source(), t).unwrap();
        let fitted = Shape::from_brep(&input, t)
            .unwrap()
            .offset_with_accuracy(distance, false, 2.5e-5)
            .unwrap()
            .to_single_surface_approximated(2.5e-5)
            .unwrap()
            .try_trimmed(0.0..=1.0, 0.0..=1.0)
            .unwrap();
        let data = (
            fitted.degree_u(),
            fitted.degree_v(),
            fitted.control_point_count_u(),
            fitted.control_point_count_v(),
            fitted
                .control_points()
                .iter()
                .map(|p| (p.point().to_array(), p.weight()))
                .collect(),
            fitted.knots_u().to_vec(),
            fitted.knots_v().to_vec(),
        );
        println!("CERTIFICATE_CASE {case} {distance} {} {}", data.2, data.3);
        let old = baseline::edit(&data, case);
        let new = candidate::edit(&data, case);
        assert_eq!(
            old, new,
            "complete resulting B-rep geometry/topology differs"
        );
        std::fs::write(directory.join(format!("baseline-{case}.txt")), old).unwrap();
        std::fs::write(directory.join(format!("candidate-{case}.txt")), new).unwrap();
    }
}
