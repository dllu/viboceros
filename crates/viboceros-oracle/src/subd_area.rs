//! Independent sources and actual command/history execution for SubD area.
use super::*;
use viboceros_geometry::SubD;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub scene: String,
    pub flat: bool,
    #[serde(default)]
    pub curve: bool,
    #[serde(default = "preselected")]
    pub preselect: bool,
}
fn preselected() -> bool {
    true
}
pub fn source(scene: &str, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
    let (points, faces, corners) = if scene == "pentagon" {
        (
            (0..5)
                .map(|i| {
                    let a = std::f64::consts::TAU * i as f64 / 5.;
                    [3. * a.cos(), 3. * a.sin(), 0.4 * (2. * a).sin()]
                })
                .collect::<Vec<_>>(),
            vec![(0..5).collect::<Vec<_>>()],
            (0..5).collect::<Vec<_>>(),
        )
    } else if matches!(scene, "fan7" | "asymmetric8") {
        let n = if scene == "fan7" { 7 } else { 8 };
        let mut points = vec![[0., 0., 0.6]];
        for i in 0..n {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            points.push([1.5 * a.cos(), 1.5 * a.sin(), 0.15 * (2. * a).sin()]);
        }
        for i in 0..n {
            let a = std::f64::consts::TAU * (i as f64 + 0.5) / n as f64;
            points.push([3. * a.cos(), 3. * a.sin(), 0.45 * (3. * a).cos()]);
        }
        if scene == "asymmetric8" {
            points[1] = [1.2, 0.2, 0.7];
            points[n + 2][2] += 0.35;
        }
        (
            points,
            (0..n)
                .map(|i| {
                    vec![
                        0,
                        (1 + i) as u32,
                        (1 + n + i) as u32,
                        (1 + (i + 1) % n) as u32,
                    ]
                })
                .collect(),
            (n + 1..2 * n + 1).collect(),
        )
    } else {
        return component_length::source("subd", scene, tolerance).map(|(g, _)| g);
    };
    let s = SubD::try_from_polygons(
        points
            .into_iter()
            .map(Point3::try_from)
            .collect::<Result<_, _>>()?,
        faces,
    )?
    .try_with_vertex_tags(&corners.into_iter().map(|i| (i, 3)).collect::<Vec<_>>())?;
    Ok(Geometry::SubD(s))
}
pub fn snapshot(doc: &Document, owner: ObjectId) -> Value {
    let points = doc
        .objects()
        .filter_map(|o| match o.geometry() {
            Geometry::Point(p) => Some(p.to_array()),
            _ => None,
        })
        .collect::<Vec<_>>();
    json!({"selected":doc.is_selected(owner),"objects":doc.objects().len(),"points":points})
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let source = source(&f.scene, tolerance)?;
        let mut commands = Vec::new();
        for command in ["Area", "AreaCentroid"] {
            let mut doc = Document::new(tolerance);
            let registry = CommandRegistry::with_builtins();
            let owner = doc.add_geometry(source.clone())?;
            if f.flat {
                registry.execute(&mut doc, "SubDDisplayToggle")?;
            }
            if f.curve {
                registry.execute(&mut doc, "Circle 20,0,0 2")?;
            }
            let sources = doc.objects().cloned().collect::<Vec<_>>();
            doc.clear_selection();
            doc.clear_history()?;
            registry.execute(&mut doc, "Point 40,50,60")?;
            registry.execute(&mut doc, "Undo")?;
            if f.preselect {
                doc.select_objects_direct(sources.iter().map(|o| o.id()), SelectionMode::Add)?;
            }
            let before = snapshot(&doc, owner);
            let report = if f.preselect {
                registry.execute(&mut doc, command)?
            } else {
                doc.select_objects_direct([owner], SelectionMode::Add)?;
                registry.execute_postselected(&mut doc, command, Default::default())?
            };
            let area = if command == "Area" {
                Some(
                    report
                        .split_once("total area ")
                        .and_then(|(_, s)| s.split_whitespace().next())
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or(ProbeError::FixtureInvariant("numeric area report required"))?,
                )
            } else {
                None
            };
            let after = snapshot(&doc, owner);
            let (undo, redo) = if command == "Area" {
                registry.execute(&mut doc, "Redo")?;
                let redo = snapshot(&doc, owner);
                registry.execute(&mut doc, "Undo")?;
                (snapshot(&doc, owner), redo)
            } else {
                registry.execute(&mut doc, "Undo")?;
                let undo = snapshot(&doc, owner);
                registry.execute(&mut doc, "Redo")?;
                (undo, snapshot(&doc, owner))
            };
            let source_unchanged = sources
                .iter()
                .all(|o| doc.object(o.id()).is_some_and(|current| current == o));
            commands.push(json!({"command":command,"success":true,"area":area,"before":before,"after":after,"undo":undo,"redo":redo,"source_unchanged":source_unchanged}));
        }
        let source_unchanged = commands.iter().all(|c| c["source_unchanged"] == true);
        Ok(json!({"commands":commands,"source_unchanged":source_unchanged}))
    })
}
