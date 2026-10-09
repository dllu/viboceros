//! Plane sections use validated intersection results and atomic output admission.
use super::*;
use viboceros_geometry::Frame3;
#[cfg(test)]
mod tests;
pub(super) struct SectionCommand;
pub const SECTION_USAGE: &str = "Section start-point end-point [ExtendSection=Yes|No] [AssignProperties=ByCurrentLayer|ByInputObject] [Output=All|CurvesOnly] [GroupObjectsBySectionPlane=Yes|No]";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionOptions {
    pub extend: bool,
    pub input_properties: bool,
    pub group: bool,
}
impl Default for SectionOptions {
    fn default() -> Self {
        Self {
            extend: true,
            input_properties: false,
            group: false,
        }
    }
}
pub fn parse_options(words: &[&str]) -> Result<SectionOptions, CommandError> {
    let mut result = SectionOptions::default();
    let mut seen = BTreeSet::new();
    for word in words {
        let (key, value) = word
            .trim_start_matches('_')
            .split_once('=')
            .ok_or(CommandError::Usage(SECTION_USAGE))?;
        let key = key.to_ascii_lowercase();
        if !seen.insert(key.clone()) {
            return Err(CommandError::Usage(SECTION_USAGE));
        }
        match key.as_str() {
            "extendsection" => result.extend = parse_bool(value)?,
            "groupobjectsbysectionplane" => result.group = parse_bool(value)?,
            "assignproperties" => {
                result.input_properties =
                    match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                        "bycurrentlayer" => false,
                        "byinputobject" => true,
                        _ => return Err(CommandError::Usage(SECTION_USAGE)),
                    }
            }
            "output"
                if matches!(
                    value.trim_start_matches('_').to_ascii_lowercase().as_str(),
                    "all" | "curvesonly"
                ) => {}
            _ => return Err(CommandError::Usage(SECTION_USAGE)),
        }
    }
    Ok(result)
}
fn parse_bool(word: &str) -> Result<bool, CommandError> {
    match word.trim_start_matches('_').to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(CommandError::Usage(SECTION_USAGE)),
    }
}
pub fn section_frame(
    start: Point3,
    end: Point3,
    cplane: Frame3,
    tolerance: Tolerance,
) -> Result<(Frame3, f64), GeometryError> {
    let delta = start.vector_to(end)?;
    let height = delta.dot(cplane.z_axis().as_vector())?;
    let direction = subtract_vectors(delta, cplane.z_axis().as_vector().scaled(height)?)?;
    let length = direction.length()?;
    let frame =
        Frame3::try_from_directions(start, direction, cplane.z_axis().as_vector(), tolerance)?;
    Ok((frame, length))
}
fn cutter_for(
    geometry: &Geometry,
    frame: Frame3,
    length: f64,
    extend: bool,
    tolerance: Tolerance,
) -> Result<NurbsSurface, GeometryError> {
    let bounds = geometry.bounds();
    let a = bounds.min().to_array();
    let b = bounds.max().to_array();
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for mask in 0..8 {
        let point = Point3::try_from(std::array::from_fn::<_, 3, _>(|axis| {
            if mask & (1 << axis) == 0 {
                a[axis]
            } else {
                b[axis]
            }
        }))?;
        let p = frame.coordinates_of(point)?;
        for axis in 0..2 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    for axis in 0..2 {
        let width = high[axis] - low[axis];
        let margin = width.max(tolerance.absolute() * 8.).max(1e-12);
        low[axis] -= margin;
        high[axis] += margin;
    }
    if !extend {
        low[0] = 0.;
        high[0] = length;
    }
    let points = [
        [low[0], low[1], 0.],
        [low[0], high[1], 0.],
        [high[0], low[1], 0.],
        [high[0], high[1], 0.],
    ]
    .map(|p| frame.point_at(p))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    NurbsSurface::try_clamped_uniform(1, 1, 2, 2, points)
}
pub fn section_geometry(
    geometry: &Geometry,
    frame: Frame3,
    length: f64,
    extend: bool,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if matches!(geometry, Geometry::Point(_) | Geometry::PointCloud(_)) {
        return Err(CommandError::UnsupportedIntersectGeometry);
    }
    if let Geometry::Mesh(mesh) = geometry {
        let curves = mesh.section_with_plane(frame, tolerance)?;
        let mut output = Vec::new();
        for curve in curves {
            if extend {
                output.push(Geometry::Polyline(curve));
                continue;
            }
            let mut chain = Vec::new();
            for endpoints in curve.vertices().windows(2) {
                let a = frame.coordinates_of(endpoints[0])?[0];
                let b = frame.coordinates_of(endpoints[1])?[0];
                let mut low = 0_f64;
                let mut high = 1_f64;
                if a == b {
                    if a < 0. || a > length {
                        if chain.len() > 1 {
                            output.push(Geometry::Polyline(
                                viboceros_geometry::Polyline3::try_new(
                                    std::mem::take(&mut chain),
                                    tolerance,
                                )?,
                            ));
                        }
                        continue;
                    }
                } else {
                    let first = (0. - a) / (b - a);
                    let second = (length - a) / (b - a);
                    low = low.max(first.min(second));
                    high = high.min(first.max(second));
                }
                if low > high {
                    if chain.len() > 1 {
                        output.push(Geometry::Polyline(viboceros_geometry::Polyline3::try_new(
                            std::mem::take(&mut chain),
                            tolerance,
                        )?));
                    }
                    continue;
                }
                let start =
                    endpoints[0].translated(endpoints[0].vector_to(endpoints[1])?.scaled(low)?)?;
                let end =
                    endpoints[0].translated(endpoints[0].vector_to(endpoints[1])?.scaled(high)?)?;
                if chain
                    .last()
                    .is_none_or(|p: &Point3| !model_points_near(*p, start, tolerance))
                {
                    if chain.len() > 1 {
                        output.push(Geometry::Polyline(viboceros_geometry::Polyline3::try_new(
                            std::mem::take(&mut chain),
                            tolerance,
                        )?));
                    }
                    chain.push(start);
                }
                if !model_points_near(start, end, tolerance) {
                    chain.push(end);
                }
            }
            if chain.len() > 1 {
                output.push(Geometry::Polyline(viboceros_geometry::Polyline3::try_new(
                    chain, tolerance,
                )?));
            }
        }
        return Ok(output);
    }
    let cutter = IntersectInput::Surface(cutter_for(geometry, frame, length, extend, tolerance)?);
    if extend
        && let Geometry::NurbsSurface(surface) = geometry
        && surface.plane(tolerance)?.is_some_and(|plane| {
            plane
                .normal()
                .as_vector()
                .cross(frame.z_axis().as_vector())
                .is_ok_and(|v| v.length().is_ok_and(|length| length <= tolerance.angular()))
                && plane
                    .signed_distance_to(frame.origin())
                    .is_ok_and(|d| d.abs() <= tolerance.absolute())
        })
    {
        return border::surface_components(surface)?
            .into_iter()
            .map(|curves| border::assemble(curves, tolerance).map_err(CommandError::from))
            .collect();
    }
    let input = intersect_input(geometry)?;
    let output = intersect_pair(&input, &cutter, tolerance)?;
    let mut points = Vec::new();
    let mut curves = Vec::new();
    for geometry in output {
        match geometry {
            Geometry::Point(point) => {
                if !points
                    .iter()
                    .any(|p| model_points_near(*p, point, tolerance))
                {
                    points.push(point);
                }
            }
            geometry => {
                if let Some(curve) = geometry.curve_ref() {
                    curves.push(curve.to_owned());
                }
            }
        }
    }
    let joined = viboceros_geometry::join_curves(
        &curves,
        viboceros_geometry::CurveJoinOptions {
            tolerance: tolerance.absolute() * 2.,
            preserve_direction: false,
            style: viboceros_geometry::CurveJoinStyle::Batch,
        },
        tolerance,
    )?;
    let mut result = points.into_iter().map(Geometry::Point).collect::<Vec<_>>();
    result.extend(
        joined
            .into_iter()
            .map(|curve| Geometry::from(curve.into_curve())),
    );
    Ok(result)
}
impl Command for SectionCommand {
    fn name(&self) -> &'static str {
        "Section"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        Ok(args.is_empty().then(|| ObjectSelectionPrompt {
            command: self.name(),
            filter: ObjectSelectionFilter::Section,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let (start, first) = parse_point(args)?;
        let (end, second) = parse_point(&args[first..])?;
        let options = parse_options(&args[first + second..])?;
        let (frame, length) =
            section_frame(start, end, context.construction_plane, doc.tolerance())?;
        let inputs = doc
            .selected_objects()
            .filter(|object| ObjectSelectionFilter::Section.accepts_object(object))
            .collect::<Vec<_>>();
        if inputs.is_empty() {
            return Err(CommandError::NoObjectsSelected);
        }
        let mut output = Vec::new();
        for source in inputs {
            for geometry in section_geometry(
                source.geometry(),
                frame,
                length,
                options.extend,
                doc.tolerance(),
            )? {
                let attributes = if options.input_properties {
                    source.attributes().clone()
                } else {
                    viboceros_document::ObjectAttributes::on_layer(doc.current_layer_id())
                };
                output.push((geometry, attributes));
            }
        }
        let count = output.len();
        let mut ids = Vec::new();
        for (geometry, attributes) in output {
            ids.push(doc.add_geometry_with_attributes(geometry, attributes)?);
        }
        if options.group && !ids.is_empty() {
            doc.add_group(Some(doc.next_unused_group_name()), ids.iter().copied())?;
        }
        doc.select_command_results(ids)?;
        Ok(format!("Created {count} section object(s)"))
    }
}
