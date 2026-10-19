//! Complete Delete/Undo/Redo replay over authored, shared grip-owner models.
use super::*;
use viboceros_document::{ControlPointId, GroupId};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    sources: Vec<Source>,
    peer: bool,
    off: bool,
    artifact_path: PathBuf,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Source {
    kind: String,
    mode: String,
    selected: String,
    hidden: String,
    parent: bool,
}
pub struct Prepared {
    pub document: Document,
    pub ids: Vec<ObjectId>,
    pub peer: ObjectId,
    pub marker: ObjectId,
    pub group: GroupId,
}
fn picks(seed: &str, n: usize, dimensions: Option<[usize; 2]>) -> Result<Vec<usize>, ProbeError> {
    if n == 0 {
        return Ok(vec![]);
    }
    Ok(match seed {
        "first" => vec![0],
        "last" => vec![n - 1],
        "middle" => vec![n / 2],
        "second" => vec![1.min(n - 1)],
        "before_last" => vec![n.saturating_sub(2)],
        "ends" => BTreeSet::from([0, n - 1]).into_iter().collect(),
        "all" => (0..n).collect(),
        "empty" => vec![],
        "alternate" => (0..n).step_by(2).collect(),
        "adjacent" => BTreeSet::from([(n / 2).saturating_sub(1), n / 2])
            .into_iter()
            .collect(),
        "except_first" => (1..n).collect(),
        "except_ends" => (1..n - 1).collect(),
        "neighbor_hidden" | "next_hidden" => vec![(n / 2).saturating_sub(1)],
        "row_u" | "row_v" | "cross_rows" => {
            let [u, v] = dimensions.ok_or(ProbeError::FixtureInvariant(
                "Delete surface row requires a surface",
            ))?;
            (0..u * v)
                .filter(|i| {
                    (seed != "row_v" && i % v == v / 2) || (seed != "row_u" && i / v == u / 2)
                })
                .collect()
        }
        _ => return Err(ProbeError::FixtureInvariant("invalid Delete grip seed")),
    })
}
impl Fixture {
    pub(super) fn points_off(&self) -> bool {
        self.off
    }
    pub(super) fn rebase(&mut self, folder: &Path) {
        self.artifact_path = folder.join(self.artifact_path.file_name().unwrap());
    }
    fn validate(&self) -> Result<(), ProbeError> {
        const KINDS: &[&str] = &[
            "curve",
            "rational",
            "closed",
            "periodic",
            "line",
            "polyline",
            "circle",
            "arc",
            "surface",
            "surface_closed",
            "mesh",
            "polycurve",
            "surface_periodic_u",
            "surface_periodic_v",
            "cubic",
            "quartic",
            "nonuniform",
            "weighted",
            "periodic_cubic",
            "closed_weighted",
            "surface_wide",
            "surface_bicubic",
            "mesh_fan",
            "mesh_grid",
            "mesh_box",
            "polycurve_curved",
            "mesh_colored",
            "mesh_ngon",
            "periodic_min",
        ];
        if !(1..=4).contains(&self.sources.len()) {
            return Err(ProbeError::FixtureInvariant("Delete source count"));
        }
        for s in &self.sources {
            if !KINDS.contains(&s.kind.as_str())
                || !matches!(s.mode.as_str(), "control" | "edit")
                || (s.mode == "edit"
                    && (s.kind.starts_with("surface") || s.kind.starts_with("mesh")))
                || !matches!(s.hidden.as_str(), "empty" | "first" | "middle" | "all")
                || (matches!(s.selected.as_str(), "row_u" | "row_v" | "cross_rows")
                    && !s.kind.starts_with("surface"))
                || (matches!(s.selected.as_str(), "neighbor_hidden" | "next_hidden")
                    && (!matches!(s.kind.as_str(), "curve" | "rational" | "periodic")
                        || s.mode != "control"
                        || s.hidden != "middle"))
            {
                return Err(ProbeError::FixtureInvariant("invalid Delete grip source"));
            }
            picks(&s.selected, 4, Some([2, 2]))?;
        }
        Ok(())
    }
}
pub fn prepare(f: &Fixture) -> Result<Prepared, ProbeError> {
    f.validate()?;
    let (mut doc, _, _, _) = viboceros_command::open_3dm_with_views(
        f.artifact_path
            .to_str()
            .ok_or(ProbeError::FixtureInvariant("Delete path"))?,
    )?;
    let all = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    if all.len() != f.sources.len() + 1 {
        return Err(ProbeError::FixtureInvariant("Delete model source count"));
    }
    let (ids, peer) = (all[..f.sources.len()].to_vec(), *all.last().unwrap());
    let group = doc.add_group(Some("Delete grips".into()), all)?;
    let registry = CommandRegistry::with_builtins();
    for (&id, s) in ids.iter().zip(&f.sources) {
        if s.mode == "edit" {
            doc.select_objects_direct([id], SelectionMode::Add)?;
            registry.execute(&mut doc, "EditPtOn")?;
        } else {
            doc.enable_control_points([id])?;
        }
    }
    doc.clear_history()?;
    let marker = doc.add_geometry(Geometry::Point(Point3::try_new(30., 30., 30.)?))?;
    doc.undo()?;
    doc.clear_selection();
    for (&id, s) in ids.iter().zip(&f.sources) {
        let n = doc
            .control_point_locations(id)
            .ok_or(ProbeError::FixtureInvariant("initial Delete display"))?
            .len();
        doc.set_control_points_hidden(
            picks(&s.hidden, n, None)?
                .into_iter()
                .map(|index| ControlPointId { object: id, index }),
            true,
        )?;
    }
    for (&id, s) in ids.iter().zip(&f.sources) {
        let n = doc.control_point_locations(id).unwrap().len();
        let dimensions = match doc.object(id).unwrap().geometry() {
            Geometry::NurbsSurface(s) => Some(s.grip_dimensions()),
            Geometry::Brep(b) if b.faces().len() == 1 => {
                Some(b.faces()[0].surface().grip_dimensions())
            }
            _ => None,
        };
        let picked = picks(&s.selected, n, dimensions)?
            .into_iter()
            .map(|index| ControlPointId { object: id, index })
            .filter(|g| doc.is_control_point_selectable(*g))
            .collect::<Vec<_>>();
        doc.select_control_points(picked, SelectionMode::Add)?;
        if s.selected == "neighbor_hidden" {
            registry.execute(&mut doc, "AddNextU")?;
        }
        if s.selected == "next_hidden" {
            registry.execute(&mut doc, "NextU")?;
        }
        if s.parent {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
    }
    if f.peer {
        doc.select_objects_direct([peer], SelectionMode::Add)?;
    }
    Ok(Prepared {
        document: doc,
        ids,
        peer,
        marker,
        group,
    })
}
pub fn snapshot(
    doc: &Document,
    ids: &[ObjectId],
    peer: ObjectId,
    marker: ObjectId,
    group: GroupId,
) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for object in doc.objects().filter(|o| o.id() != marker) {
        let name = object
            .attributes()
            .name()
            .ok_or(ProbeError::FixtureInvariant("unnamed Delete object"))?;
        let expected = if name == "peer" {
            peer
        } else {
            let i = name
                .strip_prefix("source")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|i| ids.get(i));
            *i.ok_or(ProbeError::FixtureInvariant("unexpected Delete role"))?
        };
        let id = object.id();
        let g = object.geometry();
        let color = object.attributes().object_color();
        let kind = match g {
            Geometry::Line(_) => "LineCurve",
            Geometry::Polyline(_) => "PolylineCurve",
            Geometry::PolyCurve(_) => "PolyCurve",
            Geometry::Circle(_) | Geometry::Arc(_) => "ArcCurve",
            Geometry::NurbsCurve(_) => "NurbsCurve",
            Geometry::NurbsSurface(_) | Geometry::Brep(_) => "Brep",
            Geometry::Mesh(_) => "Mesh",
            Geometry::Point(_) => "Point",
            _ => return Err(ProbeError::FixtureInvariant("unexpected Delete geometry")),
        };
        let mut row = json!({"role":name,"same_id":id==expected,"name":name,"color":[color.red,color.green,color.blue],
            "grouped":object.group_ids().contains(&group),"selected":doc.is_selected(id),"visible":object.attributes().is_visible(),
            "grips_on":doc.grip_mode(id).is_some(),"kind":kind,
            "grips":doc.all_control_points().filter(|(p,_,_)|p.object==id).map(|(p,point,selected)|json!({"index":p.index,"point":point.to_array(),"selected":selected,"hidden":doc.is_control_point_hidden(p).unwrap(),"selectable":doc.is_control_point_selectable(p)})).collect::<Vec<_>>()});
        match g {
            Geometry::Point(p) => row["point"] = json!(p.to_array()),
            Geometry::Mesh(m) => {
                row["mesh"] = polygon_mesh_value(m);
                row["mesh"]["ngons"] = json!(
                    m.ngons()
                        .iter()
                        .map(|n| json!({"vertices":n.vertices(),"faces":n.faces()}))
                        .collect::<Vec<_>>()
                );
                // RhinoCommon exposes opacity; the kernel stores OpenNURBS transparency.
                row["mesh"]["colors"] = json!(m.vertex_colors().map(|v| {
                    v.iter()
                        .map(|c| [c[0], c[1], c[2], 255 - c[3]])
                        .collect::<Vec<_>>()
                }));
            }
            Geometry::NurbsSurface(s) => row["surface"] = nurbs_surface_definition_value(s),
            Geometry::Brep(b) if b.faces().len() == 1 => {
                row["surface"] = nurbs_surface_definition_value(b.faces()[0].surface())
            }
            _ => {
                row["curve"] = nurbs_curve_definition_value(
                    &g.curve_ref()
                        .ok_or(ProbeError::FixtureInvariant("Delete curve"))?
                        .to_nurbs()?,
                )
            }
        }
        if row.get("curve").is_some() {
            native_outer_knots(&mut row["curve"], "knots");
        }
        if row.get("surface").is_some() {
            native_outer_knots(&mut row["surface"], "knots_u");
            native_outer_knots(&mut row["surface"], "knots_v");
        }
        rows.push(row);
    }
    Ok(json!({"objects":rows,"marker":doc.object(marker).is_some()}))
}
fn native_outer_knots(definition: &mut Value, key: &str) {
    // The public OpenNURBS vector omits two artificial outer slots. The native
    // protocol repeats its first/last values; all represented knots stay exact.
    let a = definition[key].as_array_mut().unwrap();
    let last = a.len() - 1;
    a[0] = a[1].clone();
    a[last] = a[last - 1].clone();
}
pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    f.validate()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let Prepared {
            document: mut doc,
            ids,
            peer,
            marker,
            group,
        } = prepare(f)?;
        let registry = CommandRegistry::with_builtins();
        let snap = |d: &Document| snapshot(d, &ids, peer, marker, group);
        let before = snap(&doc)?;
        let mut steps = Vec::new();
        for command in std::iter::once("Delete")
            .chain(f.off.then_some("PointsOff"))
            .chain(["Undo", "Redo"])
        {
            let initial = snap(&doc)?;
            let counts = (doc.objects().count(), doc.all_control_points().count());
            registry.execute(&mut doc, command)?;
            let result = if command == "Delete"
                && doc.objects().count() >= counts.0
                && doc.all_control_points().count() >= counts.1
            {
                "Nothing"
            } else {
                "Success"
            };
            let after = snap(&doc)?;
            steps.push(json!({"command":command,"before":initial,"after":after,"after_script":after,"result":result}));
        }
        Ok(json!({"before":before,"steps":steps}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_native_delete_grips_and_history() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/rhino_oracle");
        let mut q: ProbeRequest = serde_json::from_str(
            &fs::read_to_string(root.join("fixtures/delete_grips.json")).unwrap(),
        )
        .unwrap();
        for op in &mut q.operations {
            if let Operation::DeleteGrips { fixture, .. } = op {
                fixture.artifact_path = root
                    .join("fixtures/delete_grips")
                    .join(fixture.artifact_path.file_name().unwrap());
            }
        }
        let expected: ProbeResponse = serde_json::from_str(
            &fs::read_to_string(root.join("observations/delete_grips.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(expected.results.len(), 397);
        assert_eq!(q.operations.len(), 397);
        let mut single = q.clone();
        let mut errors = Vec::new();
        for (op, e) in q.operations.iter().zip(expected.results) {
            single.operations = vec![op.clone()];
            match run_request(&single) {
                Ok(actual) => {
                    assert_eq!(actual.results[0].id, e.id);
                    if std::panic::catch_unwind(|| {
                        crate::test_json::close(
                            &actual.results[0].value,
                            &e.value,
                            &e.id,
                            1e-8,
                            1e-12,
                        )
                    })
                    .is_err()
                    {
                        errors.push(e.id);
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", op.id())),
            }
        }
        assert!(errors.is_empty(), "Delete differences: {errors:?}");
    }
}
