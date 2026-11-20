//! Curve division with staged point/split outputs and source-domain intervals.
use super::*;
use crate::curve_reference::CurvePortion;
use viboceros_geometry::CurveDivisionPoint;
pub mod prompt;
#[cfg(test)]
mod tests;

pub const USAGE: &str = "Divide count|Length length|EqualChordLength length [MarkEnds=Yes|No] [Split=Yes|No] [DeleteRemainder=Yes|No] [GroupOutput=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Specification {
    Count(usize),
    Length(Real),
    Chord(Real),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub specification: Specification,
    pub mark_ends: bool,
    pub split: bool,
    pub delete_remainder: bool,
    pub group_output: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Traversal {
    pub seam: Option<Real>,
    pub flip_seam: bool,
    pub reverse_measurement: bool,
}

/// One selected whole curve or a privately prepared portion of a curve or edge.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub object: ObjectId,
    pub portion: Option<std::sync::Arc<CurvePortion>>,
    pub traversal: Traversal,
}

impl Source {
    pub fn whole(object: ObjectId) -> Self {
        Self {
            object,
            portion: None,
            traversal: Traversal::default(),
        }
    }

    fn curve<'a>(&'a self, geometry: &'a Geometry) -> Result<CurveRef<'a>, CommandError> {
        if let Some(portion) = &self.portion {
            return Ok(portion.curve_ref());
        }
        geometry
            .curve_ref()
            .ok_or(CommandError::UnsupportedDivideGeometry)
    }
}

impl Options {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        Self::parse_with_defaults(arguments, prompt::Prompt::default().options)
    }

    fn parse_with_defaults(arguments: &[&str], defaults: Self) -> Result<Self, CommandError> {
        let mut positional = Vec::new();
        let mut seen = BTreeSet::new();
        let mut flags = [
            defaults.mark_ends,
            defaults.split,
            defaults.delete_remainder,
            defaults.group_output,
        ];
        for word in arguments {
            let word = word.trim_start_matches('_');
            if let Some((key, value)) = word.split_once('=') {
                let key = key.to_ascii_lowercase();
                if !seen.insert(key.clone()) {
                    return Err(CommandError::Usage(USAGE));
                }
                let index = match key.as_str() {
                    "markends" => 0,
                    "split" => 1,
                    "deleteremainder" => 2,
                    "groupoutput" => 3,
                    _ => return Err(CommandError::Usage(USAGE)),
                };
                flags[index] = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                    "yes" => true,
                    "no" => false,
                    _ => return Err(CommandError::Usage(USAGE)),
                };
            } else if word.eq_ignore_ascii_case("MarkEnds") {
                if !seen.insert("markends".into()) {
                    return Err(CommandError::Usage(USAGE));
                }
                flags[0] = true;
            } else {
                positional.push(word);
            }
        }
        let specification = match positional.as_slice() {
            [number] => Specification::Count(
                number
                    .parse()
                    .map_err(|_| CommandError::InvalidInteger((*number).into()))?,
            ),
            [mode, value] if mode.eq_ignore_ascii_case("Length") => {
                Specification::Length(parse_finite_real(value)?)
            }
            [mode, value] if mode.eq_ignore_ascii_case("EqualChordLength") => {
                Specification::Chord(parse_finite_real(value)?)
            }
            _ => return Err(CommandError::Usage(USAGE)),
        };
        match specification {
            Specification::Count(count) if count == 0 || count > MAX_CURVE_DIVISION_POINTS => {
                return Err(GeometryError::InvalidCurveDivisionCount {
                    actual: count,
                    maximum: MAX_CURVE_DIVISION_POINTS,
                }
                .into());
            }
            Specification::Length(value) | Specification::Chord(value) if value <= 0. => {
                return Err(GeometryError::InvalidCurveDivisionLength.into());
            }
            _ => {}
        }
        Ok(Self {
            specification,
            mark_ends: flags[0],
            split: flags[1],
            delete_remainder: flags[2],
            group_output: flags[3],
        })
    }
}

impl CommandRegistry {
    pub fn divide_prompt_default(&self) -> prompt::Prompt {
        self.divide_preferences.get().for_invocation()
    }

    /// Remember valid preview edits independently of document history or cancellation.
    pub fn accept_divide_prompt(&self, prompt: prompt::Prompt) -> Result<(), CommandError> {
        Options::parse(
            &prompt
                .command_line()
                .split_whitespace()
                .skip(1)
                .collect::<Vec<_>>(),
        )?;
        self.divide_preferences
            .set(prompt.with_options(prompt.options));
        Ok(())
    }

    /// Apply the same staged result shown by the interactive preview.
    pub fn finish_divide_prompt(
        &self,
        doc: &mut Document,
        prompt: prompt::Prompt,
        sources: &[Source],
    ) -> Result<String, CommandError> {
        let options = prompt.options;
        let message = run_command_transaction(doc, "Divide", |doc| {
            prepare(doc, options, sources)?.apply(doc, options)
        })?;
        self.divide_preferences.set(prompt.with_options(options));
        Ok(message)
    }
}

pub(super) struct DivideCommand(pub(super) std::sync::Arc<remembered::Remembered<prompt::Prompt>>);
impl Command for DivideCommand {
    fn name(&self) -> &'static str {
        "Divide"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["Div"]
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        if args.is_empty() {
            return Ok(Some(ObjectSelectionPrompt {
                command: "Divide",
                filter: ObjectSelectionFilter::Curves,
                options: Vec::new(),
                menus: Vec::new(),
                choices: Vec::new(),
                workflow: ObjectSelectionWorkflow::ConfirmAfterSelection,
            }));
        }
        Options::parse_with_defaults(args, self.0.get().options)?;
        Ok(Some(ObjectSelectionPrompt {
            command: "Divide",
            filter: ObjectSelectionFilter::Curves,
            options: Vec::new(),
            menus: Vec::new(),
            choices: Vec::new(),
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
        }))
    }
    fn object_selection_confirmation(
        &self,
        _document: &Document,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        self.object_selection_prompt(arguments)
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let preferences = self.0.get();
        let options = Options::parse_with_defaults(args, preferences.options)?;
        let message = prepare(doc, options, &selected_sources(doc))?.apply(doc, options)?;
        self.0.set(preferences.with_options(options));
        Ok(message)
    }
}

struct Prepared {
    source_count: usize,
    replaced_sources: Vec<ObjectId>,
    output: Vec<(Geometry, ObjectAttributes)>,
    batches: Vec<usize>,
}

impl Prepared {
    fn apply(self, doc: &mut Document, options: Options) -> Result<String, CommandError> {
        let Self {
            source_count,
            replaced_sources,
            output,
            batches,
        } = self;
        if output.is_empty() {
            return Err(CommandError::NoCurveDivisionPoints);
        }
        if !replaced_sources.is_empty() {
            doc.delete_objects(replaced_sources)?;
        }
        let ids = output
            .into_iter()
            .map(|(g, a)| doc.add_geometry_with_attributes(g, a))
            .collect::<Result<Vec<_>, _>>()?;
        if options.group_output && !options.split {
            let mut first = 0;
            for end in batches {
                if end > first {
                    doc.add_group(None, ids[first..end].iter().copied())?;
                }
                first = end;
            }
        }
        replace_selection(doc, ids.iter().copied())?;
        Ok(format!(
            "Divided {} curve(s), adding {} {}",
            source_count,
            ids.len(),
            if options.split {
                "curve(s)"
            } else {
                "point(s)"
            }
        ))
    }
}

/// Read-only station markers from the same output preparation as execution.
/// Split markers show each piece's boundaries, including a retained remainder.
pub fn preview_points(doc: &Document, options: Options) -> Result<Vec<Point3>, CommandError> {
    preview_source_points(doc, options, &selected_sources(doc))
}

pub fn preview_source_points(
    doc: &Document,
    options: Options,
    sources: &[Source],
) -> Result<Vec<Point3>, CommandError> {
    let prepared = prepare(doc, options, sources)?;
    let mut points = Vec::new();
    for (geometry, _) in prepared.output {
        match geometry {
            Geometry::Point(point) => points.push(point),
            geometry => {
                let curve = geometry
                    .curve_ref()
                    .ok_or(CommandError::UnsupportedDivideGeometry)?;
                points.push(curve.start_point()?);
                points.push(curve.end_point()?);
            }
        }
    }
    Ok(points)
}

fn selected_sources(doc: &Document) -> Vec<Source> {
    doc.selected_objects()
        .map(|object| Source::whole(object.id()))
        .collect()
}

fn prepare(doc: &Document, options: Options, sources: &[Source]) -> Result<Prepared, CommandError> {
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let mut output = Vec::new();
    let mut batches = Vec::new();
    let mut replaced_sources = Vec::new();
    let tolerance = doc.tolerance();
    let point_attributes = ObjectAttributes::on_layer(doc.current_layer_id());
    let indexed = (sources.len() > 16).then(|| {
        let ids = sources
            .iter()
            .map(|source| source.object)
            .collect::<BTreeSet<_>>();
        doc.objects()
            .filter(|object| ids.contains(&object.id()))
            .map(|object| (object.id(), object))
            .collect::<BTreeMap<_, _>>()
    });
    for source in sources {
        let object = match &indexed {
            Some(objects) => objects.get(&source.object).copied(),
            None => doc.object(source.object),
        }
        .ok_or(DocumentError::ObjectNotFound(source.object))?;
        let attributes = object.attributes();
        let curve = source.curve(object.geometry())?;
        let traversal = source.traversal;
        let adjusted = traversal
            .seam
            .map(|parameter| prepare_curve_seam(&curve.to_owned(), parameter))
            .transpose()?;
        let curve = adjusted.as_ref().map_or(curve, Curve3::as_ref);
        let measured_backwards = match options.specification {
            Specification::Count(_) => traversal.flip_seam,
            Specification::Length(_) => traversal.flip_seam ^ traversal.reverse_measurement,
            Specification::Chord(_) => false,
        };
        let reversed = measured_backwards
            .then(|| curve.to_owned().reversed(tolerance))
            .transpose()?;
        let measured = reversed.as_ref().map_or(curve, Curve3::as_ref);
        let mut stations = stations(measured, options.specification, tolerance)?;
        if options.split {
            if source.portion.is_none() {
                replaced_sources.push(source.object);
            }
            let end = *measured.domain().end();
            // Rhino omits the leading partial piece after reversing Length.
            // If there are no interior stations it can still retain the whole curve.
            if stations.last().is_some_and(|s| s.parameter < end)
                && !options.delete_remainder
                && (!measured_backwards || stations.len() == 1)
            {
                stations.push(CurveDivisionPoint {
                    parameter: end,
                    point: measured.end_point()?,
                });
            }
            if measured_backwards {
                stations.reverse();
                // Curve reversal negates the native domain. Trim and emit the
                // pieces in the original parameter direction.
                for station in &mut stations {
                    station.parameter = -station.parameter;
                }
            }
            for station in &stations {
                if !curve
                    .evaluate(station.parameter)?
                    .is_near(station.point, tolerance)
                {
                    return Err(GeometryError::UnrepresentableCurveDivisionParameter.into());
                }
            }
            let owned = curve.to_owned();
            for pair in stations.windows(2) {
                let fragment = owned.try_trimmed(pair[0].parameter..=pair[1].parameter)?;
                output.push((Geometry::from(fragment), attributes.clone()));
            }
        } else {
            let closed = measured.is_closed()?;
            if closed {
                if stations.len() > 1
                    && stations
                        .last()
                        .is_some_and(|s| s.point.is_near(stations[0].point, tolerance))
                {
                    stations.pop();
                }
            } else if !matches!(options.specification, Specification::Chord(_)) {
                let end = measured.end_point()?;
                if !options.mark_ends {
                    if stations
                        .last()
                        .is_some_and(|s| s.point.is_near(end, tolerance))
                    {
                        stations.pop();
                    }
                    if !stations.is_empty() {
                        stations.remove(0);
                    }
                }
            }
            if measured_backwards {
                stations.reverse();
            }
            output.extend(
                stations
                    .into_iter()
                    .map(|s| (Geometry::Point(s.point), point_attributes.clone())),
            );
        }
        if output.len() > MAX_CURVE_DIVISION_POINTS {
            return Err(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            }
            .into());
        }
        batches.push(output.len());
    }
    Ok(Prepared {
        source_count: sources.len(),
        replaced_sources,
        output,
        batches,
    })
}

fn stations(
    curve: CurveRef<'_>,
    specification: Specification,
    tolerance: Tolerance,
) -> Result<Vec<CurveDivisionPoint>, GeometryError> {
    if let Specification::Chord(distance) = specification {
        return curve.divide_by_chord_length(distance, tolerance);
    }
    let samples = match specification {
        Specification::Count(count) => curve.divide_by_count_samples(count, true, tolerance)?,
        Specification::Length(length) => curve.divide_by_length_samples(length, true, tolerance)?,
        Specification::Chord(_) => unreachable!(),
    };
    Ok(samples
        .into_iter()
        .map(|s| CurveDivisionPoint {
            parameter: s.parameter(),
            point: s.point(),
        })
        .collect())
}
