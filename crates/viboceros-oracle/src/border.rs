//! Actual border commands and representation-preserving output records.
use super::*;
use crate::{curve_join_close::CurveInput, object_source::ObjectSource};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BorderFixture {
    command: String,
    source: BorderSource,
    output_layer: Option<String>,
    faces: Option<Vec<usize>>,
    #[serde(default)]
    preselect: bool,
    artifact_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum BorderSource {
    Primitive(BorderPrimitive),
    Object(ObjectSource),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BorderPrimitive {
    Box {
        min: [f64; 3],
        max: [f64; 3],
        keep_faces: Option<Vec<usize>>,
    },
    Extrusion {
        curve: CurveInput,
        vector: [f64; 3],
    },
}

impl BorderSource {
    fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(match self {
            Self::Object(source) => source.geometry(tolerance)?,
            Self::Primitive(BorderPrimitive::Box {
                min,
                max,
                keep_faces,
            }) => {
                let brep = Brep::try_box(
                    viboceros_command::CommandContext::default().construction_plane,
                    std::array::from_fn(|i| [min[i], max[i]]),
                    tolerance,
                )?;
                Geometry::Brep(if let Some(faces) = keep_faces {
                    brep.duplicate_faces(faces, tolerance)?
                } else {
                    brep
                })
            }
            Self::Primitive(BorderPrimitive::Extrusion { curve, vector }) => Geometry::Brep(
                Brep::try_extruded_curve(
                    &curve.geometry()?.as_ref().to_nurbs()?,
                    Vector3::try_new(0., 0., 0.)?,
                    Vector3::try_from(*vector)?,
                    tolerance,
                )?
                .duplicate_faces(&[0], tolerance)?,
            ),
        })
    }
}

pub(super) fn run(f: &BorderFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid border fixture");
    if !matches!(f.command.as_str(), "DupBorder" | "DupFaceBorder") {
        return Err(invalid());
    }
    let layer = f.output_layer.as_deref().unwrap_or("Current");
    if !matches!(layer, "Current" | "Input") {
        return Err(invalid());
    }
    if let Some(faces) = &f.faces
        && (!f.preselect
            || faces.is_empty()
            || faces.iter().collect::<BTreeSet<_>>().len() != faces.len())
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let input_layer = document.add_layer("Source", ColorRgb::BLACK)?;
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let geometry = f.source.geometry(tolerance)?;
    if let Some(path) = &f.artifact_path {
        use viboceros_io::{
            ThreeDmGeometry, ThreeDmLayer, ThreeDmModel, ThreeDmObject, read_3dm_file,
            write_3dm_file,
        };
        let geometry = match &geometry {
            Geometry::Brep(brep) => ThreeDmGeometry::Brep(brep.clone()),
            _ => {
                return Err(ProbeError::FixtureInvariant(
                    "border source artifact requires B-rep",
                ));
            }
        };
        let model = ThreeDmModel::new(
            vec![ThreeDmLayer {
                name: "Source".into(),
                color: [0, 0, 0],
                visible: true,
                locked: false,
            }],
            vec![],
            vec![ThreeDmObject::new(geometry, 0)],
        );
        drop(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?,
        );
        write_3dm_file(path, &model)?;
        let decoded = read_3dm_file(path, tolerance)?;
        let (
            ThreeDmGeometry::Brep(source),
            Some(ThreeDmObject {
                geometry: ThreeDmGeometry::Brep(restored),
                ..
            }),
        ) = (&model.objects[0].geometry, decoded.objects.first())
        else {
            return Err(ProbeError::FixtureInvariant(
                "border source artifact lost its B-rep",
            ));
        };
        if !super::brep_interchange::roundtrip_equal(
            &super::brep_interchange::geometry_record(source)?,
            &super::brep_interchange::geometry_record(restored)?,
        ) {
            return Err(ProbeError::FixtureInvariant(
                "border source artifact changed topology or geometry",
            ));
        }
    }
    let source = document.add_geometry_with_attributes(
        geometry,
        ObjectAttributes::on_layer(input_layer)
            .with_name("Source")
            .with_object_color(ColorRgb::new(11, 22, 33)),
    )?;
    document.add_group(Some("Source group".into()), [source])?;
    document.select_objects_direct([source], SelectionMode::Replace)?;
    let mut command = format!("{} OutputLayer={layer}", f.command);
    if let Some(faces) = &f.faces {
        command.push_str(&format!(
            " Faces={}",
            faces
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ));
    } else if f.command == "DupFaceBorder" {
        command.push_str(" Faces=All");
    }
    let registry = CommandRegistry::with_builtins();
    let result = if f.preselect {
        registry.execute(&mut document, &command)
    } else {
        registry.execute_postselected(&mut document, &command, Default::default())
    };
    result?;
    let outputs=document.objects().filter(|o|o.id()!=source).map(|o| {
        let attrs=o.attributes();let color=attrs.object_color();
        Ok(json!({"curve":crate::curve_interchange::curve_record(o.geometry().curve_ref().ok_or_else(invalid)?)?,
            "selected":document.is_selected(o.id()),"name":attrs.name(),
            "layer":if attrs.layer_id()==input_layer {"Source"} else if attrs.layer_id()==current_layer {"Current"} else {"Unexpected"},
            "color":[color.red,color.green,color.blue],"color_source":format!("ColorFrom{:?}",attrs.color_source()),
            "group_count":o.group_ids().len()}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok((
        json!({"succeeded":true,"source_retained":document.object(source).is_some(),"source_selected":document.is_selected(source),"outputs":outputs,"new_groups":document.groups().len()-1}),
        0,
    ))
}
