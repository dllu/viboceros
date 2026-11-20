//! Selection, direction options, and live seam placement on a closed surface.
use super::*;
use crate::viewport::surface_seam::{Contact, Overlay, Preview, seam_curves};
use viboceros_command::ObjectSelectionFilter;
use viboceros_command::surface_seam::{Source, parse_direction};
use viboceros_drafting::PointInput;
use viboceros_geometry::{NurbsCurve, Real, SurfaceKnotDirection as Direction};

#[derive(Clone, Copy, PartialEq)]
enum Getter {
    ObjectId,
    Direction,
}

struct Adjustment {
    source: Source,
    mode: Direction,
    original: [Option<NurbsCurve>; 2],
    preview: Option<Preview>,
}

impl Adjustment {
    fn new(source: Source, preferred: Direction) -> Self {
        let mode = match source.closed_axes() {
            [true, true] => preferred,
            [true, false] => Direction::V,
            _ => Direction::U,
        };
        let surface = source.surface();
        let uv = [*surface.domain_u().start(), *surface.domain_v().start()];
        let original = seam_curves(surface, mode, uv);
        Self {
            source,
            mode,
            original,
            preview: None,
        }
    }

    fn can_change_direction(&self) -> bool {
        self.source.closed_axes() == [true, true]
    }

    // The interactive option names the varying coordinate of the seam curve.
    fn fixed_axis(&self) -> Direction {
        match self.mode {
            Direction::U => Direction::V,
            Direction::V => Direction::U,
            Direction::Both => Direction::Both,
        }
    }

    fn change_mode(&mut self, mode: Direction) {
        self.mode = mode;
        let surface = self.source.surface();
        let uv = [*surface.domain_u().start(), *surface.domain_v().start()];
        self.original = seam_curves(surface, mode, uv);
        if let Some(preview) = &self.preview {
            self.preview = Preview::at(surface, mode, preview.uv);
        }
    }
}

pub(super) struct Session {
    before: Vec<ObjectId>,
    preferred: Direction,
    getter: Option<Getter>,
    adjustment: Option<Adjustment>,
}

impl Session {
    pub(super) fn selecting(&self) -> bool {
        self.adjustment.is_none()
    }

    pub(super) fn hint(&self) -> &'static str {
        if self.getter == Some(Getter::ObjectId) {
            return "Enter the closed surface object ID";
        }
        let Some(adjustment) = &self.adjustment else {
            return "Select a closed surface for seam adjustment";
        };
        if self.getter == Some(Getter::Direction) {
            return match adjustment.mode {
                Direction::U => "Direction <U> (U V Both)",
                Direction::V => "Direction <V> (U V Both)",
                Direction::Both => "Direction <Both> (U V Both)",
            };
        }
        if adjustment.can_change_direction() {
            match adjustment.mode {
                Direction::U => "Adjust surface seam (Direction=U Toggle)",
                Direction::V => "Adjust surface seam (Direction=V Toggle)",
                Direction::Both => "Adjust surface seam (Direction=Both)",
            }
        } else {
            "Adjust surface seam"
        }
    }

    pub(super) fn overlay(&self) -> Option<Overlay<'_>> {
        let a = self.adjustment.as_ref()?;
        Some(Overlay {
            source: &a.source,
            original: &a.original,
            preview: a.preview.as_ref(),
            picking: self.getter.is_none(),
        })
    }
}

impl VibocerosApp {
    pub(super) fn start_surface_seam(&mut self, input: &str) -> bool {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|word| {
            word.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("SrfSeam")
        }) {
            return false;
        }
        let options = words.collect::<Vec<_>>();
        let (preferred, getter) = match options.as_slice() {
            [] => (self.surface_seam_mode, None),
            [name]
                if name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("Direction") =>
            {
                (self.surface_seam_mode, Some(Getter::Direction))
            }
            options => {
                let Some(mode) = seam_direction_options(options) else {
                    return false;
                };
                (mode, None)
            }
        };
        let before = self.document.selected_object_ids().collect::<Vec<_>>();
        let sources = self
            .document
            .selected_objects()
            .filter(|object| ObjectSelectionFilter::ClosedSurfaces.accepts_object(object))
            .map(|object| object.id())
            .take(2)
            .collect::<Vec<_>>();
        self.cancel_interactive_command(false);
        self.restore_curve_selection(Vec::new());
        self.surface_seam_session = Some(Session {
            before,
            preferred,
            getter,
            adjustment: None,
        });
        self.push_log(format!("> {input}"));
        if let [id] = sources[..] {
            self.pick_surface_seam_sources([id]);
        } else {
            self.log_surface_seam();
        }
        true
    }

    pub(super) fn pick_surface_seam_sources(
        &mut self,
        ids: impl IntoIterator<Item = ObjectId>,
    ) -> bool {
        if self.plane_prompt.is_some() {
            return false;
        }
        let Some(session) = &mut self.surface_seam_session else {
            return false;
        };
        if !session.selecting() {
            return true;
        }
        let mut ids = ids.into_iter();
        let Some(id) = ids.next() else { return true };
        if ids.next().is_some() {
            self.push_log("Select one closed surface".into());
            return true;
        }
        match Source::capture(&self.document, id) {
            Ok(source) => {
                let adjustment = Adjustment::new(source, session.preferred);
                if !adjustment.can_change_direction() || session.getter != Some(Getter::Direction) {
                    session.getter = None;
                }
                self.surface_seam_mode = if adjustment.can_change_direction() {
                    adjustment.mode
                } else {
                    self.surface_seam_mode
                };
                session.adjustment = Some(adjustment);
                self.restore_curve_selection(vec![id]);
                self.log_surface_seam();
            }
            Err(error) => self.push_log(format!("Error: {error}")),
        }
        true
    }

    fn log_surface_seam(&mut self) {
        if let Some(session) = &self.surface_seam_session {
            self.push_log(session.hint().into());
        }
    }

    pub(super) fn validate_surface_seam(&mut self) {
        if self
            .surface_seam_session
            .as_ref()
            .and_then(|s| s.adjustment.as_ref())
            .is_some_and(|a| !a.source.current(&self.document))
        {
            self.cancel_surface_seam(false);
            self.push_log("The SrfSeam source or document tolerance changed".into());
        }
    }

    pub(super) fn cancel_surface_seam(&mut self, announce: bool) {
        if let Some(session) = self.surface_seam_session.take() {
            self.restore_curve_selection(session.before);
            if announce {
                self.push_log("Cancelled SrfSeam".into());
            }
        }
    }

    pub(super) fn update_surface_seam_contact(&mut self, contact: Option<Contact>) -> bool {
        let Some(adjustment) = self
            .surface_seam_session
            .as_mut()
            .and_then(|s| s.adjustment.as_mut())
        else {
            return false;
        };
        if let Some(contact) = contact.filter(|contact| contact.accept) {
            self.finish_surface_seam(contact.uv);
            return true;
        }
        let uv = contact.map(|contact| contact.uv);
        if adjustment.preview.as_ref().map(|preview| preview.uv) == uv {
            return false;
        }
        adjustment.preview =
            uv.and_then(|uv| Preview::at(adjustment.source.surface(), adjustment.mode, uv));
        true
    }

    fn finish_surface_seam(&mut self, uv: [Real; 2]) {
        let adjustment = self
            .surface_seam_session
            .as_ref()
            .and_then(|s| s.adjustment.as_ref())
            .expect("surface seam adjustment");
        let id = adjustment.source.id();
        let result = adjustment
            .source
            .prepare_interactive(adjustment.fixed_axis(), uv);
        let geometry = match result {
            Ok(geometry) => geometry,
            Err(error) => {
                self.push_log(format!("Unable to adjust the surface seam: {error}"));
                return;
            }
        };
        self.cancel_surface_seam(false);
        let result = geometry.map(|geometry| {
            edit_document_transaction(&mut self.document, "SrfSeam", |document| {
                document.replace_object_geometries([(id, geometry)])?;
                Ok(())
            })
        });
        self.push_log(match result {
            Some(Ok(())) => "Adjusted surface seam".into(),
            Some(Err(error)) => format!("Unable to adjust the surface seam: {error}"),
            None => "Surface seam is unchanged".into(),
        });
    }

    pub(super) fn continue_surface_seam(&mut self, input: &str) -> bool {
        if self.surface_seam_session.is_none() || self.plane_prompt.is_some() {
            return false;
        }
        self.command_input.clear();
        self.validate_surface_seam();
        let Some(session) = &mut self.surface_seam_session else {
            return true;
        };
        let input = input.trim().trim_start_matches('_');
        let (name, value) = input
            .split_once(|c: char| c == '=' || c.is_whitespace())
            .map_or((input, ""), |(name, value)| (name, value.trim()));
        if input == "!" || input.eq_ignore_ascii_case("Cancel") {
            self.cancel_surface_seam(true);
        } else if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            if session.getter.take().is_some() {
                self.log_surface_seam();
            } else {
                self.cancel_surface_seam(false);
            }
        } else if session.selecting() {
            if name.eq_ignore_ascii_case("SelID") || session.getter == Some(Getter::ObjectId) {
                let id = if name.eq_ignore_ascii_case("SelID") {
                    value
                } else {
                    input
                };
                if id.is_empty() {
                    session.getter = Some(Getter::ObjectId);
                    self.log_surface_seam();
                } else if let Ok(id) = id.parse::<ObjectId>() {
                    self.pick_surface_seam_sources([id]);
                } else {
                    self.push_log("Enter an existing closed surface object ID".into());
                }
            } else {
                self.log_surface_seam();
            }
        } else {
            self.surface_seam_option(input, name, value);
        }
        true
    }

    fn surface_seam_option(&mut self, input: &str, name: &str, value: &str) {
        let session = self.surface_seam_session.as_mut().expect("SrfSeam session");
        let adjustment = session
            .adjustment
            .as_mut()
            .expect("surface seam adjustment");
        let direction_option = name.eq_ignore_ascii_case("Direction");
        let toggle = input.eq_ignore_ascii_case("Toggle");
        if adjustment.can_change_direction()
            && (direction_option
                || toggle
                || session.getter == Some(Getter::Direction)
                || parse_direction(input).is_some())
        {
            if direction_option && value.is_empty() {
                session.getter = Some(Getter::Direction);
            } else {
                let mode = if toggle {
                    match adjustment.mode {
                        Direction::U => Some(Direction::V),
                        Direction::V => Some(Direction::U),
                        Direction::Both => None,
                    }
                } else {
                    parse_direction(if direction_option { value } else { input })
                };
                if let Some(mode) = mode {
                    adjustment.change_mode(mode);
                    self.surface_seam_mode = mode;
                    session.getter = None;
                }
            }
            self.log_surface_seam();
            return;
        }

        let surface = adjustment.source.surface();
        let uv = if name.eq_ignore_ascii_case("Parameter") {
            parse_seam_parameters(value, adjustment.fixed_axis(), surface)
        } else {
            PointInput::parse_with_units(input, self.document.units())
                .and_then(Result::ok)
                .and_then(|point| {
                    point
                        .resolve(
                            self.viewports[self.active_viewport].construction_plane(),
                            self.last_point,
                        )
                        .ok()
                })
                .and_then(|point| {
                    surface
                        .closest_parameters(point, self.document.tolerance())
                        .ok()
                })
                .map(|(u, v)| [u, v])
        };
        if let Some(uv) = uv {
            self.finish_surface_seam(uv);
        } else {
            self.log_surface_seam();
        }
    }
}

fn seam_direction_options(words: &[&str]) -> Option<Direction> {
    match *words {
        [word] => {
            if let Some((name, value)) = word.split_once('=')
                && name
                    .trim_start_matches('_')
                    .eq_ignore_ascii_case("Direction")
            {
                parse_direction(value)
            } else {
                parse_direction(word)
            }
        }
        [name, value]
            if name
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Direction") =>
        {
            parse_direction(value)
        }
        _ => None,
    }
}

fn parse_seam_parameters(
    value: &str,
    direction: Direction,
    surface: &viboceros_geometry::NurbsSurface,
) -> Option<[Real; 2]> {
    let parse = |value: &str| {
        value
            .trim()
            .parse::<Real>()
            .ok()
            .filter(|value| value.is_finite())
    };
    if let Some((u, v)) = value.split_once(',') {
        return Some([parse(u)?, parse(v)?]);
    }
    let value = parse(value)?;
    match direction {
        Direction::U => Some([value, *surface.domain_v().start()]),
        Direction::V => Some([*surface.domain_u().start(), value]),
        Direction::Both => None,
    }
}
