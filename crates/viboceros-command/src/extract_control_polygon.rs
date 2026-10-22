//! Control-polygon extraction and its command-owned output options.
use crate::*;

const EXTRACT_CONTROL_POLYGON_USAGE: &str =
    "ExtractControlPolygon [OutputLayer=Current|Input|TargetObject]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExtractControlPolygonOutputLayer {
    Current,
    Input,
    TargetObject,
}

pub(super) struct ExtractControlPolygonCommand;

impl Command for ExtractControlPolygonCommand {
    fn name(&self) -> &'static str {
        "ExtractControlPolygon"
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let output_layer = parse_extract_control_polygon_arguments(arguments)?;
        let selected = document
            .selected_objects()
            .map(|object| (object.geometry().clone(), object.attributes().layer_id()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }

        let current_layer = document.current_layer_id();
        let mut staged = Vec::new();
        staged
            .try_reserve(selected.len())
            .map_err(|_| too_many_span_outputs("ExtractControlPolygon"))?;
        for (geometry, input_layer) in &selected {
            let layer = match output_layer {
                ExtractControlPolygonOutputLayer::Current => current_layer,
                ExtractControlPolygonOutputLayer::Input
                | ExtractControlPolygonOutputLayer::TargetObject => *input_layer,
            };
            let geometry = match geometry {
                Geometry::PolyCurve(curve) => Some(Geometry::Polyline(
                    curve.control_polygon(document.tolerance())?,
                )),
                Geometry::NurbsSurface(surface) => surface
                    .control_polygon_mesh(document.tolerance())?
                    .map(Geometry::Mesh),
                Geometry::NurbsCurve(curve) => Some(Geometry::Polyline(
                    curve.control_polygon(document.tolerance())?,
                )),
                Geometry::Line(_)
                | Geometry::Circle(_)
                | Geometry::Arc(_)
                | Geometry::Ellipse(_)
                | Geometry::Polyline(_) => {
                    let curve = geometry
                        .nurbs_curve_representation()?
                        .expect("supported non-NURBS curves have exact NURBS forms");
                    Some(Geometry::Polyline(
                        curve.control_polygon(document.tolerance())?,
                    ))
                }
                Geometry::Point(_)
                | Geometry::BlockInstance(_)
                | Geometry::PointCloud(_)
                | Geometry::Brep(_)
                | Geometry::Mesh(_)
                | Geometry::SubD(_) => {
                    return Err(CommandError::UnsupportedExtractControlPolygonGeometry);
                }
            };
            let Some(geometry) = geometry else {
                continue;
            };
            if staged.len() == MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("ExtractControlPolygon"));
            }
            staged.push((geometry, layer));
        }
        if staged.is_empty() {
            return Err(CommandError::NoExtractableControlPolygons);
        }

        let output_count = staged.len();
        let mut output_ids = Vec::with_capacity(output_count);
        for (geometry, layer) in staged {
            output_ids.push(
                document
                    .add_geometry_with_attributes(geometry, ObjectAttributes::on_layer(layer))?,
            );
        }
        replace_selection(document, output_ids)?;
        Ok(format!(
            "Extracted {output_count} control polygon(s) from {} object(s)",
            selected.len()
        ))
    }
}

fn parse_extract_control_polygon_arguments(
    arguments: &[&str],
) -> Result<ExtractControlPolygonOutputLayer, CommandError> {
    if arguments.is_empty() {
        return Ok(ExtractControlPolygonOutputLayer::Current);
    }
    let (name, value, consumed) = if let Some((name, value)) = arguments[0].split_once('=') {
        (name, value, 1)
    } else {
        let value = arguments
            .get(1)
            .ok_or(CommandError::Usage(EXTRACT_CONTROL_POLYGON_USAGE))?;
        (arguments[0], *value, 2)
    };
    require_consumed(arguments, consumed, EXTRACT_CONTROL_POLYGON_USAGE)?;
    if !option_name_eq(name, "OutputLayer") {
        return Err(CommandError::Usage(EXTRACT_CONTROL_POLYGON_USAGE));
    }
    let value = value.trim_start_matches('_');
    if value.eq_ignore_ascii_case("Current") {
        Ok(ExtractControlPolygonOutputLayer::Current)
    } else if value.eq_ignore_ascii_case("Input") {
        Ok(ExtractControlPolygonOutputLayer::Input)
    } else if value.eq_ignore_ascii_case("TargetObject") {
        Ok(ExtractControlPolygonOutputLayer::TargetObject)
    } else {
        Err(CommandError::Usage(EXTRACT_CONTROL_POLYGON_USAGE))
    }
}
