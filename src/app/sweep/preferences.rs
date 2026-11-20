use super::*;
use std::borrow::Cow;

/// An inherited surface frame applies only to a rail with a supporting face.
impl State {
    pub(super) fn effective_options(&self) -> Cow<'_, Options> {
        if self.inherited_surface_style
            && self.options.surface_aligned
            && !self.rail.as_ref().is_some_and(Rail::supports_alignment)
        {
            let mut options = self.options.clone();
            use_freeform(&mut options);
            Cow::Owned(options)
        } else {
            Cow::Borrowed(&self.options)
        }
    }
}

pub(super) fn selects_frame_style(input: &str) -> bool {
    input
        .split_whitespace()
        .filter_map(|argument| argument.split_once('='))
        .any(|(key, _)| {
            let key = key.trim_start_matches(['_', '-']);
            key.eq_ignore_ascii_case("Style") || key.eq_ignore_ascii_case("FrameStyle")
        })
}

fn use_freeform(options: &mut Options) {
    options.roadlike = Some(false);
    options.surface_aligned = false;
    options.axis = None;
}

impl VibocerosApp {
    pub(super) fn initial_sweep_options(
        &self,
        input: &str,
    ) -> Result<(Options, bool), CommandError> {
        let options = self
            .sweep_options
            .updated(&input.split_whitespace().skip(1).collect::<Vec<_>>())?;
        let inherited_surface_style = options.surface_aligned && !selects_frame_style(input);
        Ok((options, inherited_surface_style))
    }

    pub(in crate::app) fn direct_sweep_options(&self, input: &str) -> Option<Options> {
        let name = input
            .split_whitespace()
            .next()?
            .trim_start_matches(['_', '-']);
        if !name.eq_ignore_ascii_case("Sweep1") {
            return None;
        }
        let (mut options, inherited_surface_style) = self.initial_sweep_options(input).ok()?;
        if inherited_surface_style
            && !options.rail_edge.is_some_and(|edge| {
                Rail::capture_reference(&self.document, edge)
                    .is_ok_and(|rail| rail.supports_alignment())
            })
        {
            use_freeform(&mut options);
        }
        Some(options)
    }

    pub(in crate::app) fn remember_sweep_options(&mut self, options: &Options) {
        self.sweep_options = Options {
            roadlike: options.roadlike,
            surface_aligned: options.surface_aligned,
            axis: options.axis,
            global: options.global,
            refit: options.refit,
            untrimmed_miters: options.untrimmed_miters,
            rebuild_point_count: options.rebuild_point_count,
            saved_rebuild_count: options.saved_rebuild_count,
            refit_tolerance: options.refit_tolerance,
            saved_refit_tolerance: options.saved_refit_tolerance,
            ..Default::default()
        };
    }
}
