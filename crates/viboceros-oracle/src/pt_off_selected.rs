//! Selected point-display command replay using the production registry and filters.
use super::*;
use viboceros_document::ControlPointId;
use viboceros_geometry::SubD;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fixture {
    pub kind: String,
    pub mode: String,
    pub pattern: String,
}
impl Fixture {
    fn validate(&self) -> Result<(), ProbeError> {
        if !matches!(
            self.kind.as_str(),
            "line" | "curve" | "rational" | "surface" | "mesh" | "subd"
        ) || !matches!(self.mode.as_str(), "control" | "edit")
            || (self.mode == "edit" && matches!(self.kind.as_str(), "surface" | "mesh"))
            || !matches!(
                self.pattern.as_str(),
                "owner"
                    | "grip"
                    | "both-owners"
                    | "both-grips"
                    | "none"
                    | "peer"
                    | "mixed"
                    | "group-owner"
                    | "group-grip"
                    | "owner-peer"
                    | "off-owner"
                    | "postselect"
                    | "owner-hidden-peer"
                    | "postselect-peer"
                    | "postselect-grip"
                    | "postcancel-peer"
                    | "postcancel-grip"
                    | "postcancel"
            )
        {
            return Err(ProbeError::FixtureInvariant(
                "invalid selected point-display recipe",
            ));
        }
        Ok(())
    }
}
fn geometry(kind: &str) -> Result<Geometry, ProbeError> {
    let p = |x, y, z| Point3::try_new(x, y, z);
    Ok(match kind {
        "line" => Geometry::Line(LineSegment::try_new(
            p(0., 0., 0.)?,
            p(3., 4., 0.)?,
            Tolerance::DEFAULT,
        )?),
        "curve" | "rational" => {
            let weights = if kind == "rational" {
                [1., 2., 0.5, 1.]
            } else {
                [1.; 4]
            };
            let points = [[0., 0., 0.], [1., 3., 1.], [3., 2., -1.], [4., 0., 0.]];
            Geometry::NurbsCurve(NurbsCurve::try_new_rational(
                3,
                points
                    .into_iter()
                    .zip(weights)
                    .map(|(p, w)| WeightedPoint3::try_new(Point3::try_new(p[0], p[1], p[2])?, w))
                    .collect::<Result<Vec<_>, GeometryError>>()?,
                vec![0., 0., 0., 0., 1., 1., 1., 1.],
            )?)
        }
        "surface" => Geometry::Brep(Brep::try_surface_face(
            NurbsSurface::try_bilinear([
                p(0., 0., 0.)?,
                p(2., 0., 0.)?,
                p(2., 3., 0.)?,
                p(0., 3., 0.)?,
            ])?,
            Tolerance::DEFAULT,
        )?),
        "mesh" => Geometry::Mesh(TriangleMesh::try_new_faces(
            vec![
                p(0., 0., 0.)?,
                p(2., 0., 0.)?,
                p(2., 2., 0.)?,
                p(0., 2., 0.)?,
            ],
            vec![MeshFace::Quad([0, 1, 2, 3])],
            Tolerance::DEFAULT,
        )?),
        "subd" => Geometry::SubD(SubD::try_from_polygons(
            [
                [0., 0., 0.],
                [2., 0., 0.],
                [2., 2., 0.],
                [0., 2., 0.],
                [0., 0., 2.],
                [2., 0., 2.],
                [2., 2., 2.],
                [0., 2., 2.],
            ]
            .into_iter()
            .map(|p| Point3::try_new(p[0], p[1], p[2]))
            .collect::<Result<_, _>>()?,
            vec![
                vec![0, 3, 2, 1],
                vec![4, 5, 6, 7],
                vec![0, 1, 5, 4],
                vec![1, 2, 6, 5],
                vec![2, 3, 7, 6],
                vec![3, 0, 4, 7],
            ],
        )?),
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "invalid point-display geometry",
            ));
        }
    })
}

/// Shared native recipe for both registry and application getter tests.
pub fn setup(f: &Fixture) -> Result<(Document, [ObjectId; 2], ObjectId, ObjectId), ProbeError> {
    f.validate()?;
    let mut doc = Document::default();
    let ids = [
        doc.add_geometry(geometry(&f.kind)?)?,
        doc.add_geometry(geometry(&f.kind)?)?,
    ];
    let peer = doc.add_geometry(Geometry::Point(Point3::try_new(20., 20., 20.)?))?;
    doc.clear_history()?;
    for id in ids {
        if f.mode == "edit" {
            doc.enable_edit_points([id])?;
        } else {
            doc.enable_control_points([id])?;
        }
    }
    if f.pattern.starts_with("group-") {
        doc.add_group(Some("Point display peers".into()), ids)?;
    }
    if f.pattern == "off-owner" {
        doc.disable_control_points_for_objects([ids[0]]);
    }
    if f.pattern == "owner-hidden-peer" {
        doc.select_control_points(
            [ControlPointId {
                object: ids[1],
                index: 1,
            }],
            SelectionMode::Add,
        )?;
        CommandRegistry::with_builtins().execute(&mut doc, "HidePt")?;
    }
    let marker = doc.add_geometry(Geometry::Point(Point3::try_new(99., 99., 99.)?))?;
    doc.undo()?;
    doc.redo()?;
    if doc.object(marker).is_none() {
        return Err(ProbeError::FixtureInvariant("Redo witness failed"));
    }
    doc.undo()?;
    if doc.object(marker).is_some() || !doc.can_redo() {
        return Err(ProbeError::FixtureInvariant("Undo witness failed"));
    }
    for (i, id) in ids.into_iter().enumerate() {
        if (i == 0
            && matches!(
                f.pattern.as_str(),
                "owner"
                    | "group-owner"
                    | "mixed"
                    | "owner-peer"
                    | "off-owner"
                    | "owner-hidden-peer"
            ))
            || f.pattern == "both-owners"
        {
            doc.select_objects_direct([id], SelectionMode::Add)?;
        }
        if (i == 0 && matches!(f.pattern.as_str(), "grip" | "group-grip"))
            || f.pattern == "both-grips"
            || (i == 1
                && matches!(
                    f.pattern.as_str(),
                    "mixed" | "postselect-grip" | "postcancel-grip"
                ))
        {
            doc.select_control_points(
                [ControlPointId {
                    object: id,
                    index: 0,
                }],
                SelectionMode::Add,
            )?;
        }
    }
    if matches!(
        f.pattern.as_str(),
        "peer" | "owner-peer" | "postselect-peer" | "postcancel-peer"
    ) {
        doc.select_objects_direct([peer], SelectionMode::Add)?;
    }
    Ok((doc, ids, peer, marker))
}

pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Value {
    json!(ids.iter().map(|id| {
        let points = doc.control_point_locations(*id);
        json!({"on":points.is_some(),"selected":doc.is_selected(*id),
            "grips":points.unwrap_or(&[]).iter().enumerate().map(|(index,p)|{
                let g=ControlPointId{object:*id,index};
                json!({"index":index,"point":p.to_array(),"selected":doc.selected_logical_control_points().any(|(pick,_)|pick==g),"hidden":doc.is_control_point_hidden(g).unwrap()})
            }).collect::<Vec<_>>()})
    }).collect::<Vec<_>>())
}

pub(super) fn run(f: &Fixture, iterations: u32) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "selected point-display iterations must be one",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let (mut doc, ids, peer, marker) = setup(f)?;
        let before = snapshot(&doc, &ids);
        let originals = ids.map(|id| doc.object(id).unwrap().clone());
        let commands = CommandRegistry::with_builtins();
        let prompt = commands.object_selection_prompt("PtOffSelected")?.unwrap();
        let getter = !doc
            .selected_objects()
            .any(|o| prompt.filter.accepts_in_document(&doc, o));
        if getter {
            commands.begin_object_selection(&mut doc, &prompt)?;
        }
        let result = if !getter {
            commands.execute(&mut doc, "PtOffSelected")?;
            "Success"
        } else if f.pattern.starts_with("postselect") {
            doc.select_objects_direct([ids[0]], SelectionMode::Add)?;
            commands.execute_after_object_selection(
                &mut doc,
                "PtOffSelected",
                Default::default(),
                None,
            )?;
            "Success"
        } else {
            // Getter cancellation never executes the command body. A post-pick
            // Cancel restores only temporary whole-object picks, keeping grips.
            let selected = doc.selected_object_ids().collect::<Vec<_>>();
            if f.pattern.starts_with("postcancel") {
                doc.select_objects_direct([ids[0]], SelectionMode::Add)?;
            }
            doc.set_transient_selection(selected)?;
            "Cancel"
        };
        let after = snapshot(&doc, &ids);
        let unchanged = ids.iter().zip(&originals).all(|(id, old)| {
            let now = doc.object(*id).unwrap();
            old.geometry_snapshot()
                .shares_storage_with(now.geometry_snapshot())
                && old.group_ids() == now.group_ids()
        });
        let peer_selected = doc.is_selected(peer);
        doc.redo()?;
        Ok(
            json!({"before":before,"after":after,"peer_selected":peer_selected,"unchanged":unchanged,
            "redo_preserved":doc.object(marker).is_some(),"redo_witness_confirmed":true,
            "getter_opened":getter,"command_result":result}),
        )
    })
}
