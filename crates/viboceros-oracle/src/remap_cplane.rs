//! Authored RemapCPlane recipes and complete document-state observations.
use super::*;
use viboceros_command::{CommandContext, remap_cplane as command};
use viboceros_document::ControlPointId;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum SourcePlane {
    Top,
    #[serde(rename = "tilted")]
    Tilted,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Target {
    Top,
    Bottom,
    Front,
    Back,
    Right,
    Left,
    #[serde(rename = "Owned target")]
    Owned,
}
impl Target {
    fn frame(self) -> Result<Frame3, ProbeError> {
        Ok(match self {
            Self::Owned => Fixture::named_target_frame()?,
            Self::Top => command::world_frame("Top").unwrap(),
            Self::Bottom => command::world_frame("Bottom").unwrap(),
            Self::Front => command::world_frame("Front").unwrap(),
            Self::Back => command::world_frame("Back").unwrap(),
            Self::Right => command::world_frame("Right").unwrap(),
            Self::Left => command::world_frame("Left").unwrap(),
        })
    }
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Mode {
    CPlane,
    View,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Points,
    Rational,
    Surface,
    Mesh,
}

fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub source_plane: SourcePlane,
    pub target: Target,
    pub mode: Mode,
    pub source: Source,
    pub copy: bool,
    pub postselected: bool,
    pub group: bool,
    pub grips: bool,
    #[serde(default, deserialize_with = "present")]
    pub retry_target: Option<Target>,
    #[serde(default, deserialize_with = "present")]
    pub setup_copy: Option<bool>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl Fixture {
    fn validate(&self) -> Result<(), ProbeError> {
        if !self.extra.is_empty()
            || self.grips && (self.source == Source::Points || self.postselected)
            || self.retry_target == Some(Target::Owned)
            || self.setup_copy.is_some() && (self.grips || self.postselected)
        {
            return Err(ProbeError::FixtureInvariant("invalid RemapCPlane recipe"));
        }
        Ok(())
    }

    pub fn source_frame(&self) -> Result<Frame3, GeometryError> {
        match self.source_plane {
            SourcePlane::Top => Ok(command::world_frame("Top").unwrap()),
            SourcePlane::Tilted => frame([5., -4., 2.], [1., 1., 0.], [-1., 1., 2.]),
        }
    }

    pub fn named_target_frame() -> Result<Frame3, GeometryError> {
        frame([-3., 7., 11.], [0., 1., 1.], [2., 1., -1.])
    }

    /// Construct the request's sources without consulting captured outputs.
    pub fn source_geometries(&self, tolerance: Tolerance) -> Result<Vec<Geometry>, ProbeError> {
        self.validate()?;
        let point = |p| Point3::try_from(p);
        let cardinal = [[2., 0., 0.], [0., 2., 0.], [-2., 0., 0.], [0., -2., 0.]];
        Ok(match self.source {
            Source::Points => (0..2)
                .map(|i| {
                    let x = 1. + 3. * i as f64;
                    Ok(Geometry::PointCloud(PointCloud3::try_new(
                        [[x, 2., 3.], [x + 1., 2., 3.], [x, 3., 3.], [x, 2., 4.]]
                            .into_iter()
                            .map(point)
                            .collect::<Result<Vec<_>, _>>()?,
                    )?))
                })
                .collect::<Result<Vec<_>, ProbeError>>()?,
            Source::Rational => vec![Geometry::NurbsCurve(NurbsCurve::try_new_rational(
                2,
                cardinal
                    .into_iter()
                    .zip([1., 2., 0.5, 1.])
                    .map(|(p, w)| WeightedPoint3::try_new(point(p)?, w))
                    .collect::<Result<Vec<_>, _>>()?,
                vec![0., 0., 0., 1., 2., 2., 2.],
            )?)],
            Source::Surface => {
                let mut controls = Vec::new();
                for v in 0..3 {
                    for u in 0..3 {
                        let uv = (u * v) as f64;
                        controls.push(WeightedPoint3::try_new(
                            point([u as f64, v as f64, 0.25 * uv])?,
                            1. + 0.125 * uv,
                        )?);
                    }
                }
                let surface = NurbsSurface::try_new_rational(
                    2,
                    2,
                    3,
                    3,
                    controls,
                    vec![0., 0., 0., 1., 1., 1.],
                    vec![0., 0., 0., 1., 1., 1.],
                )?;
                vec![Geometry::Brep(Brep::try_surface_face(surface, tolerance)?)]
            }
            Source::Mesh => vec![Geometry::Mesh(TriangleMesh::try_new_faces(
                cardinal
                    .into_iter()
                    .chain([cardinal[0]])
                    .map(point)
                    .collect::<Result<Vec<_>, _>>()?,
                vec![MeshFace::Quad([0, 1, 2, 3]), MeshFace::Triangle([4, 2, 3])],
                tolerance,
            )?)],
        })
    }
}

fn frame(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Result<Frame3, GeometryError> {
    Frame3::try_from_unit_directions(
        Point3::try_from(origin)?,
        Vector3::try_from(x)?.normalized_nonzero()?,
        Vector3::try_from(y)?.normalized_nonzero()?,
        Tolerance::NUMERICAL_VALIDATION,
    )
}

fn snapshot(doc: &Document, sources: &[ObjectId], plane: Frame3) -> Result<Value, ProbeError> {
    let mut observed = super::orient_surface_grips::snapshot_excluding(doc, sources[0], None, &[])?;
    let mut objects = observed["outputs"].take();
    for (value, object) in objects
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(doc.objects())
    {
        let value = value.as_object_mut().unwrap();
        for key in ["original", "peer", "group_count"] {
            value.remove(key);
        }
        value.insert(
            "source".into(),
            json!(sources.iter().position(|&id| id == object.id())),
        );
        value.insert("name".into(), json!(object.attributes().name()));
    }
    Ok(json!({"objects":objects,
        "groups":doc.groups().map(|g|json!({"name":g.name(),"count":g.members().len()})).collect::<Vec<_>>(),
        "plane":{"origin":plane.origin().to_array(),"x_axis":plane.x_axis().as_vector().to_array(),"y_axis":plane.y_axis().as_vector().to_array()}}))
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    f.validate()?;
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "RemapCPlane requires one iteration",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let source_plane = f.source_frame()?;
        let mut ids = Vec::new();
        for (i, geometry) in f.source_geometries(tolerance)?.into_iter().enumerate() {
            ids.push(
                doc.add_geometry_with_attributes(
                    geometry,
                    ObjectAttributes::on_layer(doc.current_layer_id())
                        .with_name(format!("remap source {i}")),
                )?,
            );
        }
        if f.group {
            doc.add_group(Some("Owned remap group".into()), ids.iter().copied())?;
        }
        if f.grips {
            doc.enable_control_points([ids[0]])?;
            doc.select_control_points(
                [0, 2].map(|index| ControlPointId {
                    object: ids[0],
                    index,
                }),
                SelectionMode::Add,
            )?;
        } else if !f.postselected {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        let setup = if f.setup_copy.is_some() {
            let default = registry.begin_copy_options("RemapCPlane").unwrap();
            let choice = format!(
                "Copy={}",
                if f.setup_copy == Some(true) {
                    "Yes"
                } else {
                    "No"
                }
            );
            let pending = command::options(&[&choice], default)?;
            if pending.target.is_some() || registry.copy_default("RemapCPlane") != Some(default) {
                return Err(ProbeError::FixtureInvariant(
                    "canceled RemapCPlane options changed defaults",
                ));
            }
            // Discard the parsed pending choice before any placement is accepted.
            Some(json!({"success":false,"after":snapshot(&doc,&ids,source_plane)?}))
        } else {
            None
        };
        let before = snapshot(&doc, &ids, source_plane)?;
        if f.postselected {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        }
        let sources = if f.grips { Vec::new() } else { ids.clone() };
        let picks = if f.grips {
            [0, 2].map(|index| format!("{}:{index}", ids[0])).join(",")
        } else {
            String::new()
        };
        let picked = if f.postselected { "Picked" } else { "" };
        let copy = if f.setup_copy.is_some() {
            String::new()
        } else {
            format!("Copy={}", if f.copy { "Yes" } else { "No" })
        };
        let command = |target: Frame3| {
            format!(
                "RemapCPlane {} {copy} {picked}Sources={} {}",
                command::frame_argument(target),
                sources
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
                if picks.is_empty() {
                    String::new()
                } else {
                    format!("{picked}GripSources={picks}")
                }
            )
        };
        let context = CommandContext {
            construction_plane: source_plane,
        };
        let first = registry.execute_in_context(&mut doc, &command(f.target.frame()?), context);
        let rejected = first.is_err();
        if let Some(target) = f.retry_target {
            if !rejected {
                return Err(ProbeError::FixtureInvariant(
                    "retry target requires a rejected first target",
                ));
            }
            registry.execute_in_context(&mut doc, &command(target.frame()?), context)?;
        } else {
            first?;
        }
        let after = snapshot(&doc, &ids, source_plane)?;
        if f.postselected {
            doc.clear_selection();
        }
        let after_script = snapshot(&doc, &ids, source_plane)?;
        let undo = doc.undo()?.is_some();
        let undo_snapshot = snapshot(&doc, &ids, source_plane)?;
        let redo = doc.redo()?.is_some();
        let redo_snapshot = snapshot(&doc, &ids, source_plane)?;
        let mut value = json!({"success":true,"before":before,"after":after,"after_script":after_script,
            "undo":undo_snapshot,"redo":redo_snapshot,"undo_returned":undo,"redo_returned":redo,
            "target_rejections":usize::from(rejected)});
        if let Some(setup) = setup {
            value["setup"] = setup;
        }
        Ok(value)
    })
}

#[cfg(test)]
mod tests;
