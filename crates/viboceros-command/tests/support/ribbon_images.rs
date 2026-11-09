//! Full ruled images plus certified edge/vertex and face-loop incidence.
use super::{captured, support};
use serde_json::Value;
use viboceros_geometry::*;

pub fn unsupported(error: &viboceros_command::CommandError) -> bool {
    use viboceros_command::CommandError;
    if matches!(
        error,
        CommandError::Geometry(GeometryError::UnsupportedCurveOffset)
    ) {
        return true;
    }
    #[cfg(feature = "native-smlib")]
    if matches!(
        error,
        CommandError::NativeSmlib(viboceros_smlib::Error::Geometry(
            GeometryError::UnsupportedCurveOffset
        ))
    ) {
        return true;
    }
    false
}

pub fn verify(
    brep: &Brep,
    original: &NurbsCurve,
    native: &Value,
    label: &str,
    legacy: bool,
) -> (f64, f64) {
    let targets = native["surfaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(captured::surface)
        .collect::<Vec<_>>();
    let parents = brep
        .faces()
        .iter()
        .map(|f| f.surface().clone())
        .collect::<Vec<_>>();

    let cache = std::cell::RefCell::new(Vec::<(NurbsSurface, NurbsSurface, f64)>::new());
    let compare = |a: &NurbsSurface, b: &NurbsSurface, limit: f64| {
        if a == b {
            return Some(0.);
        }
        if let Some((_, _, bound)) = cache
            .borrow()
            .iter()
            .find(|(x, y, bound)| x == a && y == b && *bound <= limit)
        {
            return Some(*bound);
        }
        let bound = if legacy {
            a.parameter_surface_deviation_bound(b, limit)
                .ok()
                .flatten()?
        } else {
            let witness = original.try_trimmed(a.domain_v()).ok()?;
            a.ruled_surface_image_deviation_bound_with_witnesses(b, [Some(&witness), None], limit)
                .ok()
                .flatten()?
        };
        cache.borrow_mut().push((a.clone(), b.clone(), bound));
        Some(bound)
    };
    let image = support::coverage_with(
        brep,
        &parents,
        &native["brep"]["geometry"],
        &targets,
        &compare,
    )
    .unwrap_or_else(|| panic!("{label}: complete image mismatch"));
    assert!(image <= 1e-9);
    let mut used = vec![false; targets.len()];
    let mut face_map = Vec::new();
    for face in brep.faces() {
        let found = targets
            .iter()
            .enumerate()
            .find(|(i, s)| !used[*i] && compare(face.surface(), s, 1e-9).is_some())
            .unwrap()
            .0;
        used[found] = true;
        face_map.push(found);
    }
    let witnesses = brep
        .faces()
        .iter()
        .flat_map(|f| {
            let w = original.try_trimmed(f.surface().domain_v()).unwrap();
            [w.clone(), w.reversed().unwrap()]
        })
        .collect::<Vec<_>>();
    let compare_edge = |a: &NurbsCurve, b: &NurbsCurve, limit: f64| {
        if !legacy {
            for w in &witnesses {
                if [a, b].into_iter().any(|c| {
                    [false, true].into_iter().any(|end| {
                        let p = |c: &NurbsCurve| {
                            c.evaluate(if end {
                                *c.domain().end()
                            } else {
                                *c.domain().start()
                            })
                            .unwrap()
                        };
                        p(c).distance_to(p(w)).unwrap() > limit
                    })
                }) {
                    continue;
                }
                if let (Some(x), Some(y)) = (
                    a.parameter_curve_deviation_bound(w, limit * 0.5)
                        .ok()
                        .flatten(),
                    b.parameter_curve_deviation_bound(w, limit * 0.5)
                        .ok()
                        .flatten(),
                ) {
                    let sum = x + y;
                    let bound = if sum == 0. { 0. } else { sum.next_up() };
                    if bound <= limit {
                        return Some(bound);
                    }
                }
            }
        }
        a.parameter_curve_deviation_bound(b, limit).ok().flatten()
    };
    let incidence =
        support::incidence_with(brep, &native["brep"]["geometry"], &face_map, &compare_edge)
            .unwrap_or_else(|| panic!("{label}: incidence mismatch"));
    assert!(incidence <= 1e-9);
    (image, incidence)
}

// Native successes whose angular/projection mode switches remain unfinished.
// Every one is executed and must fail without editing the document or options.
pub fn expected_gap(family: &str, id: &str) -> bool {
    match family {
        "ribbon_cplane_boundaries" => [
            "parallel-tilt-0.001",
            "parallel-tilt-1e-05",
            "parallel-tilt-1e-07",
            "parallel-tilt-1e-09",
            "parallel-tilt-1e-11",
            "linear-projection-0.0001",
        ]
        .contains(&id),
        "ribbon_cplane_policy" => [
            "parallel-angle-0.005",
            "parallel-angle-0.01",
            "perpendicular-0.001",
            "height-0.0001",
        ]
        .contains(&id),
        "ribbon_cplane_switches" => id != "perpendicular-0.008",
        "ribbon_cplane_thresholds" => true,
        "ribbon_cplane_holdouts" => !id.starts_with("cubic-"),
        _ => false,
    }
}
