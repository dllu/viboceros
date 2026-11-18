use super::*;
use std::f64::consts::TAU;
use viboceros_geometry::CircularArc3;

#[test]
fn pipe_subd_circular_matches_rhino_limit_walls_and_cap_controls() {
    // Rhino 8 limit samples evaluated from the recorded subdivision cages at
    // common world-space bearings. The angular positions of individual control
    // rows can vary with the rail fit.
    let cases: Vec<Value> = serde_json::from_str(include_str!("../rhino_circular.json")).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let sweep = case["sweep_degrees"].as_f64().unwrap().to_radians();
        let closed = case["sweep_degrees"] == 360;
        let mut doc = Document::default();
        let tolerance = doc.tolerance();
        let origin = Point3::try_new(0., 0., 0.).unwrap();
        let world_xy =
            Frame3::try_from_normal(origin, Vector3::try_new(0., 0., 1.).unwrap(), tolerance)
                .unwrap();
        let circle = Circle3::try_from_frame(
            origin,
            case["major_radius"].as_f64().unwrap(),
            world_xy.x_axis(),
            world_xy.z_axis(),
            tolerance,
        )
        .unwrap();
        let source = if closed {
            Geometry::Circle(circle)
        } else {
            Geometry::Arc(CircularArc3::try_from_circle_sweep(circle, sweep).unwrap())
        };
        let rail_id = doc.add_geometry(source).unwrap();
        let rail = Rail::capture(&doc, rail_id, None).unwrap();
        let mut input = Input::new(Preferences::default());
        input.start_sizes(closed);
        let cap = case["cap"].as_str().unwrap();
        let blend = case["blend"].as_str().unwrap();
        input.option("Output=SubD").unwrap();
        input.option(&format!("Cap={cap}")).unwrap();
        input.option(&format!("ShapeBlending={blend}")).unwrap();
        let first: [Real; 2] = serde_json::from_value(case["first"].clone()).unwrap();
        let second: Option<[Real; 2]> = serde_json::from_value(case["second"].clone()).unwrap();
        if second.is_some() {
            input.option("Thick=Yes").unwrap();
        }
        for (end, r) in first
            .into_iter()
            .take(if closed { 1 } else { 2 })
            .enumerate()
        {
            input.number(r).unwrap();
            if let Some(other) = second {
                input.number(other[end]).unwrap();
            }
        }
        assert!(input.enter().unwrap(), "{name}");
        let outputs = input.geometries(&[rail], true, tolerance).unwrap();
        let [Geometry::SubD(subd)] = outputs.as_slice() else {
            panic!("{name}: expected one SubD");
        };
        let points = subd.control_points();
        let cap_points = match (cap, second) {
            ("Round", _) => 18,
            ("Flat", None) => 2,
            _ => 0,
        };
        let walls = if second.is_some() { 2 } else { 1 };
        let wall_len = (points.len() - cap_points) / walls;
        if name != "arc45" {
            assert_eq!(
                wall_len / 8,
                case["ring_count"].as_u64().unwrap() as usize,
                "{name}"
            );
        }
        let is_open = !closed && cap == "None";
        assert_eq!(
            subd.edge_face_counts().iter().filter(|&&n| n == 1).count(),
            if is_open { 16 } else { 0 },
            "{name}"
        );
        let expected_crease = match cap {
            "None" | "Flat" if !closed => 16 * walls,
            _ => 0,
        };
        assert_eq!(
            subd.edge_tags().iter().filter(|&&tag| tag == 2).count(),
            expected_crease,
            "{name}"
        );
        let cap_controls: Vec<[Real; 3]> =
            serde_json::from_value(case["cap_control_points"].clone()).unwrap();
        for (&p, expected) in points[wall_len * walls..].iter().zip(cap_controls) {
            assert!(
                p.distance_to(Point3::try_from(expected).unwrap()).unwrap() < 1e-9,
                "{name}"
            );
        }
        let caps = &points[wall_len * walls..];
        let round_rims = (cap == "Round").then(|| [&caps[..8], &caps[9..17]]);
        let epsilon = case["error"].as_f64().unwrap();
        for value in case["samples"].as_array().unwrap() {
            let (wall, bearing, around, expected): (usize, Real, Real, [Real; 3]) =
                serde_json::from_value(value.clone()).unwrap();
            let points = &points[wall * wall_len..(wall + 1) * wall_len];
            let actual = at_bearing(points, closed, round_rims, bearing * sweep, around);
            let error = actual
                .distance_to(Point3::try_from(expected).unwrap())
                .unwrap();
            assert!(
                error < epsilon,
                "{name}, wall {wall}, bearing {bearing}, octant {around}: {error}"
            );
        }
    }
}

fn at_bearing(
    points: &[Point3],
    closed: bool,
    round_rims: Option<[&[Point3]; 2]>,
    target: Real,
    around: Real,
) -> Point3 {
    let rows = (points.len() / 8) as Real;
    let [mut low, mut high] = if closed {
        [1., rows + 1.]
    } else {
        [0., rows - 1.]
    };
    for _ in 0..48 {
        let t = low.midpoint(high);
        let p = sample_patch(points, round_rims, closed, t, around);
        let mut bearing = p.y().atan2(p.x());
        if closed {
            let predicted = TAU * (t - 1.) / rows;
            bearing += TAU * ((predicted - bearing) / TAU).round();
        }
        if bearing < target {
            low = t;
        } else {
            high = t;
        }
    }
    sample_patch(points, round_rims, closed, low.midpoint(high), around)
}

fn sample_patch(
    points: &[Point3],
    round_rims: Option<[&[Point3]; 2]>,
    closed: bool,
    u: Real,
    v: Real,
) -> Point3 {
    let rows = (points.len() / 8) as isize;
    let iu = u.floor() as isize;
    let iv = v.floor() as isize;
    let mut result = [0.; 3];
    for (a, wu) in cubic_weights(u.fract()).into_iter().enumerate() {
        for (b, wv) in cubic_weights(v.fract()).into_iter().enumerate() {
            let row = iu + a as isize - 1;
            let column = (iv + b as isize - 1).rem_euclid(8) as usize;
            let at = |r: isize| points[r as usize * 8 + column].to_array();
            let p = if closed {
                at(row.rem_euclid(rows))
            } else if (0..rows).contains(&row) {
                at(row)
            } else if let Some(rims) = round_rims {
                rims[usize::from(row >= rows)][column].to_array()
            } else {
                let [edge, neighbor] = if row < 0 {
                    [0, 1]
                } else {
                    [rows - 1, rows - 2]
                };
                let [p, q] = [at(edge), at(neighbor)];
                std::array::from_fn(|i| 2. * p[i] - q[i])
            };
            for i in 0..3 {
                result[i] += wu * wv * p[i];
            }
        }
    }
    Point3::try_from(result).unwrap()
}

fn cubic_weights(t: Real) -> [Real; 4] {
    [
        (1. - t).powi(3),
        t * t * (3. * t - 6.) + 4.,
        1. + t * (3. + t * (3. - 3. * t)),
        t.powi(3),
    ]
    .map(|w| w / 6.)
}
