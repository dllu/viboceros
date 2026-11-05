//! Independent polygon recipes for mesh/SubD edge measurement and selection.
use super::*;
use viboceros_command::{ComponentPick, ComponentSelectionKind as Kind};
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub kind: String,
    pub scene: String,
    pub pairs: Vec<[usize; 2]>,
    pub flat: bool,
    pub curve: bool,
    #[serde(default)]
    pub steps: Vec<Step>,
    pub finish: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Step {
    pub kind: String,
    pub modifiers: String,
    pub target: Option<String>,
}
pub fn source(
    kind: &str,
    scene: &str,
    tolerance: Tolerance,
) -> Result<(Geometry, Vec<Point3>), ProbeError> {
    let (mut points, faces, corners, creases) = match scene {
        "grid" | "bent_grid" | "creased_grid" => {
            let points = (0..4)
                .flat_map(|y| (0..5).map(move |x| [3. * x as f64, 3. * y as f64, 0.]))
                .collect();
            let faces = (0..3)
                .flat_map(|y| {
                    (0..4).map(move |x| {
                        let a = 5 * y + x;
                        vec![a, a + 1, a + 6, a + 5]
                    })
                })
                .collect();
            (
                points,
                faces,
                vec![0, 4, 15, 19],
                if scene == "creased_grid" {
                    (5..9).map(|i| [i, i + 1]).collect()
                } else {
                    vec![]
                },
            )
        }
        "fan5" | "raised_fan" => {
            let mut points = vec![[0., 0., 0.]];
            for (r, offset) in [(3., 0.), (6., 0.5)] {
                for i in 0..5 {
                    let angle = std::f64::consts::TAU * (i as f64 + offset) / 5.;
                    points.push([r * angle.cos(), r * angle.sin(), 0.]);
                }
            }
            (
                points,
                (0..5)
                    .map(|i| vec![0, i + 1, 5 + i + 1, (i + 1) % 5 + 1])
                    .collect(),
                vec![],
                vec![],
            )
        }
        "cube" => (
            vec![
                [0., 0., 0.],
                [4., 0., 0.],
                [4., 3., 0.],
                [0., 3., 0.],
                [0., 0., 5.],
                [4., 0., 5.],
                [4., 3., 5.],
                [0., 3., 5.],
            ],
            vec![
                vec![0, 3, 2, 1],
                vec![4, 5, 6, 7],
                vec![0, 1, 5, 4],
                vec![1, 2, 6, 5],
                vec![2, 3, 7, 6],
                vec![3, 0, 4, 7],
            ],
            vec![],
            vec![],
        ),
        "surface" => (
            vec![[0., 0., 0.], [3., 0., 0.], [3., 3., 0.], [0., 3., 0.]],
            vec![vec![0, 1, 2, 3]],
            vec![],
            vec![],
        ),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "owned component length scene required",
            ));
        }
    };
    if matches!(scene, "bent_grid" | "creased_grid") {
        for [x, y, z] in &mut points {
            *z = 0.04 * (*x) * (*x) + 0.07 * (*y) * (*y) + 0.03 * (*x) * (*y);
        }
    }
    if scene == "raised_fan" {
        for (i, [x, y, z]) in points.iter_mut().enumerate() {
            *z = if i == 0 { 2. } else { 0.05 * (*x) * (*y) };
        }
    }
    if kind == "brep" {
        if scene != "surface" {
            return Err(ProbeError::FixtureInvariant(
                "owned length surface required",
            ));
        }
        let p = points
            .into_iter()
            .map(Point3::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let surface = NurbsSurface::try_bilinear([p[0], p[1], p[2], p[3]])?;
        return Ok((
            Geometry::Brep(Brep::try_surface_face(surface, tolerance)?),
            p,
        ));
    }
    edge_pattern_source::from_parts(kind, points, faces, corners, creases, vec![], tolerance)
}
pub fn edges(geometry: &Geometry, points: &[Point3]) -> Vec<[usize; 2]> {
    let lookup = points
        .iter()
        .enumerate()
        .map(|(i, p)| (p.to_array().map(f64::to_bits), i))
        .collect::<BTreeMap<_, _>>();
    let index = |p: Point3| lookup[&p.to_array().map(f64::to_bits)];
    match geometry {
        Geometry::Mesh(m) => m
            .topology_edge_points()
            .into_iter()
            .map(|p| p.map(index))
            .collect(),
        Geometry::SubD(s) => s
            .edges()
            .iter()
            .map(|e| e.map(|i| index(s.control_points()[i as usize])))
            .collect(),
        Geometry::Brep(b) => b
            .edges()
            .iter()
            .map(|e| {
                let c = e.curve();
                let d = c.domain();
                [
                    index(c.evaluate(*d.start()).unwrap()),
                    index(c.evaluate(*d.end()).unwrap()),
                ]
            })
            .collect(),
        _ => unreachable!(),
    }
}
fn snapshot(
    doc: &Document,
    registry: &CommandRegistry,
    owner: ObjectId,
    curve: Option<ObjectId>,
    native: &[usize],
    kind: Kind,
) -> Result<Value, ProbeError> {
    let picks = registry
        .component_selection()
        .checked_picks(doc)
        .map_err(ProbeError::FixtureInvariant)?;
    let kind = match kind {
        Kind::MeshEdge => "MeshTopologyEdge",
        Kind::SubDEdge => "SubdEdge",
        _ => "BrepEdge",
    };
    let mut components = picks
        .into_iter()
        .map(|p| json!({"kind":kind,"index":native[p.index]}))
        .collect::<Vec<_>>();
    components.sort_by_key(|p| p["index"].as_u64().unwrap());
    Ok(
        json!({"whole":doc.is_selected(owner),"curve":curve.is_some_and(|id|doc.is_selected(id)),"objects":doc.objects().len(),"components":components}),
    )
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.pairs.is_empty()
        || f.pairs.len() > 64
        || f.steps.len() > 24
        || f.finish
            .as_ref()
            .is_some_and(|s| !matches!(s.as_str(), "Enter" | "Cancel"))
    {
        return Err(ProbeError::FixtureInvariant("owned edge pairs required"));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (geometry, points) = source(&f.kind, &f.scene, tolerance)?;
        let edges = edges(&geometry, &points);
        let kind = match &geometry {
            Geometry::Mesh(_) => Kind::MeshEdge,
            Geometry::SubD(_) => Kind::SubDEdge,
            _ => Kind::BrepEdge,
        };
        let native = if let Geometry::SubD(s) = &geometry {
            s.component_ids()?
                .edges
                .into_iter()
                .map(|i| i as usize)
                .collect::<Vec<_>>()
        } else {
            (0..edges.len()).collect()
        };
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        if f.flat {
            registry.execute(&mut doc, "SubDDisplayToggle")?;
        }
        let owner = doc.add_geometry(geometry)?;
        let curve = if f.curve {
            registry.execute(&mut doc, "Line 30,0,0 33,0,0")?;
            Some(doc.objects().last().unwrap().id())
        } else {
            None
        };
        doc.clear_selection();
        doc.clear_history()?;
        let originals = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        let selected = f
            .pairs
            .iter()
            .map(|&p| {
                edges
                    .iter()
                    .position(|e| *e == p || *e == [p[1], p[0]])
                    .ok_or(ProbeError::FixtureInvariant("owned edge pair required"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let postselected = !f.steps.is_empty();
        if !postselected {
            registry
                .component_selection()
                .replace(
                    &doc,
                    selected
                        .iter()
                        .map(|&index| ComponentPick {
                            object: owner,
                            index,
                            kind,
                        })
                        .collect(),
                )
                .map_err(ProbeError::FixtureInvariant)?;
        }
        if let Some(id) = curve
            && !postselected
        {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
        let before = snapshot(&doc, &registry, owner, curve, &native, kind)?;
        let mut trace = Vec::new();
        for step in &f.steps {
            if step.kind != "click" || !matches!(step.modifiers.as_str(), "sub" | "plain" | "ctrl")
            {
                return Err(ProbeError::FixtureInvariant("owned edge click required"));
            }
            let remove = step.modifiers == "ctrl";
            if step.target.as_deref() == Some("curve") {
                doc.select_objects_direct(
                    [curve.ok_or(ProbeError::FixtureInvariant("owned curve required"))?],
                    if remove {
                        SelectionMode::Remove
                    } else {
                        SelectionMode::Add
                    },
                )?;
            } else {
                registry
                    .component_selection()
                    .apply(
                        &doc,
                        vec![ComponentPick {
                            object: owner,
                            index: selected[0],
                            kind,
                        }],
                        if remove {
                            viboceros_command::component_selection::ComponentAction::Remove
                        } else {
                            viboceros_command::component_selection::ComponentAction::Add
                        },
                    )
                    .map_err(ProbeError::FixtureInvariant)?;
            }
            trace.push(snapshot(&doc, &registry, owner, curve, &native, kind)?);
        }
        let report = if postselected && f.finish.as_deref() == Some("Cancel") {
            doc.clear_selection();
            registry.component_selection().clear();
            None
        } else {
            Some(if postselected {
                registry.execute_postselected(&mut doc, "Length", Default::default())?
            } else {
                registry.execute(&mut doc, "Length")?
            })
        };
        let length = report
            .as_ref()
            .map(|report| {
                report
                    .split_once("total length ")
                    .and_then(|(_, s)| s.split_whitespace().next())
                    .and_then(|s| s.parse::<f64>().ok())
                    .ok_or(ProbeError::FixtureInvariant(
                        "numeric length report required",
                    ))
            })
            .transpose()?;
        let after = snapshot(&doc, &registry, owner, curve, &native, kind)?;
        registry.execute(&mut doc, "Redo")?;
        let redo = snapshot(&doc, &registry, owner, curve, &native, kind)?;
        registry.execute(&mut doc, "Undo")?;
        let undo = snapshot(&doc, &registry, owner, curve, &native, kind)?;
        Ok(
            json!({"before":before,"trace":trace,"after":after,"redo":redo,"undo":undo,"success":report.is_some(),"length_mm":length,"source_unchanged":doc.objects().eq(originals.iter())}),
        )
    })
}
