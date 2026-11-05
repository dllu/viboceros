//! Independent SubD volume inputs and actual command, selection and history paths.
use super::*;
use viboceros_geometry::SubD;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub scene: String,
    pub command: String,
    pub flat: bool,
    pub preselect: bool,
    pub mixed: bool,
    #[serde(default)]
    pub replace_reversed: bool,
    pub open_confirmation: Option<String>,
}
pub fn source(scene: &str, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
    if scene == "bent_grid" {
        return component_length::source("subd", scene, tolerance).map(|(g, _)| g);
    }
    let mut subd = if scene == "bipyramid7" {
        let mut points = vec![[0.3, -0.2, 4.], [0., 0., -2.]];
        for i in 0..7 {
            let a = std::f64::consts::TAU * i as f64 / 7.;
            points.push([3. * a.cos(), 2.5 * a.sin(), 0.12 * (3. * a).sin()]);
        }
        let faces = (0..7)
            .map(|i| vec![0, 2 + i, 2 + (i + 1) % 7])
            .chain((0..7).map(|i| vec![1, 2 + (i + 1) % 7, 2 + i]))
            .collect();
        SubD::try_from_polygons(
            points
                .into_iter()
                .map(Point3::try_from)
                .collect::<Result<_, _>>()?,
            faces,
        )?
    } else {
        let (Geometry::SubD(s), _) = component_length::source("subd", "cube", tolerance)? else {
            unreachable!()
        };
        s
    };
    if matches!(scene, "open_cube" | "reversed_open") {
        let mut faces = subd.faces().to_vec();
        faces.remove(1);
        subd = SubD::try_from_polygons(subd.control_points().to_vec(), faces)?;
    } else if matches!(scene, "skew_cube" | "translated_cube" | "scaled_cube") {
        let mut p = subd
            .control_points()
            .iter()
            .map(|p| p.to_array())
            .collect::<Vec<_>>();
        p[0] = [-0.4, 0.2, -0.1];
        p[6] = [4.5, 2.6, 5.8];
        for p in &mut p {
            if scene == "translated_cube" {
                p[0] += 100.;
                p[1] -= 200.;
                p[2] += 300.;
            } else if scene == "scaled_cube" {
                for c in p {
                    *c *= 2.;
                }
            }
        }
        subd = subd.try_with_control_points(
            &p.into_iter()
                .map(Point3::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
    } else if matches!(scene, "creased_cube" | "sharp_cube") {
        let pairs = if scene == "sharp_cube" {
            subd.edges().to_vec()
        } else {
            vec![[0, 1], [1, 2], [2, 3], [0, 3]]
        };
        let tags = subd
            .edges()
            .iter()
            .enumerate()
            .filter(|(_, e)| pairs.iter().any(|p| **e == *p || **e == [p[1], p[0]]))
            .map(|(i, _)| (i, 2))
            .collect::<Vec<_>>();
        subd = subd.try_with_edge_tags(&tags)?;
        if scene == "sharp_cube" {
            subd = subd.try_with_vertex_tags(&(0..8).map(|i| (i, 3)).collect::<Vec<_>>())?;
        }
    } else if !matches!(scene, "cube" | "reversed_cube" | "bipyramid7") {
        return Err(ProbeError::FixtureInvariant(
            "owned SubD volume source required",
        ));
    }
    if matches!(scene, "reversed_cube" | "reversed_open") {
        subd = subd.reversed()?;
    }
    Ok(Geometry::SubD(subd))
}
pub fn definition(geometry: &Geometry) -> Value {
    let Geometry::SubD(s) = geometry else {
        unreachable!()
    };
    json!({"points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"faces":s.faces(),"vertex_tags":s.vertex_tags()})
}
pub fn snapshot(doc: &Document, keys: &[ObjectId]) -> Value {
    json!({"selected":keys.iter().map(|id|doc.is_selected(*id)).collect::<Vec<_>>(),"objects":doc.objects().len(),"points":doc.objects().filter_map(|o|match o.geometry(){Geometry::Point(p)=>Some(p.to_array()),_=>None}).collect::<Vec<_>>()})
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !matches!(f.command.as_str(), "Volume" | "VolumeCentroid")
        || f.open_confirmation
            .as_ref()
            .is_some_and(|v| !matches!(v.as_str(), "yes" | "no" | "escape"))
    {
        return Err(ProbeError::FixtureInvariant(
            "owned volume command required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let geometry = source(&f.scene, tolerance)?;
        let constructed = definition(&geometry);
        let owner = doc.add_geometry(geometry)?;
        let replacement = if f.replace_reversed {
            let Geometry::SubD(s) = doc.object(owner).unwrap().geometry() else {
                unreachable!()
            };
            let candidate = Geometry::SubD(s.reversed()?);
            let record = definition(&candidate);
            doc.replace_object_geometries([(owner, candidate)])?;
            Some(record)
        } else {
            None
        };
        let stored = definition(doc.object(owner).unwrap().geometry());
        let mut keys = vec![owner];
        if f.mixed {
            registry.execute(
                &mut doc,
                "MeshBox 20,0,0 22,3,0 4 XCount=1 YCount=1 ZCount=1",
            )?;
            keys.push(doc.objects().last().unwrap().id());
        }
        if f.flat {
            registry.execute(&mut doc, "SubDDisplayToggle")?;
        }
        let sources = doc.objects().cloned().collect::<Vec<_>>();
        doc.clear_selection();
        doc.clear_history()?;
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        if f.preselect {
            doc.select_objects_direct(keys.iter().copied(), SelectionMode::Add)?;
        }
        let before = snapshot(&doc, &keys);
        if !f.preselect {
            doc.select_objects_direct(keys.iter().copied(), SelectionMode::Add)?;
        }
        let prompt = registry.object_selection_prompt(&f.command)?.unwrap();
        let warning = registry
            .object_selection_confirmation(&doc, &prompt)?
            .is_some();
        let command = match f.open_confirmation.as_deref() {
            Some("no") => format!("{} Continue=No", f.command),
            Some(_) => format!("{} Continue=Yes", f.command),
            None => f.command.clone(),
        };
        let result = if f.preselect {
            registry.execute(&mut doc, &command)
        } else {
            registry.execute_postselected(&mut doc, &command, Default::default())
        };
        let report = match result {
            Ok(report) => Some(report),
            Err(CommandError::OperationDeclined) => None,
            Err(e) => return Err(e.into()),
        };
        let volume = if f.command == "Volume" {
            report
                .as_ref()
                .map(|s| {
                    s.split_once("total volume ")
                        .and_then(|(_, s)| s.split_whitespace().next())
                        .and_then(|s| s.parse::<f64>().ok())
                        .ok_or(ProbeError::FixtureInvariant(
                            "numeric volume report required",
                        ))
                })
                .transpose()?
        } else {
            None
        };
        let after = snapshot(&doc, &keys);
        let (undo, redo) = if f.command == "VolumeCentroid"
            && doc
                .objects()
                .any(|o| matches!(o.geometry(), Geometry::Point(_)))
        {
            registry.execute(&mut doc, "Undo")?;
            let undo = snapshot(&doc, &keys);
            registry.execute(&mut doc, "Redo")?;
            (undo, snapshot(&doc, &keys))
        } else {
            registry.execute(&mut doc, "Redo")?;
            let redo = snapshot(&doc, &keys);
            registry.execute(&mut doc, "Undo")?;
            (snapshot(&doc, &keys), redo)
        };
        let unchanged = sources
            .iter()
            .all(|o| doc.object(o.id()).is_some_and(|current| current == o));
        Ok(
            json!({"constructed":constructed,"stored":stored,"replacement":replacement,"source_unchanged":unchanged,"success":report.is_some(),"before":before,"after":after,"undo":undo,"redo":redo,"warning":warning,"volume":volume}),
        )
    })
}
