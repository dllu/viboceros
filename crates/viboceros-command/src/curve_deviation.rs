//! Transient two-curve deviation reports and optional permanent witness marks.
use super::*;
use std::sync::{Arc, Mutex};
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{CurveDistanceExtrema, CurveDistanceWitness, CurveRef};

#[cfg(test)]
mod tests;
pub const USAGE: &str = "CrvDeviation [KeepMarks=Yes|No] | CrvDeviation End|Cancel";

#[derive(Clone, Debug)]
pub struct View {
    pub extrema: CurveDistanceExtrema,
    pub ends: [CurveDistanceWitness; 2],
    pub keep_marks: bool,
    pub marker_tolerance: Real,
    /// Overlap and correspondence are independent policies, and complete Rhino
    /// parity is open, including asymmetric line pairs.
    pub approximate_overlap: bool,
}
impl View {
    pub fn report(&self) -> String {
        let end = |label: &str, w: &CurveDistanceWitness| {
            let points = w
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "  crv{}({}) = ({}, {}, {})",
                        ['A', 'B'][i],
                        w.parameters[i],
                        p.x(),
                        p.y(),
                        p.z()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{label}: distance = {}\n{points}\n", w.distance)
        };
        format!(
            "Overlap interval 1 of 1:\n{}{}Minimum deviation = {}\nMaximum deviation = {}\n{}Press Enter when done (KeepMarks={})",
            end("start", &self.ends[0]),
            end("end", &self.ends[1]),
            self.extrema.minimum.distance,
            self.extrema.maximum.distance,
            if self.approximate_overlap {
                "Overlap and correspondence can differ from Rhino.\n"
            } else {
                ""
            },
            if self.keep_marks { "Yes" } else { "No" },
        )
    }
    /// Paint interval ends first, then maximum and minimum. Tied witnesses
    /// remain separate occurrences, as they do in permanent marker geometry.
    pub fn markers(&self) -> [(&CurveDistanceWitness, [u8; 3]); 4] {
        [
            (&self.ends[0], [255, 255, 255]),
            (&self.ends[1], [255, 255, 255]),
            (&self.extrema.maximum, [255, 0, 0]),
            (&self.extrema.minimum, [0, 255, 0]),
        ]
    }
}

pub fn analyze(
    first: CurveRef<'_>,
    second: CurveRef<'_>,
    tolerance: Tolerance,
) -> Result<Option<View>, CommandError> {
    if let (CurveRef::Line(a), CurveRef::Line(b)) = (first, second) {
        return Ok(viboceros_geometry::line_deviation(*a, *b)?.map(|r| View {
            extrema: r.extrema,
            ends: r.ends,
            keep_marks: false,
            marker_tolerance: tolerance.absolute(),
            approximate_overlap: true,
        }));
    }
    if let (CurveRef::Circle(a), CurveRef::Circle(b)) = (first, second) {
        // On parallel circle planes, mutual nearest-point maps can wind once
        // only while each projected center lies strictly inside the other
        // circle. At a boundary the nearest angular correspondence is singular.
        let normals = a.normal()?.as_vector().cross(b.normal()?.as_vector())?;
        if normals.length()? <= 64. * f64::EPSILON {
            let projected = viboceros_geometry::PointProjection3::onto_plane(
                a.center(),
                a.normal()?.as_vector(),
            )?
            .project(b.center())?;
            if projected.distance_to(a.center())? >= a.radius().min(b.radius()) {
                return Ok(None);
            }
        }
    }
    #[cfg(feature = "native-smlib")]
    {
        let Some(intervals) = viboceros_smlib::projected_curve_intervals(first, second, tolerance)?
        else {
            return Ok(None);
        };
        let intervals = viboceros_geometry::preserve_transverse_curve_ends(
            first, second, intervals, tolerance,
        )?;
        let Some(extrema) =
            viboceros_smlib::curve_distance_extrema(first, second, intervals, tolerance)?
        else {
            return Ok(None);
        };
        let curves = [first, second];
        let sources = [first.to_nurbs()?, second.to_nurbs()?];
        let end = |f: [f64; 2]| -> Result<CurveDistanceWitness, CommandError> {
            let points = [
                sources[0].evaluate(sources[0].parameter_at(f[0])?)?,
                sources[1].evaluate(sources[1].parameter_at(f[1])?)?,
            ];
            let mut parameters = [0.; 2];
            for i in 0..2 {
                parameters[i] = curves[i].parameter_from_nurbs(sources[i].parameter_at(f[i])?)?;
            }
            Ok(CurveDistanceWitness {
                normalized_parameters: f,
                parameters,
                points,
                distance: points[0].distance_to(points[1])?,
            })
        };
        let start = sources[0].evaluate(sources[0].parameter_at(intervals[0][0])?)?;
        let b = intervals[1]
            .map(|t| sources[1].evaluate(sources[1].parameter_at(t)?))
            .into_iter()
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let reverse = start.distance_to(b[1])? < start.distance_to(b[0])?;
        let ends = [
            end([intervals[0][0], intervals[1][usize::from(reverse)]])?,
            end([intervals[0][1], intervals[1][usize::from(!reverse)]])?,
        ];
        Ok(Some(View {
            extrema,
            ends,
            keep_marks: false,
            marker_tolerance: tolerance.absolute(),
            approximate_overlap: true,
        }))
    }
    #[cfg(not(feature = "native-smlib"))]
    {
        let _ = tolerance;
        Err(CommandError::Usage(
            "CrvDeviation for curved inputs requires native-smlib",
        ))
    }
}

#[derive(Clone)]
struct Active {
    ids: [ObjectId; 2],
    sources: [GeometrySnapshot; 2],
    tolerance: Tolerance,
    view: View,
}
impl Active {
    fn valid(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.ids.iter().zip(&self.sources).all(|(id, s)| {
                doc.object(*id)
                    .is_some_and(|o| s.shares_storage_with(o.geometry_snapshot()))
            })
    }
}
#[derive(Default)]
struct State {
    active: Option<Active>,
    keep_marks: bool,
}
#[derive(Default)]
pub(super) struct Session(Mutex<State>);
impl Session {
    pub(super) fn view(&self, doc: &Document) -> Option<View> {
        let mut state = self.0.lock().expect("curve deviation session");
        if state.active.as_ref().is_some_and(|a| !a.valid(doc)) {
            state.active = None;
        }
        state.active.as_ref().map(|a| a.view.clone())
    }
    pub(super) fn clear(&self) {
        self.0.lock().expect("curve deviation session").active = None;
    }
}

fn parse<'a>(args: &[&'a str]) -> Result<(Option<&'a str>, Option<bool>), CommandError> {
    let mut action = None;
    let mut keep = None;
    for a in args {
        let a = a.trim_start_matches('_');
        if let Some((key, v)) = a.split_once('=') {
            if !key.eq_ignore_ascii_case("KeepMarks") || keep.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            keep = Some(parse_yes_no(v.trim_start_matches('_')).ok_or(CommandError::Usage(USAGE))?);
        } else if action.is_none()
            && (a.eq_ignore_ascii_case("End") || a.eq_ignore_ascii_case("Cancel"))
        {
            action = Some(a);
        } else {
            return Err(CommandError::Usage(USAGE));
        }
    }
    if action.is_some_and(|a| a.eq_ignore_ascii_case("Cancel")) && keep.is_some() {
        return Err(CommandError::Usage(USAGE));
    }
    Ok((action, keep))
}

pub(super) struct CurveDeviationCommand(pub Arc<Session>);
impl Command for CurveDeviationCommand {
    fn name(&self) -> &'static str {
        "CrvDeviation"
    }
    fn object_selection_prompt(
        &self,
        args: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let (action, keep) = parse(args)?;
        if action.is_some() || keep.is_some() && self.0.0.lock().unwrap().active.is_some() {
            return Ok(None);
        }
        Ok(Some(ObjectSelectionPrompt {
            command: "CrvDeviation",
            filter: ObjectSelectionFilter::Curves,
            workflow: ObjectSelectionWorkflow::OptionsDuringSelection,
            options: vec![BooleanSelectionOption {
                name: "KeepMarks",
                value: keep.unwrap_or(self.0.0.lock().unwrap().keep_marks),
                aliases: &[],
            }],
            menus: vec![],
            choices: vec![],
        }))
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        let (action, keep) = parse(args)?;
        let mut state = self.0.0.lock().expect("curve deviation session");
        if action.is_some_and(|a| a.eq_ignore_ascii_case("Cancel")) {
            state.active = None;
            return Ok("CrvDeviation canceled".into());
        }
        let keep = keep.unwrap_or(state.keep_marks);
        if action.is_some() {
            let active = state
                .active
                .as_ref()
                .ok_or(CommandError::Usage("No active CrvDeviation report"))?;
            if !active.valid(doc) {
                state.active = None;
                return Err(CommandError::Usage(
                    "CrvDeviation sources changed; run it again",
                ));
            }
            if keep {
                // Prepare every line before inserting anything. The registry
                // transaction rolls back admission failures as one model edit.
                let mut geometry = Vec::new();
                for (w, c) in active.view.markers() {
                    let color = ColorRgb::new(c[0], c[1], c[2]);
                    geometry.push((Geometry::Point(w.points[0]), color));
                    if w.distance > active.view.marker_tolerance {
                        geometry.push((
                            Geometry::Line(LineSegment::try_new(
                                w.points[0],
                                w.points[1],
                                Tolerance::NUMERICAL_VALIDATION,
                            )?),
                            color,
                        ));
                    }
                    if w.distance > active.view.marker_tolerance {
                        geometry.push((Geometry::Point(w.points[1]), color));
                    }
                }
                for (g, c) in geometry {
                    doc.add_geometry_with_attributes(
                        g,
                        ObjectAttributes::on_layer(doc.current_layer_id()).with_object_color(c),
                    )?;
                }
            }
            state.active = None;
            state.keep_marks = keep;
            return Ok(if keep {
                "CrvDeviation ended; marks kept"
            } else {
                "CrvDeviation ended"
            }
            .into());
        }
        if !args.is_empty()
            && let Some(active) = state.active.as_mut().filter(|a| a.valid(doc))
        {
            active.view.keep_marks = keep;
            let message = active.view.report();
            state.keep_marks = keep;
            return Ok(message);
        }
        let objects = doc
            .selected_objects()
            .filter(|o| o.geometry().curve_ref().is_some())
            .collect::<Vec<_>>();
        if objects.len() != 2 {
            return Err(CommandError::Usage(
                "CrvDeviation requires exactly two curves",
            ));
        }
        let report = analyze(
            objects[0].geometry().curve_ref().unwrap(),
            objects[1].geometry().curve_ref().unwrap(),
            doc.tolerance(),
        )?;
        state.keep_marks = keep;
        state.active = report.map(|mut view| {
            view.keep_marks = keep;
            Active {
                ids: [objects[0].id(), objects[1].id()],
                sources: [
                    objects[0].geometry_snapshot().clone(),
                    objects[1].geometry_snapshot().clone(),
                ],
                tolerance: doc.tolerance(),
                view,
            }
        });
        Ok(state
            .active
            .as_ref()
            .map_or_else(|| "Curves do not overlap.".into(), |a| a.view.report()))
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        let result = self.run(doc, args)?;
        doc.clear_selection();
        Ok(result)
    }
}
