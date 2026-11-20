//! Match command-first pick order, temporary portions, and source replacement.
use super::*;
use crate::curve_pair_edit::{Plan, Target, retain_line_domain};
use viboceros_document::Object;
use viboceros_geometry::{CurveRef, CurveSegment3, PolyCurve3, Polyline3};

#[derive(Clone)]
pub struct Source {
    object: Object,
    edit_start: bool,
    portion: Option<Curve3>,
}

impl Source {
    pub fn capture(
        document: &Document,
        object: ObjectId,
        point: Option<Point3>,
        portion: Option<Curve3>,
    ) -> Result<Self, CommandError> {
        let object = document
            .object(object)
            .filter(|o| document.is_object_selectable(o.id()))
            .cloned()
            .ok_or(CommandError::ConnectRequiresTwoCurves)?;
        let curve = portion
            .as_ref()
            .map(Curve3::as_ref)
            .or_else(|| object.geometry().curve_ref())
            .ok_or(CommandError::ConnectRequiresTwoCurves)?;
        if curve.is_closed()? {
            return Err(CommandError::Usage("Select an open curve"));
        }
        // A viewport pick identifies the end to retain. A temporary portion
        // is picked at the last point used to define it.
        let edit_start = if let Some(point) = point {
            point.distance_to(curve.end_point()?)? < point.distance_to(curve.start_point()?)?
        } else {
            portion.is_some()
        };
        Ok(Self {
            object,
            edit_start,
            portion,
        })
    }

    pub fn id(&self) -> ObjectId {
        self.object.id()
    }

    pub fn is_current(&self, document: &Document) -> bool {
        crate::curve_pair_edit::source_is_current(document, &self.object)
    }

    pub fn edits_start(&self) -> bool {
        self.edit_start
    }

    fn curve(&self) -> Result<viboceros_geometry::CurveRef<'_>, CommandError> {
        self.portion
            .as_ref()
            .map(Curve3::as_ref)
            .or_else(|| self.object.geometry().curve_ref())
            .ok_or(CommandError::ConnectRequiresTwoCurves)
    }
}

pub fn preview(
    sources: [&Source; 2],
    options: Options,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if sources[0].id() == sources[1].id() {
        return Err(CommandError::ConnectRequiresTwoCurves);
    }
    let originals = [sources[0].curve()?, sources[1].curve()?];
    let editable = |curve: CurveRef<'_>| -> Result<Curve3, GeometryError> {
        if let CurveRef::Polyline(polyline) = curve {
            Ok(Curve3::PolyCurve(PolyCurve3::try_with_segment_domains(
                polyline.segments().collect(),
                polyline.parameters().to_vec(),
            )?))
        } else {
            Ok(curve.to_owned())
        }
    };
    let [a, b] = [editable(originals[0])?, editable(originals[1])?];
    let endpoints = |curve: CurveRef<'_>| -> Result<[Point3; 2], GeometryError> {
        Ok([curve.end_point()?, curve.start_point()?])
    };
    let [p, q] = [endpoints(originals[0])?, endpoints(originals[1])?];
    let connect = |[start_a, start_b]: [bool; 2]| {
        super::curve_connections::connect_parts(
            (&a, p[usize::from(start_a)]),
            (&b, q[usize::from(start_b)]),
            options.arc_extension,
            options.other_extension,
            tolerance,
        )
    };
    let [start_a, start_b] = sources.map(|source| source.edit_start);
    let preferred = [start_a, start_b];
    let (parts, ends) = match connect(preferred) {
        Ok(parts) => (parts, preferred),
        Err(error @ CommandError::Geometry(GeometryError::InvalidPolyCurve { .. })) => [
            [start_a, !start_b],
            [!start_a, start_b],
            [!start_a, !start_b],
        ]
        .into_iter()
        .find_map(|ends| connect(ends).ok().map(|parts| (parts, ends)))
        .ok_or(error)?,
        Err(error) => return Err(error),
    };
    let [first, second]: [Curve3; 2] = parts.try_into().expect("Connect produces two curves");
    let first = retain_source_chart(originals[0], first, tolerance)?;
    let second = retain_source_chart(originals[1], second, tolerance)?;
    if !options.join {
        return Ok(vec![Geometry::from(first), Geometry::from(second)]);
    }
    let first = if ends[0] {
        first.reversed(tolerance)?
    } else {
        first
    }
    .to_polycurve()?;
    let second = if ends[1] {
        second
    } else {
        second.reversed(tolerance)?
    }
    .to_polycurve()?;
    let segments = first
        .segments()
        .iter()
        .chain(second.segments())
        .cloned()
        .collect();
    Ok(vec![Geometry::PolyCurve(PolyCurve3::try_new::<
        CurveSegment3,
    >(segments)?)])
}

fn retain_source_chart(
    source: CurveRef<'_>,
    mut result: Curve3,
    tolerance: Tolerance,
) -> Result<Curve3, CommandError> {
    if let CurveRef::Line(original) = source {
        retain_line_domain(&Curve3::Line(*original), &mut result, tolerance)?;
    }
    let (CurveRef::Polyline(original), Curve3::PolyCurve(polycurve)) = (source, &result) else {
        return Ok(result);
    };
    if polycurve.segments().len() != original.segment_count()
        || !polycurve
            .segments()
            .iter()
            .all(|segment| matches!(segment, CurveSegment3::Line(_)))
    {
        return Ok(result);
    }
    let mut points = Vec::with_capacity(original.vertices().len());
    let mut parameters = Vec::with_capacity(points.capacity());
    for (i, (old, new)) in original.segments().zip(polycurve.segments()).enumerate() {
        let CurveSegment3::Line(line) = new else {
            unreachable!("checked straight segments")
        };
        let mut result = Curve3::Line(*line);
        retain_line_domain(&Curve3::Line(old), &mut result, tolerance)?;
        let Curve3::Line(line) = result else {
            unreachable!("line chart retention preserves type")
        };
        if i == 0 {
            points.push(line.start());
            parameters.push(*line.domain().start());
        }
        points.push(line.end());
        parameters.push(*line.domain().end());
    }
    Ok(Curve3::Polyline(Polyline3::try_with_parameters(
        points, parameters, tolerance,
    )?))
}

/// Prepare the ordered interactive picks in a single edit.
pub fn prepare(
    document: &Document,
    sources: [&Source; 2],
    options: Options,
) -> Result<Prepared, CommandError> {
    if sources.iter().any(|source| !source.is_current(document)) {
        return Err(CommandError::ConnectRequiresTwoCurves);
    }
    let outputs = preview(sources, options, document.tolerance())?;
    let mut deleted = Vec::new();
    let targets = if options.join {
        if sources[1].portion.is_none() {
            deleted.push(1);
        }
        vec![if sources[0].portion.is_some() {
            Target::New
        } else {
            Target::Source(0)
        }]
    } else {
        sources
            .iter()
            .enumerate()
            .map(|(i, source)| {
                if source.portion.is_some() {
                    Target::New
                } else {
                    Target::Source(i)
                }
            })
            .collect()
    };
    Ok(Prepared::new(
        document,
        sources.map(|source| source.object.clone()),
        Plan {
            outputs,
            targets,
            deleted,
        },
        "Connect",
    ))
}
