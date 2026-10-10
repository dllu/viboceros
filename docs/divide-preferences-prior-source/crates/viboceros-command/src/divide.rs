//! Curve division with staged point/split outputs and source-domain intervals.
use super::*;
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
impl Options {
    pub fn parse(arguments: &[&str]) -> Result<Self, CommandError> {
        let mut positional = Vec::new();
        let mut seen = BTreeSet::new();
        let mut flags = [false; 4];
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

pub struct DivideCommand;
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
        Options::parse(args)?;
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
        let options = Options::parse(args)?;
        let Prepared {
            source_ids,
            output,
            batches,
        } = prepare(doc, options)?;
        if output.is_empty() {
            return Err(CommandError::NoCurveDivisionPoints);
        }
        if options.split {
            doc.delete_objects(source_ids.iter().copied())?;
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
            source_ids.len(),
            ids.len(),
            if options.split {
                "curve(s)"
            } else {
                "point(s)"
            }
        ))
    }
}

struct Prepared {
    source_ids: Vec<ObjectId>,
    output: Vec<(Geometry, ObjectAttributes)>,
    batches: Vec<usize>,
}

/// Read-only station markers from the same output preparation as execution.
/// Split markers show each piece's boundaries, including a retained remainder.
pub fn preview_points(doc: &Document, options: Options) -> Result<Vec<Point3>, CommandError> {
    let prepared = prepare(doc, options)?;
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

fn prepare(doc: &Document, options: Options) -> Result<Prepared, CommandError> {
    let sources = doc
        .selected_objects()
        .map(|o| (o.id(), o.geometry().clone(), o.attributes().clone()))
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err(CommandError::NoObjectsSelected);
    }
    let mut output = Vec::new();
    let mut batches = Vec::new();
    for (_, geometry, attributes) in &sources {
        let curve = geometry
            .curve_ref()
            .ok_or(CommandError::UnsupportedDivideGeometry)?;
        let mut stations = stations(curve, options.specification, doc.tolerance())?;
        if options.split {
            for station in &stations {
                if !curve
                    .evaluate(station.parameter)?
                    .is_near(station.point, doc.tolerance())
                {
                    return Err(GeometryError::UnrepresentableCurveDivisionParameter.into());
                }
            }
            let end = *curve.domain().end();
            if stations.last().is_some_and(|s| s.parameter < end) && !options.delete_remainder {
                stations.push(CurveDivisionPoint {
                    parameter: end,
                    point: curve.end_point()?,
                });
            }
            let owned = curve.to_owned();
            for pair in stations.windows(2) {
                let fragment = owned.try_trimmed(pair[0].parameter..=pair[1].parameter)?;
                output.push((Geometry::from(fragment), attributes.clone()));
            }
        } else {
            let closed = curve.is_closed()?;
            if closed {
                if stations.len() > 1
                    && stations
                        .last()
                        .is_some_and(|s| s.point.is_near(stations[0].point, doc.tolerance()))
                {
                    stations.pop();
                }
            } else if !matches!(options.specification, Specification::Chord(_)) {
                let end = curve.end_point()?;
                if !options.mark_ends {
                    if stations
                        .last()
                        .is_some_and(|s| s.point.is_near(end, doc.tolerance()))
                    {
                        stations.pop();
                    }
                    if !stations.is_empty() {
                        stations.remove(0);
                    }
                }
            }
            let attributes = ObjectAttributes::on_layer(doc.current_layer_id());
            output.extend(
                stations
                    .into_iter()
                    .map(|s| (Geometry::Point(s.point), attributes.clone())),
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
        source_ids: sources.iter().map(|s| s.0).collect(),
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
