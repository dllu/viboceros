//! Independent sources for native SelChain lifecycle comparisons.
use super::*;
use viboceros_command::chain_selection::{Session, Target, defaults, parse_options};
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    scene: String,
    seed: Option<usize>,
    pre: Vec<usize>,
    steps: Vec<Step>,
    finish: String,
    #[serde(default)]
    options: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Step {
    kind: String,
    modifiers: Option<String>,
    value: Option<String>,
    source: Option<usize>,
}
pub fn source(scene: &str, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
    let p = |x, y| Point3::try_new(x, y, 0.);
    let line = |a, b| LineSegment::try_new(a, b, tolerance).map(Geometry::Line);
    let rows = match scene {
        s if s.starts_with("arc_") || s == "line_arc" || s == "closed_circle" => {
            if s == "closed_circle" {
                let circle = viboceros_geometry::Circle3::try_from_three_points(
                    p(-3., 0.)?,
                    p(0., 3.)?,
                    p(3., 0.)?,
                    tolerance,
                )?;
                vec![Geometry::Circle(circle), line(p(3., 0.)?, p(6., 0.)?)?]
            } else {
                let q = std::f64::consts::FRAC_1_SQRT_2;
                let radius = if s == "arc_opposite" || s == "line_arc" {
                    1.
                } else {
                    s.rsplit('_')
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .map_err(|_| ProbeError::FixtureInvariant("owned chain radius"))?
                };
                let sign = if s == "arc_opposite" { -1. } else { 1. };
                let scale = if s.starts_with("arc_scaled_") {
                    s.rsplit('_')
                        .nth(1)
                        .unwrap()
                        .parse::<f64>()
                        .map_err(|_| ProbeError::FixtureInvariant("owned chain scale"))?
                } else {
                    1.
                };
                let point = |x, y| {
                    let (x, y) = (x * scale, y * scale);
                    if s.starts_with("arc_spatial_") {
                        Point3::try_new(x, y * q, y * q)
                    } else {
                        p(x, y)
                    }
                };
                let arc = |points: [Point3; 3]| {
                    viboceros_geometry::CircularArc3::try_from_three_points(
                        points[0], points[1], points[2], tolerance,
                    )
                    .map(Geometry::Arc)
                };
                let first = if s == "line_arc" {
                    line(point(-3., 0.)?, point(0., 0.)?)?
                } else {
                    arc([point(-1., 1.)?, point(-q, 1. - q)?, point(0., 0.)?])?
                };
                vec![
                    first,
                    arc([
                        point(0., 0.)?,
                        point(radius * q, sign * radius * (1. - q))?,
                        point(radius, sign * radius)?,
                    ])?,
                ]
            }
        }
        "lines" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3., 0.)?, p(6., 0.)?)?,
            line(p(6., 0.)?, p(9., 0.)?)?,
            line(p(0., 4.)?, p(3., 4.)?)?,
        ],
        "reversed" => vec![
            line(p(3., 0.)?, p(0., 0.)?)?,
            line(p(3., 0.)?, p(6., 0.)?)?,
            line(p(9., 0.)?, p(6., 0.)?)?,
        ],
        "corner" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3., 0.)?, p(3., 3.)?)?,
            line(p(3., 3.)?, p(6., 3.)?)?,
        ],
        "cycle" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3., 0.)?, p(3., 3.)?)?,
            line(p(3., 3.)?, p(0., 3.)?)?,
            line(p(0., 3.)?, p(0., 0.)?)?,
        ],
        "branch" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3., 0.)?, p(6., 0.)?)?,
            line(p(3., 0.)?, p(6., 3.)?)?,
        ],
        "subdoc_gap" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3.0000005, 0.)?, p(6., 0.)?)?,
        ],
        "gap" => vec![
            line(p(0., 0.)?, p(3., 0.)?)?,
            line(p(3.00001, 0.)?, p(6., 0.)?)?,
        ],
        "surface" | "mixed" | "prefer_curve" => {
            let x = if scene == "prefer_curve" { 3. } else { 0. };
            let patch =
                NurbsSurface::try_bilinear([p(x, 0.)?, p(x + 3., 0.)?, p(x + 3., 3.)?, p(x, 3.)?])?;
            let surface = Geometry::Brep(Brep::try_surface_face(patch, tolerance)?);
            if scene == "prefer_curve" {
                vec![
                    line(p(0., 0.)?, p(3., 0.)?)?,
                    line(p(3., 0.)?, p(6., 0.)?)?,
                    surface,
                ]
            } else {
                let mut v = vec![surface];
                if scene == "mixed" {
                    v.push(line(p(3., 0.)?, p(6., 0.)?)?);
                }
                v
            }
        }
        s if s.starts_with("angle_") => {
            let a = s[6..]
                .parse::<f64>()
                .map_err(|_| ProbeError::FixtureInvariant("owned chain angle"))?
                .to_radians();
            vec![
                line(p(0., 0.)?, p(3., 0.)?)?,
                line(p(3., 0.)?, p(3. + 3. * a.cos(), 3. * a.sin())?)?,
            ]
        }
        _ => return Err(ProbeError::FixtureInvariant("owned chain scene required")),
    };
    Ok(rows)
}
fn snapshot(
    doc: &Document,
    registry: &CommandRegistry,
    ids: &[ObjectId],
) -> Result<Value, ProbeError> {
    let mut components=registry.component_selection().checked_picks(doc).map_err(ProbeError::FixtureInvariant)?.iter().map(|p|json!({"object":ids.iter().position(|&id|id==p.object).unwrap(),"kind":"BrepEdge","index":p.index})).collect::<Vec<_>>();
    components.sort_by_key(|c| (c["object"].as_u64().unwrap(), c["index"].as_u64().unwrap()));
    components.dedup();
    Ok(
        json!({"whole":ids.iter().enumerate().filter_map(|(i,&id)|doc.is_selected(id).then_some(i)).collect::<Vec<_>>(),"components":components,"objects":doc.objects().len()}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.steps.len() > 24 || !matches!(f.finish.as_str(), "Enter" | "Cancel") {
        return Err(ProbeError::FixtureInvariant(
            "owned chain workflow required",
        ));
    }
    measure_probe(iterations, || {
        let mut doc = Document::new(tolerance);
        let ids = source(&f.scene, tolerance)?
            .into_iter()
            .map(|g| doc.add_geometry(g))
            .collect::<Result<Vec<_>, _>>()?;
        doc.clear_history()?;
        let original = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        let pre = f
            .pre
            .iter()
            .map(|&i| {
                ids.get(i).copied().ok_or(ProbeError::FixtureInvariant(
                    "chain preselection outside sources",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        doc.select_objects_direct(pre, SelectionMode::Add)?;
        let mut opts = defaults(&doc);
        for (key, value) in &f.options {
            let key = match key.as_str() {
                "continuity" => "ChainContinuity",
                "direction" => "Direction",
                "gap" => "GapTolerance",
                "angle" => "AngleTolerance",
                _ => return Err(ProbeError::FixtureInvariant("owned chain option required")),
            };
            opts = parse_options(&format!("{key}={value}"), opts, tolerance)?;
        }
        let before = snapshot(&doc, &registry, &ids)?;
        let mut session = Session::new(&doc, registry.component_selection(), opts)?;
        let mut trace = Vec::new();
        let mut defer_apply = false;
        for step in &f.steps {
            let mut transparent = false;
            match step.kind.as_str() {
                "click" => {
                    let i = step
                        .source
                        .or(f.seed)
                        .ok_or(ProbeError::FixtureInvariant("chain seed required"))?;
                    let id = *ids
                        .get(i)
                        .ok_or(ProbeError::FixtureInvariant("chain seed outside sources"))?;
                    let target = if doc.object(id).unwrap().geometry().curve_ref().is_some() {
                        Target::Curve(id)
                    } else {
                        Target::Edge(id, 0)
                    };
                    session.pick_input(&doc, target)?;
                    defer_apply = false;
                }
                "key" => match step.value.as_deref() {
                    Some("Enter") => {
                        let done = session.enter(&doc)?;
                        transparent = defer_apply && !done;
                    }
                    Some("Undo") if session.choosing_branch() => {
                        let _ = doc.undo();
                        transparent = true;
                    }
                    Some("All") if session.choosing_branch() => {
                        registry.execute(&mut doc, "SelAll")?;
                        defer_apply = true;
                        transparent = true;
                    }
                    Some("Next") if session.choosing_branch() => {
                        transparent = true;
                    }
                    Some("Undo") => session.undo(&doc)?,
                    Some("Next") => session.extend(&doc, true)?,
                    Some("All") => session.extend(&doc, false)?,
                    Some(s) => {
                        let opts = parse_options(s, session.options(), tolerance)?;
                        session.set_options(opts)?;
                    }
                    None => return Err(ProbeError::FixtureInvariant("chain input required")),
                },
                _ => return Err(ProbeError::FixtureInvariant("owned chain step required")),
            }
            if !transparent {
                session.apply(&mut doc, registry.component_selection())?;
            }
            trace.push(snapshot(&doc, &registry, &ids)?);
        }
        if f.finish == "Cancel" {
            session.cancel(&mut doc, registry.component_selection())?;
        } else {
            if !session.enter(&doc)? {
                return Err(ProbeError::FixtureInvariant(
                    "confirm the chain proposal before finishing",
                ));
            }
            session.apply(&mut doc, registry.component_selection())?;
        }
        let after = snapshot(&doc, &registry, &ids)?;
        registry.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &registry, &ids)?;
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &registry, &ids)?;
        Ok(
            json!({"before":before,"trace":trace,"after":after,"redo":redo,"undo":undo,"success":f.finish=="Enter" && session.has_seed(),"source_unchanged":ids.iter().zip(&original).all(|(&id,o)|doc.object(id)==Some(o))}),
        )
    })
}
