//! Owned CurveBoolean command, metadata and history observations.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: Vec<NurbsCurveDefinition>,
    #[serde(default)]
    pub surfaces: Vec<NurbsSurfaceDefinition>,
    pub options: BTreeMap<String, Value>,
    pub points: Vec<[Real; 3]>,
    pub all_regions: bool,
    #[serde(default = "yes")]
    pub preselect: bool,
    #[serde(default)]
    pub cancel: bool,
    #[serde(default)]
    pub cplane: Option<ConstructionPlane>,
    #[serde(default)]
    pub selections: Option<Vec<Selection>>,
    #[serde(default)]
    pub cancel_subcurve: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Selection {
    pub source: usize,
    #[serde(default)]
    pub edge: Option<usize>,
    pub range: Option<[[Real; 3]; 2]>,
    #[serde(default)]
    pub length: Option<Real>,
    #[serde(default)]
    pub confirmation: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConstructionPlane {
    pub origin: [Real; 3],
    pub x_axis: [Real; 3],
    pub y_axis: [Real; 3],
}
type Real = f64;
fn yes() -> bool {
    true
}
impl Fixture {
    pub fn temporary_ids(&self, ids: &[ObjectId]) -> Vec<ObjectId> {
        let whole = self
            .whole_ids(ids)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        self.selections
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|s| s.range.is_some() && !whole.contains(&ids[s.source]))
            .map(|s| ids[s.source])
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn whole_ids(&self, ids: &[ObjectId]) -> Vec<ObjectId> {
        self.selections.as_ref().map_or_else(
            || ids.to_vec(),
            |selections| {
                selections
                    .iter()
                    .filter(|s| s.range.is_none())
                    .map(|s| ids[s.source])
                    .collect()
            },
        )
    }
    pub fn command_with_sources(
        &self,
        doc: &Document,
        ids: &[ObjectId],
    ) -> Result<String, ProbeError> {
        let mut command = self.command()?;
        if let Some(selections) = &self.selections {
            let whole = self.whole_ids(ids);
            if !whole.is_empty() {
                command.push_str(&format!(
                    " Sources={}",
                    whole
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            for selection in selections {
                if let Some(picks) = selection.range {
                    let id = ids[selection.source];
                    let reference = viboceros_command::curve_reference::CapturedCurve::capture(
                        doc,
                        id,
                        selection.edge,
                    )?;
                    let curve = reference.curve_ref();
                    let a =
                        curve.closest_parameter(Point3::try_from(picks[0])?, doc.tolerance())?;
                    let b =
                        curve.closest_parameter(Point3::try_from(picks[1])?, doc.tolerance())?;
                    let range = if let Some(length) = selection.length {
                        if length == 0. {
                            None
                        } else {
                            viboceros_command::subcurve_input::interval(
                                curve,
                                a,
                                b,
                                length.abs(),
                                doc.tolerance(),
                            )?
                        }
                    } else {
                        viboceros_command::subcurve_input::point_parameters(
                            curve, a, b, None, true,
                        )?
                    };
                    let Some([a, b]) = range else {
                        continue;
                    };
                    if let Some(edge) = selection.edge {
                        let kind = if selection.length.is_some() {
                            "EdgeWrap"
                        } else {
                            "Edge"
                        };
                        command.push_str(&format!(" SubCrv={id},{kind},{edge},{a},{b}"));
                    } else {
                        command.push_str(&format!(" SubCrv={id},{a},{b}"));
                    }
                }
            }
        }
        Ok(command)
    }
    pub fn context(
        &self,
        tolerance: Tolerance,
    ) -> Result<viboceros_command::CommandContext, ProbeError> {
        Ok(if let Some(p) = &self.cplane {
            viboceros_command::CommandContext {
                construction_plane: Frame3::try_from_directions(
                    Point3::try_from(p.origin)?,
                    Vector3::try_from(p.x_axis)?,
                    Vector3::try_from(p.y_axis)?,
                    tolerance,
                )?,
            }
        } else {
            Default::default()
        })
    }
    pub fn setup(&self, tolerance: Tolerance) -> Result<(Document, Vec<ObjectId>), ProbeError> {
        let count = self.curves.len() + self.surfaces.len();
        if count == 0
            || count > 1000
            || self.points.len() > 1000
            || self
                .selections
                .as_ref()
                .is_some_and(|s| s.len() > 1000 || s.iter().any(|s| s.source >= count))
        {
            return Err(ProbeError::FixtureInvariant(
                "bounded owned CurveBoolean inputs required",
            ));
        }
        let mut doc = Document::new(tolerance);
        let layer = doc.add_layer("CurveBoolean Input", ColorRgb::BLACK)?;
        let output = doc.add_layer("CurveBoolean Output", ColorRgb::BLACK)?;
        let mut ids = Vec::new();
        let geometry = self
            .curves
            .iter()
            .map(|curve| Ok(Geometry::NurbsCurve(nurbs_curve_from_definition(curve)?)))
            .chain(self.surfaces.iter().map(|surface| {
                Ok::<_, ProbeError>(Geometry::Brep(Brep::try_surface_face(
                    nurbs_surface_from_definition(surface)?,
                    tolerance,
                )?))
            }));
        for (i, geometry) in geometry.enumerate() {
            let name = format!("source-{i}");
            let attrs = ObjectAttributes::on_layer(layer)
                .with_name(&name)
                .with_object_color(ColorRgb::new(
                    (25 + i) as u8,
                    (67 + i) as u8,
                    (109 + i) as u8,
                ))
                .try_with_user_text("attribute-origin", &name)?;
            let id = doc.add_geometry_with_attributes(geometry?, attrs)?;
            doc.set_object_geometry_user_text([id], "geometry-origin", Some(&name))?;
            ids.push(id);
        }
        doc.add_group(Some("CurveBoolean Sources".into()), ids.iter().copied())?;
        doc.set_current_layer(output)?;
        if self.preselect {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        Ok((doc, ids))
    }
    pub fn command(&self) -> Result<String, ProbeError> {
        let mut command = String::from("CurveBoolean");
        for (key, value) in &self.options {
            let name = match key.as_str() {
                "delete_input" => "DeleteInput",
                "combine" => "CombineRegions",
                "output" => "Output",
                "simplify" => "SimplifyCurves",
                _ => return Err(ProbeError::FixtureInvariant("unknown CurveBoolean option")),
            };
            let value = if let Some(v) = value.as_bool() {
                if v { "Yes" } else { "No" }
            } else {
                value
                    .as_str()
                    .ok_or(ProbeError::FixtureInvariant("invalid CurveBoolean option"))?
            };
            command.push_str(&format!(" {name}={value}"));
        }
        if self.all_regions {
            command.push_str(" AllRegions");
        }
        for p in &self.points {
            command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
        }
        Ok(command)
    }
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "single owned CurveBoolean workflow required",
        ));
    }
    let (mut doc, ids) = f.setup(tolerance)?;
    let temporary = f
        .temporary_ids(&ids)
        .into_iter()
        .map(|id| doc.object(id).unwrap().clone())
        .collect::<Vec<_>>();
    let commands = CommandRegistry::with_builtins();
    let before = snapshot(&doc, &ids)?;
    let success = if f.cancel_subcurve.is_some() {
        false
    } else if f.cancel {
        // Exercise the same staged preview used by the UI, then discard it.
        let input = viboceros_command::curve_boolean::parse(
            &f.command()?.split_whitespace().skip(1).collect::<Vec<_>>(),
            Default::default(),
        )?;
        let mut session = viboceros_command::curve_boolean::prepare(
            &doc,
            &ids,
            f.context(tolerance)?.construction_plane,
        )?;
        if input.all_regions {
            session.all_regions();
        }
        for p in input.points {
            session.toggle(p)?;
        }
        session.stage(input.options)?;
        false
    } else {
        if !f.preselect {
            doc.set_transient_selection(f.whole_ids(&ids))?;
        }
        let command = f.command_with_sources(&doc, &ids)?;
        let result = if f.preselect {
            commands.execute_in_context(&mut doc, &command, f.context(tolerance)?)
        } else {
            commands.execute_postselected(&mut doc, &command, f.context(tolerance)?)
        };
        result.is_ok()
    };
    let after = snapshot(&doc, &ids)?;
    for original in &temporary {
        if doc.object(original.id()).is_none_or(|current| {
            current != original
                || !current
                    .geometry_snapshot()
                    .shares_storage_with(original.geometry_snapshot())
        }) {
            return Err(ProbeError::FixtureInvariant("temporary parent changed"));
        }
    }
    if doc.can_undo() {
        commands.execute(&mut doc, "Undo")?;
    }
    let undo = snapshot(&doc, &ids)?;
    if doc.can_redo() {
        commands.execute(&mut doc, "Redo")?;
    }
    let redo = snapshot(&doc, &ids)?;
    Ok((
        json!({"success":success,"before":before,"after":after,"undo":undo,"redo":redo}),
        0,
    ))
}
pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Result<Value, ProbeError> {
    let objects=doc.objects().map(|o|{
        let a=o.attributes();let c=a.object_color();
        let groups=o.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap_or("")).collect::<Vec<_>>();
        let mut row=json!({"source":ids.iter().position(|id|*id==o.id()),"selected":if doc.is_selected(o.id()){2}else{0},
            "attributes":{"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),
                "color_source":if a.color_source()==ObjectColorSource::Object {"ColorFromObject"}else{"ColorFromLayer"},
                "color":[c.red,c.green,c.blue],"groups":groups,"user_text":a.user_text()},"geometry_user_text":o.geometry_user_text(),"native_valid":true});
        if let Geometry::Brep(b)=o.geometry() {
            row["kind"]=json!("brep");row["brep"]=brep_interchange::definition_record(b)?;row["area"]=json!(b.area(doc.tolerance())?);
        } else if let Some(curve)=o.geometry().curve_ref() {
            let c=curve.to_nurbs()?;let closed=c.is_closed()?;
            row["kind"]=json!("curve");row["closed"]=json!(closed);row["curve"]=nurbs_curve_definition_value(&c);
            if closed {
                let area=match Brep::try_planar_face(&c,doc.tolerance()) {
                    Ok(b)=>Some(b.area(doc.tolerance())?),
                    Err(GeometryError::InvalidPlanarFaceBoundary) if c.degree()==1=>{
                        let points=c.control_points().iter().map(|p|p.point()).collect::<Vec<_>>();
                        if let Some(plane)=viboceros_geometry::try_control_polygon_plane(&points,doc.tolerance())? {
                            Some(points.windows(2).map(|w|{
                                let a=plane.origin().vector_to(w[0])?;let b=plane.origin().vector_to(w[1])?;
                                a.cross(b)?.dot(plane.normal().as_vector())
                            }).collect::<Result<Vec<_>,_>>()?.iter().sum::<f64>().abs()*0.5)
                        }else{None}
                    }
                    Err(e)=>return Err(e.into()),
                };
                row["area"]=json!(area);
            }
        } else {return Err(ProbeError::FixtureInvariant("unexpected CurveBoolean object"));}
        Ok(row)
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok(json!({"objects":objects,"group_alive":doc.group_by_name("CurveBoolean Sources").is_some()}))
}
