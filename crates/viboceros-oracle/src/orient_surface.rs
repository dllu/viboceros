//! OrientOnSrf from authored surface controls, source witnesses, and UV picks.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_command::{CommandContext, orient_surface::Options};
use viboceros_geometry::Real;
fn yes() -> bool {
    true
}
fn one() -> Real {
    1.
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlacementOptions {
    #[serde(default = "yes")]
    pub copy: bool,
    #[serde(default = "yes")]
    pub rigid: bool,
    #[serde(default)]
    pub flip: bool,
    #[serde(default)]
    pub constrain_normal: bool,
    #[serde(default)]
    pub on_surface: bool,
    #[serde(default = "one")]
    pub scale: Real,
    #[serde(default)]
    pub rotation: Real,
    #[serde(default)]
    pub axes: Option<[Real; 3]>,
    #[serde(default)]
    pub uniform: Option<bool>,
    #[serde(default)]
    pub scale_prompt: Option<bool>,
    #[serde(default)]
    pub rotation_prompt: Option<bool>,
}
impl PlacementOptions {
    pub fn options(&self) -> Options {
        Options {
            copy: self.copy,
            rigid: self.rigid,
            flip: self.flip,
            constrain_normal: self.constrain_normal,
            on_surface: self.on_surface,
            scale: self.scale,
            rotation: self.rotation,
            axes: if self.uniform == Some(false) {
                self.axes
            } else {
                None
            },
            scale_prompt: self.scale_prompt.unwrap_or(true),
            rotation_prompt: self.rotation_prompt.unwrap_or(true),
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub uv: [Real; 2],
    pub scale: Option<Real>,
    pub rotation: Option<Real>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub surface: ObjectSource,
    pub base: [Real; 3],
    pub reference: [Real; 3],
    pub uv: [Real; 2],
    pub options: PlacementOptions,
    #[serde(default)]
    pub plane: Option<String>,
    #[serde(default)]
    pub postselected: bool,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub as_brep: bool,
    #[serde(default)]
    pub reverse_face: bool,
    #[serde(default)]
    pub batch: Vec<Batch>,
    #[serde(default)]
    pub offsets: Option<Vec<[Real; 3]>>,
    #[serde(default)]
    pub inherit: bool,
    #[serde(default)]
    pub setup: Option<PlacementOptions>,
}
impl Fixture {
    pub fn guide(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        let Geometry::NurbsSurface(s) = self.surface.geometry(tolerance)? else {
            return Err(ProbeError::FixtureInvariant(
                "surface orientation NURBS guide",
            ));
        };
        Ok(if self.as_brep || self.reverse_face {
            Geometry::Brep(Brep::try_rectangular_surface_face_with_orientation(
                s.clone(),
                s.domain_u(),
                s.domain_v(),
                self.reverse_face,
                tolerance,
            )?)
        } else {
            Geometry::NurbsSurface(s)
        })
    }
    pub fn source(&self) -> Result<PointCloud3, GeometryError> {
        let default = vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        PointCloud3::try_new(
            self.offsets
                .as_ref()
                .unwrap_or(&default)
                .iter()
                .map(|o| Point3::try_from(std::array::from_fn(|j| self.base[j] + o[j])))
                .collect::<Result<_, _>>()?,
        )
    }
    pub fn plane(&self, tolerance: Tolerance) -> Result<Frame3, ProbeError> {
        Ok(match self.plane.as_deref().unwrap_or("top") {
            "top" => CommandContext::default().construction_plane,
            "front" => Frame3::try_from_directions(
                Point3::try_from([0.; 3])?,
                Vector3::try_new(0., 0., 1.)?,
                Vector3::try_new(1., 0., 0.)?,
                tolerance,
            )?,
            "right" => Frame3::try_from_directions(
                Point3::try_from([0.; 3])?,
                Vector3::try_new(0., 1., 0.)?,
                Vector3::try_new(0., 0., 1.)?,
                tolerance,
            )?,
            _ => return Err(ProbeError::FixtureInvariant("surface orientation plane")),
        })
    }
    pub fn target(&self, uv: [Real; 2], tolerance: Tolerance) -> Result<Point3, ProbeError> {
        let Geometry::NurbsSurface(s) = self.surface.geometry(tolerance)? else {
            return Err(ProbeError::FixtureInvariant(
                "surface orientation NURBS guide",
            ));
        };
        Ok(s.evaluate(uv[0], uv[1])?)
    }
}
/// GUI defaults differ from the retained scripted `-OrientOnSrf` fixtures.
pub(super) fn run_options(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let mut f = f.clone();
    for options in std::iter::once(&mut f.options).chain(f.setup.iter_mut()) {
        options.scale_prompt.get_or_insert(false);
        options.rotation_prompt.get_or_insert(false);
    }
    run(&f, iterations, tolerance)
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let guide = f.guide(tolerance)?;
        let guide_id = doc.add_geometry(guide.clone())?;
        let source = doc.add_geometry(Geometry::PointCloud(f.source()?))?;
        if f.group {
            doc.add_group(Some("Owned surface orientation source".into()), [source])?;
        }
        doc.select_objects_direct([source], SelectionMode::Replace)?;
        doc.clear_history()?;
        let registry = CommandRegistry::with_builtins();
        if let Some(seed) = &f.setup
            && !registry.remember_orient_surface_options(seed.options())
        {
            return Err(ProbeError::FixtureInvariant(
                "invalid surface orientation preset",
            ));
        }
        let mut initial = if f.inherit {
            registry.orient_surface_options()
        } else {
            f.options.options()
        };
        initial.copy = f.options.copy;
        let plane = f.plane(tolerance)?;
        let mut group = doc.begin_history_group("OrientOnSrf")?;
        let mut error = None;
        let placements = std::iter::once((f.uv, initial)).chain(f.batch.iter().map(|b| {
            let mut o = initial;
            if o.scale_prompt {
                o.scale = b.scale.unwrap_or(o.scale);
            }
            if o.rotation_prompt {
                o.rotation = b.rotation.unwrap_or(o.rotation);
            }
            (b.uv, o)
        }));
        for (index, (uv, options)) in placements.enumerate() {
            let point = |p: [Real; 3]| p.map(|x| x.to_string()).join(",");
            let command = format!(
                "OrientOnSrf {} {} {} Surface={guide_id} Face=0 {} {}={source}",
                point(f.base),
                point(f.reference),
                point(f.target(uv, tolerance)?.to_array()),
                if f.inherit && index == 0 {
                    format!("Copy={}", if options.copy { "Yes" } else { "No" })
                } else {
                    options.arguments()
                },
                if f.postselected {
                    "PickedSources"
                } else {
                    "Sources"
                }
            );
            if let Err(e) = registry.execute_in_history_group(
                &mut doc,
                &command,
                CommandContext {
                    construction_plane: plane,
                },
                &mut group,
            ) {
                error = Some(e.to_string());
                break;
            }
        }
        if f.postselected {
            doc.clear_selection();
        }
        let outputs=doc.objects().filter_map(|o|{
            let Geometry::PointCloud(c)=o.geometry()else{return None;};
            Some(json!({"original":o.id()==source,"selected":doc.is_selected(o.id()),"points":c.points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),"group_count":o.group_ids().len()}))
        }).collect::<Vec<_>>();
        Ok(
            json!({"error":error,"outputs":outputs,"group_count":doc.groups().len(),"guide_selected":doc.is_selected(guide_id),"guide_unchanged":doc.object(guide_id).unwrap().geometry()==&guide,"undo_label":doc.undo_label()}),
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orient_surface_options_protocol_uses_fixed_gui_defaults() {
        let mut request: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/orient_surface_options.json"
        ))
        .unwrap();
        request["operations"] = json!([request["operations"][16].clone()]);
        let explicit: ProbeRequest = serde_json::from_value(request.clone()).unwrap();
        for key in ["scale_prompt", "rotation_prompt"] {
            request["operations"][0]["options"]
                .as_object_mut()
                .unwrap()
                .remove(key);
        }
        let implicit: ProbeRequest = serde_json::from_value(request).unwrap();
        assert_eq!(
            run_request(&explicit).unwrap().results[0].value,
            run_request(&implicit).unwrap().results[0].value
        );
    }

    #[test]
    fn orient_surface_matches_native_rigid_splop_normals_and_face_reversal() {
        for (fixture, reference) in [
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_surface.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_surface_rhino_20261016.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_surface_policy.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_surface_policy_rhino_20261016.json"
                ),
            ),
            (
                include_str!("../../../tools/rhino_oracle/fixtures/orient_surface_options.json"),
                include_str!(
                    "../../../tools/rhino_oracle/observations/orient_surface_options_rhino_20261016.json"
                ),
            ),
        ] {
            let request: ProbeRequest = serde_json::from_str(fixture).unwrap();
            let local = run_request(&request).unwrap();
            let native: Value = serde_json::from_str(reference).unwrap();
            assert_eq!(
                local.results.len(),
                native["results"].as_array().unwrap().len()
            );
            for (a, b) in local
                .results
                .iter()
                .zip(native["results"].as_array().unwrap())
            {
                assert_eq!(a.id, b["id"]);
                for k in ["guide_unchanged", "guide_selected", "group_count"] {
                    assert_eq!(a.value[k], b["value"][k], "{} {k}", a.id);
                }
                let actual = a.value["outputs"].as_array().unwrap();
                let expected = b["value"]["outputs"].as_array().unwrap();
                assert_eq!(actual.len(), expected.len(), "{}", a.id);
                let mut used = BTreeSet::new();
                for cloud in actual {
                    let (i, e) = expected
                        .iter()
                        .enumerate()
                        .find(|(i, e)| {
                            !used.contains(i)
                                && e["original"] == cloud["original"]
                                && cloud["points"].as_array().unwrap().len()
                                    == e["points"].as_array().unwrap().len()
                                && cloud["points"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .zip(e["points"].as_array().unwrap())
                                    .all(|(p, q)| {
                                        p.as_array().unwrap().iter().zip(q.as_array().unwrap()).all(
                                            |(p, q)| {
                                                (p.as_f64().unwrap() - q.as_f64().unwrap()).abs()
                                                    <= 1e-6
                                            },
                                        )
                                    })
                        })
                        .unwrap_or_else(|| {
                            panic!("{} points: {} expected {}", a.id, cloud, b["value"])
                        });
                    used.insert(i);
                    for k in ["selected", "group_count"] {
                        assert_eq!(cloud[k], e[k], "{} {k}", a.id);
                    }
                }
            }
        }
    }
}
