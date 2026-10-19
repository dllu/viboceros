//! Immutable source ownership and transactional replacement for point refinement.
use super::*;
use viboceros_document::{GeometrySnapshot, ObjectId};

const OPTIONS_USAGE: &str = "Direction=U|V|Both Symmetrical=Yes|No Midpoints=Yes|No Toggle";
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub direction: SurfaceKnotDirection,
    pub symmetrical: bool,
    pub midpoints: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            direction: SurfaceKnotDirection::U,
            symmetrical: false,
            midpoints: false,
        }
    }
}
impl Options {
    pub fn arguments(self) -> String {
        format!(
            "Direction={} Symmetrical={} Midpoints={}",
            match self.direction {
                SurfaceKnotDirection::U => "U",
                SurfaceKnotDirection::V => "V",
                SurfaceKnotDirection::Both => "Both",
            },
            if self.symmetrical { "Yes" } else { "No" },
            if self.midpoints { "Yes" } else { "No" }
        )
    }
    /// Consume a surface getter option without accepting bare numbers as points.
    pub fn accept(&mut self, word: &str) -> Result<bool, CommandError> {
        let word = word.trim_start_matches(['_', '-']);
        if word.eq_ignore_ascii_case("Toggle") {
            self.direction = match self.direction {
                SurfaceKnotDirection::U => SurfaceKnotDirection::V,
                SurfaceKnotDirection::V | SurfaceKnotDirection::Both => SurfaceKnotDirection::U,
            };
            return Ok(true);
        }
        let Some((key, value)) = word.split_once('=') else {
            return Ok(false);
        };
        if option_name_eq(key, "Direction") {
            self.direction = match value.trim_start_matches('_').to_ascii_lowercase().as_str() {
                "u" => SurfaceKnotDirection::U,
                "v" => SurfaceKnotDirection::V,
                "both" => SurfaceKnotDirection::Both,
                _ => return Err(CommandError::Usage(OPTIONS_USAGE)),
            };
        } else if option_name_eq(key, "Symmetrical") {
            self.symmetrical = parse_yes_no(value).ok_or(CommandError::Usage(OPTIONS_USAGE))?;
        } else if option_name_eq(key, "Midpoints") {
            self.midpoints = parse_yes_no(value).ok_or(CommandError::Usage(OPTIONS_USAGE))?;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
pub fn accepts(geometry: &Geometry) -> bool {
    geometry.curve_ref().is_some()
        || matches!(geometry, Geometry::NurbsSurface(_))
        || matches!(geometry,Geometry::Brep(b)if b.faces().len()==1)
}
pub(crate) fn surface_knot(
    geometry: Geometry,
    direction: SurfaceKnotDirection,
    parameter: Real,
    multiplicity: usize,
) -> Result<Geometry, CommandError> {
    Ok(match geometry {
        Geometry::Brep(b) => Geometry::Brep(b.try_insert_single_surface_knot_with(
            direction,
            parameter,
            multiplicity,
            &mut |s, d, t, m| knot_refinement::surface(s, d, t, m),
        )?),
        Geometry::NurbsSurface(s) => Geometry::NurbsSurface(knot_refinement::surface(
            &s,
            direction,
            parameter,
            multiplicity,
        )?),
        _ => unreachable!("resolved single surface"),
    })
}
/// Reuse an existing station only when parameter proximity and model-space
/// coincidence agree. Reflection/reprojection can straddle a knot by an ulp.
pub(crate) fn surface_knot_parameter(
    geometry: &Geometry,
    direction: SurfaceKnotDirection,
    t: Real,
    other: Real,
) -> Result<Real, CommandError> {
    let surface = surface_reference::surface(geometry, None).unwrap().0;
    let (domain, knots) = match direction {
        SurfaceKnotDirection::U => (surface.domain_u(), surface.knots_u()),
        SurfaceKnotDirection::V => (surface.domain_v(), surface.knots_v()),
        SurfaceKnotDirection::Both => unreachable!("single knot direction"),
    };
    let evaluate = |t| match direction {
        SurfaceKnotDirection::U => surface.evaluate(t, other),
        _ => surface.evaluate(other, t),
    };
    let point = evaluate(t)?;
    Ok(knots
        .iter()
        .copied()
        .filter(|k| domain.contains(k))
        .find(|&k| {
            (k - t).abs() <= Real::EPSILON.sqrt() * (*domain.end() - *domain.start())
                && evaluate(k).is_ok_and(|q| viboceros_geometry::curve_points_coincident(point, q))
        })
        .unwrap_or(t))
}
#[derive(Clone, Debug)]
pub struct Draft {
    pub(crate) command: &'static str,
    pub(crate) object: ObjectId,
    pub(crate) source: GeometrySnapshot,
    pub(crate) result: Geometry,
    pub(crate) tolerance: Tolerance,
    pub(crate) temporary: bool,
    pub(crate) picks: usize,
}
impl Draft {
    pub fn new(
        doc: &Document,
        object: ObjectId,
        subcurve: Option<[Real; 2]>,
        command: &'static str,
    ) -> Result<Self, CommandError> {
        let source = doc
            .object(object)
            .filter(|o| doc.is_object_selectable(o.id()) && accepts(o.geometry()))
            .ok_or(CommandError::Usage(
                "Refinement requires one curve or single surface",
            ))?
            .geometry_snapshot()
            .clone();
        let result = if let Some([a, b]) = subcurve {
            let curve = source
                .curve_ref()
                .ok_or(CommandError::Usage("Temporary refinement requires a curve"))?
                .to_owned();
            Geometry::from(curve_segments::directed(&curve, a, b)?)
        } else {
            (*source).clone()
        };
        Ok(Self {
            command,
            object,
            source,
            result,
            tolerance: doc.tolerance(),
            temporary: subcurve.is_some(),
            picks: 0,
        })
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && doc.is_object_selectable(self.object)
            && doc
                .object(self.object)
                .is_some_and(|o| self.source.shares_storage_with(o.geometry_snapshot()))
    }
    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn geometry(&self) -> &Geometry {
        &self.result
    }
    pub fn picks(&self) -> usize {
        self.picks
    }
    /// Commit inside the caller's command transaction, after validating the source.
    pub fn commit(&self, doc: &mut Document, postselected: bool) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage(
                "Refinement source changed; start the command again",
            ));
        }
        if self.picks == 0 {
            return Ok(format!("{}: no edits", self.command));
        }
        let changed = self.result != *self.source;
        if changed && (postselected || self.temporary) {
            doc.release_command_selection_on_history_replay([self.object])?;
            doc.clear_selection();
        }
        if self.temporary {
            doc.add_geometry(self.result.clone())?;
        } else {
            // Trimmed faces keep their existing B-rep container metadata.
            let retain = matches!(&*self.source,Geometry::Brep(b)if !b.faces()[0].is_untrimmed(self.tolerance)?);
            let text = doc
                .object(self.object)
                .unwrap()
                .geometry_user_text()
                .clone();
            doc.replace_object_geometries([(self.object, self.result.clone())])?;
            if retain {
                for (key, value) in text {
                    doc.set_object_geometry_user_text([self.object], &key, Some(&value))?;
                }
            } else if !changed {
                for key in text.keys() {
                    doc.set_object_geometry_user_text([self.object], key, None)?;
                }
            }
        }
        Ok(format!("{}: {} edit(s)", self.command, self.picks))
    }
}
