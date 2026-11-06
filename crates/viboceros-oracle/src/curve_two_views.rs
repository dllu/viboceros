//! Native two-profile reconstruction and complete source/history observations.
use super::*;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub curves: [NurbsCurveDefinition; 2],
    pub directions: [[f64; 3]; 2],
    pub mode: String,
    #[serde(default)]
    pub preselect: bool,
    #[serde(default)]
    pub ranges: Option<[Option<[[f64; 3]; 2]>; 2]>,
    #[serde(default)]
    pub cancel: Option<String>,
}
impl Fixture {
    pub fn setup(&self, tolerance: Tolerance) -> Result<(Document, [ObjectId; 2]), ProbeError> {
        let mut doc = Document::new(tolerance);
        let input = doc.add_layer("Crv2View Input", ColorRgb::BLACK)?;
        let output = doc.add_layer("Crv2View Output", ColorRgb::BLACK)?;
        let mut ids = Vec::new();
        for (i, curve) in self.curves.iter().enumerate() {
            let name = format!("source-{i}");
            let attrs = ObjectAttributes::on_layer(input)
                .with_name(&name)
                .with_object_color(ColorRgb::new(25 + i as u8, 67 + i as u8, 109 + i as u8))
                .try_with_user_text("attribute-origin", &name)?;
            let id = doc.add_geometry_with_attributes(
                Geometry::NurbsCurve(nurbs_curve_from_definition(curve)?),
                attrs,
            )?;
            doc.set_object_geometry_user_text([id], "geometry-origin", Some(&name))?;
            ids.push(id);
        }
        doc.add_group(Some("Crv2View Sources".into()), ids.iter().copied())?;
        doc.set_current_layer(output)?;
        if self.preselect {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        }
        doc.clear_history()?;
        Ok((doc, ids.try_into().unwrap()))
    }
    pub fn profiles(
        &self,
        doc: &Document,
        ids: [ObjectId; 2],
    ) -> Result<[viboceros_command::curve_two_views::Profile; 2], ProbeError> {
        let mut profiles = ids.map(viboceros_command::curve_two_views::Profile::whole);
        for i in 0..2 {
            if self.mode != "auto" && self.mode != "preselected" {
                profiles[i].direction = Some(Vector3::try_from(self.directions[i])?);
            }
            if let Some(points) = self.ranges.as_ref().and_then(|r| r[i]) {
                let curve = doc.object(ids[i]).unwrap().geometry().curve_ref().unwrap();
                let parameters =
                    points.map(|p| curve.closest_parameter(Point3::try_from(p)?, doc.tolerance()));
                let [a, b] = parameters;
                profiles[i].parameters =
                    viboceros_command::subcurve_input::point_parameters(curve, a?, b?, None, true)?;
            }
        }
        Ok(profiles)
    }
    pub fn command(&self, doc: &Document, ids: [ObjectId; 2]) -> Result<String, ProbeError> {
        let mut command = format!("Crv2View Sources={},{}", ids[0], ids[1]);
        for (i, profile) in self.profiles(doc, ids)?.iter().enumerate() {
            if let Some(d) = profile.direction {
                command.push_str(&format!(
                    " Direction{}={},{},{}",
                    i + 1,
                    d.x(),
                    d.y(),
                    d.z()
                ));
            }
            if let Some([a, b]) = profile.parameters {
                command.push_str(&format!(" SubCrv{}={a},{b}", i + 1));
            }
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
            "single owned two-view workflow required",
        ));
    }
    if !matches!(f.mode.as_str(), "before" | "auto" | "preselected") {
        return Err(ProbeError::FixtureInvariant(
            "native SDK audit modes have no local command replay",
        ));
    }
    let (mut doc, ids) = f.setup(tolerance)?;
    let original = doc.objects().cloned().collect::<Vec<_>>();
    let before = snapshot(&doc, &ids)?;
    let commands = CommandRegistry::with_builtins();
    if f.cancel.is_some() {
        return Err(ProbeError::FixtureInvariant(
            "Crv2View controller capture has no local kernel replay",
        ));
    }
    let command = f.command(&doc, ids)?;
    let result = (|| -> Result<_, viboceros_command::CommandError> {
        let input = viboceros_command::curve_two_views::parse(
            &command.split_whitespace().skip(1).collect::<Vec<_>>(),
        )?;
        let prepared = viboceros_command::curve_two_views::prepare(
            &doc,
            &input.profiles(&doc)?,
            viboceros_command::CommandContext::default().construction_plane,
        )?;
        prepared.commit(&mut doc)
    })()
    .map_err(|e| e.to_string());
    let success = result.is_ok();
    let after = snapshot(&doc, &ids)?;
    for old in &original {
        let current = doc
            .object(old.id())
            .ok_or(ProbeError::FixtureInvariant("profile disappeared"))?;
        if current != old
            || !current
                .geometry_snapshot()
                .shares_storage_with(old.geometry_snapshot())
        {
            return Err(ProbeError::FixtureInvariant("profile changed"));
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
        json!({"success":success,"before":before,"after":after,"undo":undo,"redo":redo,"local_error":result.err()}),
        0,
    ))
}
pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Result<Value, ProbeError> {
    let objects=doc.objects().map(|o| {
        let a=o.attributes();let color=a.object_color();let c=o.geometry().curve_ref().ok_or(ProbeError::FixtureInvariant("unexpected Crv2View object"))?;
        let curve=c.to_nurbs()?;
        Ok(json!({"kind":"curve","source":ids.iter().position(|id|*id==o.id()),"selected":if doc.is_selected(o.id()){2}else{0},
            "attributes":{"name":a.name().unwrap_or(""),"layer":doc.layer(a.layer_id()).unwrap().name(),"color_source":if a.color_source()==ObjectColorSource::Object {"ColorFromObject"}else{"ColorFromLayer"},"color":[color.red,color.green,color.blue],"groups":o.group_ids().iter().map(|id|doc.group(*id).unwrap().name().unwrap_or("")).collect::<Vec<_>>(),"user_text":a.user_text()},"geometry_user_text":o.geometry_user_text(),"native_valid":true,"curve":nurbs_curve_definition_value(&curve),"closed":curve.is_closed()?,"length":c.length(doc.tolerance())?}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok(json!({"objects":objects,"group_alive":doc.group_by_name("Crv2View Sources").is_some()}))
}
