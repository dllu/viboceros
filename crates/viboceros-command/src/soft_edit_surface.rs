//! Repeated surface soft edits with trim-aware edit-station picking.
use super::*;
use crate::remembered::Remembered;
use viboceros_geometry::SurfaceSoftEdit;

pub const USAGE: &str = "SoftEditSrf reference target [U_Distance=value] [V_Distance=value] [FixEdges=Yes|No] [Copy=Yes|No] [DirConstraint=None|SrfNormal|UTangent|VTangent]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Constraint {
    None,
    #[default]
    SrfNormal,
    UTangent,
    VTangent,
}
impl Constraint {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::SrfNormal => "SrfNormal",
            Self::UTangent => "UTangent",
            Self::VTangent => "VTangent",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub distances: [Real; 2],
    pub fix_edges: bool,
    pub copy: bool,
    pub constraint: Constraint,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            distances: [1., 1.],
            fix_edges: false,
            copy: false,
            constraint: Constraint::default(),
        }
    }
}
impl Options {
    pub fn accept(&mut self, word: &str) -> Result<bool, CommandError> {
        let Some((key, value)) = word.split_once('=') else {
            return Ok(false);
        };
        let mut next = *self;
        if option_name_eq(key, "U_Distance") || option_name_eq(key, "UDistance") {
            next.distances[0] = parse_finite_real(value)?;
        } else if option_name_eq(key, "V_Distance") || option_name_eq(key, "VDistance") {
            next.distances[1] = parse_finite_real(value)?;
        } else if option_name_eq(key, "FixEdges") {
            next.fix_edges = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(key, "Copy") {
            next.copy = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?;
        } else if option_name_eq(key, "DirConstraint") {
            next.constraint = [
                Constraint::None,
                Constraint::SrfNormal,
                Constraint::UTangent,
                Constraint::VTangent,
            ]
            .into_iter()
            .find(|c| option_name_eq(value, c.name()))
            .ok_or(CommandError::Usage(USAGE))?;
        } else {
            return Ok(false);
        }
        next.validate()?;
        *self = next;
        Ok(true)
    }
    fn validate(self) -> Result<(), CommandError> {
        if self.distances.iter().any(|d| !d.is_finite() || *d <= 0.) {
            Err(CommandError::Usage(USAGE))
        } else {
            Ok(())
        }
    }
    pub fn arguments(self) -> String {
        format!(
            "U_Distance={} V_Distance={} FixEdges={} Copy={} DirConstraint={}",
            self.distances[0],
            self.distances[1],
            if self.fix_edges { "Yes" } else { "No" },
            if self.copy { "Yes" } else { "No" },
            self.constraint.name()
        )
    }
}
#[derive(Default)]
pub(super) struct SoftEditSurfaceCommand(Remembered<Options>);
impl Command for SoftEditSurfaceCommand {
    fn name(&self) -> &'static str {
        "SoftEditSrf"
    }
    fn copy_option_default(&self) -> Option<bool> {
        Some(false)
    }
    fn soft_edit_surface_options(&self) -> Option<Options> {
        Some(self.0.get())
    }
    fn remember_soft_edit_surface_options(&self, value: Options) -> bool {
        if value.validate().is_err() {
            false
        } else {
            self.0.set(value);
            true
        }
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_selected(doc, args, false)
    }
    fn run_postselected(
        &self,
        doc: &mut Document,
        args: &[&str],
        _: CommandContext,
    ) -> Result<String, CommandError> {
        self.run_selected(doc, args, true)
    }
}
impl SoftEditSurfaceCommand {
    fn run_selected(
        &self,
        doc: &mut Document,
        args: &[&str],
        post: bool,
    ) -> Result<String, CommandError> {
        let ids = doc
            .selected_objects()
            .filter(|o| {
                doc.is_object_selectable(o.id())
                    && surface_reference::surface(o.geometry(), None).is_some()
            })
            .map(|o| o.id())
            .collect::<Vec<_>>();
        let [id] = ids.as_slice() else {
            return Err(CommandError::Usage(USAGE));
        };
        let mut draft = Draft::new(doc, *id)?;
        let mut options = self.0.get();
        let mut points = Vec::new();
        let mut i = 0;
        while i < args.len() {
            if options.accept(args[i])? {
                i += 1;
            } else {
                let (p, n) = parse_point(&args[i..])?;
                points.push(p);
                i += n;
            }
        }
        draft.set_options(options)?;
        for p in points {
            draft.point(p)?;
        }
        if draft.anchor.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        let result = draft.commit(doc, post)?;
        self.0.set(draft.options());
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct Draft {
    state: crate::soft_edit_state::SoftEditState,
    surface: NurbsSurface,
    options: Options,
    anchor: Option<Point3>,
    raw_parameter: Option<[Real; 2]>,
    parameter: Option<[Real; 2]>,
    prepared: Option<SurfaceSoftEdit>,
}
impl std::ops::Deref for Draft {
    type Target = crate::refinement::Draft;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
impl Draft {
    pub fn new(doc: &Document, id: ObjectId) -> Result<Self, CommandError> {
        let state = crate::refinement::Draft::new(doc, id, None, "SoftEditSrf")?;
        let surface = surface_reference::surface(&state.result, None)
            .ok_or(CommandError::Usage(USAGE))?
            .0
            .clone();
        if surface.is_closed_v()? {
            return Err(CommandError::Usage(
                "SoftEditSrf currently requires an open V direction",
            ));
        }
        Ok(Self {
            state: crate::soft_edit_state::SoftEditState::new(state),
            surface,
            options: Options::default(),
            anchor: None,
            raw_parameter: None,
            parameter: None,
            prepared: None,
        })
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn anchor(&self) -> Option<Point3> {
        self.anchor
    }
    pub fn cancel_offset(&mut self) {
        self.anchor = None;
        self.raw_parameter = None;
        self.parameter = None;
        self.prepared = None;
    }
    pub fn set_options(&mut self, options: Options) -> Result<(), CommandError> {
        options.validate()?;
        let prepared = self
            .parameter
            .map(|uv| {
                self.surface.prepare_soft_edit(
                    uv,
                    options.distances,
                    options.fix_edges,
                    self.state.tolerance,
                )
            })
            .transpose()?;
        self.options = options;
        self.prepared = prepared;
        Ok(())
    }
    pub fn marker_points(&self) -> Result<Vec<Point3>, CommandError> {
        let p = self.surface.soft_edit_parameters()?;
        let mut points = Vec::new();
        for v in p[1].iter() {
            for u in &p[0] {
                points.push(self.surface.evaluate(*u, *v)?);
            }
        }
        Ok(points)
    }
    pub fn source_surface(&self) -> &NurbsSurface {
        &self.surface
    }
    pub fn constraint_axis(&self) -> Result<Option<Vector3>, CommandError> {
        let Some([u, v]) = self.raw_parameter else {
            return Ok(None);
        };
        Ok(match self.options.constraint {
            Constraint::None => None,
            Constraint::SrfNormal => Some(self.surface.normal_at(u, v)?.as_vector()),
            Constraint::UTangent => Some(
                self.surface
                    .evaluate_with_derivatives(u, v)?
                    .1
                    .normalized_nonzero()?
                    .as_vector(),
            ),
            Constraint::VTangent => Some(
                self.surface
                    .evaluate_with_derivatives(u, v)?
                    .2
                    .normalized_nonzero()?
                    .as_vector(),
            ),
        })
    }
    fn offset(&self, point: Point3) -> Result<Vector3, CommandError> {
        let mut delta = self
            .anchor
            .ok_or(CommandError::Usage(USAGE))?
            .vector_to(point)?;
        if let Some(axis) = self.constraint_axis()? {
            delta = axis.scaled(axis.dot(delta)?)?;
        }
        Ok(delta)
    }
    fn edited_geometry(&self, surface: NurbsSurface) -> Result<Geometry, CommandError> {
        Ok(match self.state.working_geometry() {
            Geometry::NurbsSurface(_) => Geometry::NurbsSurface(surface),
            Geometry::Brep(b) => {
                Geometry::Brep(b.try_with_edited_single_surface(surface, self.state.tolerance)?)
            }
            _ => return Err(CommandError::Usage(USAGE)),
        })
    }
    pub fn point(&mut self, point: Point3) -> Result<(), CommandError> {
        if let Some(anchor) = self.anchor {
            let delta = self.offset(point)?;
            if viboceros_geometry::curve_points_coincident(anchor, anchor.translated(delta)?) {
                return Err(CommandError::Usage(
                    "Offset point must differ from the base point",
                ));
            }
            let surface = self.prepared.as_ref().unwrap().apply(delta)?;
            let result = self.edited_geometry(surface.clone())?;
            if !self.options.copy {
                self.surface = surface;
            }
            self.state.accept(result, self.options.copy);
            self.cancel_offset();
        } else {
            let raw = surface_reference::closest(
                self.state.working_geometry(),
                None,
                point,
                false,
                self.state.tolerance,
            )?
            .ok_or(CommandError::Usage(USAGE))?
            .uv;
            let stations = self.surface.soft_edit_parameters()?;
            let uv = std::array::from_fn(|axis| {
                *stations[axis]
                    .iter()
                    .min_by(|a, b| (**a - raw[axis]).abs().total_cmp(&(**b - raw[axis]).abs()))
                    .unwrap()
            });
            let prepared = self.surface.prepare_soft_edit(
                uv,
                self.options.distances,
                self.options.fix_edges,
                self.state.tolerance,
            )?;
            self.anchor = Some(prepared.anchor()?);
            self.parameter = Some(uv);
            self.raw_parameter = Some(raw);
            self.prepared = Some(prepared);
        }
        Ok(())
    }
    pub fn preview(&self, point: Point3) -> Result<Option<Geometry>, CommandError> {
        if self.anchor.is_none() {
            return Ok(None);
        }
        Ok(Some(self.edited_geometry(
            self.prepared.as_ref().unwrap().apply(self.offset(point)?)?,
        )?))
    }
    pub fn scene(&self, doc: &Document, hover: Option<Point3>) -> Result<Document, CommandError> {
        let hover = hover.map(|p| self.preview(p)).transpose()?.flatten();
        self.state.scene(doc, hover, self.options.copy)
    }
    pub fn commit(&self, doc: &mut Document, post: bool) -> Result<String, CommandError> {
        let retain = matches!(&*self.state.source,Geometry::Brep(b)if !b.faces()[0].is_untrimmed(self.state.tolerance)?);
        self.state.commit(doc, post, retain)
    }
}
