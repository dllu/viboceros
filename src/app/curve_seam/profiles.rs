//! Shared command-line editing for sweep profile seam markers.
use super::*;
use viboceros_drafting::PointInput;

impl Editor {
    pub(in crate::app) fn edit_profile(
        &mut self,
        input: &str,
        id_pending: &mut bool,
        viewport: usize,
        point: Option<Point3>,
        tolerance: Tolerance,
    ) -> Result<bool, &'static str> {
        let (name, value) = input
            .split_once(|c: char| c == '=' || c.is_whitespace())
            .map_or((input, ""), |(name, value)| (name, value.trim()));
        if *id_pending || name.eq_ignore_ascii_case("SelID") {
            let id = if *id_pending { input } else { value };
            *id_pending = id.is_empty();
            if *id_pending {
                return Ok(false);
            }
            return if id
                .parse::<ObjectId>()
                .is_ok_and(|id| self.select_id(viewport, id))
            {
                Ok(true)
            } else {
                Err("Select a closed profile by object ID")
            };
        }
        if let Some(result) = self.option(input, tolerance) {
            return result.map(|()| true);
        }
        let parameter = name
            .eq_ignore_ascii_case("Parameter")
            .then(|| value.parse::<Real>().ok())
            .flatten()
            .filter(|value| value.is_finite());
        if point.is_some_and(|point| self.flip_at_point(viewport, point, tolerance))
            || self.relocate_target(parameter, point, tolerance)
        {
            Ok(true)
        } else {
            Err("Pick a seam marker or use SelID to enter a seam point or parameter")
        }
    }

    pub(in crate::app) fn finish_profile_flip(&mut self) -> bool {
        std::mem::take(&mut self.flipping)
    }
}

impl VibocerosApp {
    pub(in crate::app) fn profile_seam_point(&self, input: &str) -> Option<Point3> {
        PointInput::parse_with_units(input, self.document.units())?
            .ok()?
            .resolve(
                self.viewports[self.active_viewport].construction_plane(),
                self.last_point,
            )
            .ok()
    }
}
