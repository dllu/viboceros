//! Numeric simplification settings remain available when their mode is inactive.
use super::*;
use viboceros_command::loft::USAGE;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Values {
    pub(super) rebuild: usize,
    pub(super) refit: Option<f64>,
}

impl Default for Values {
    fn default() -> Self {
        Self {
            rebuild: 10,
            refit: None,
        }
    }
}

impl Values {
    pub(super) fn with_active(self, options: Options) -> Self {
        Self {
            rebuild: if options.rebuild_point_count != 0 {
                options.rebuild_point_count
            } else {
                self.rebuild
            },
            refit: options.refit_tolerance.or(self.refit),
        }
    }

    pub(super) fn update(
        mut self,
        options: Options,
        words: &[&str],
        tolerance: Tolerance,
    ) -> Result<(Options, Self), CommandError> {
        let mut other = Vec::new();
        let mut rebuild_count = None;
        let mut refit_tolerance = None;
        let mut direct_rebuild = false;
        let mut direct_refit = false;
        for &word in words {
            let name = word
                .split_once('=')
                .map_or(word, |(name, _)| name)
                .trim_start_matches('_');
            if name.eq_ignore_ascii_case("RebuildCount")
                || name.eq_ignore_ascii_case("RebuildPointCount")
            {
                let value = Options::parse(&[word])?.rebuild_point_count;
                if rebuild_count.replace(value).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else if name.eq_ignore_ascii_case("RefitTolerance") {
                let value = Options::parse(&[word])?
                    .refit_tolerance
                    .ok_or(CommandError::Usage(USAGE))?;
                if refit_tolerance.replace(value).is_some() {
                    return Err(CommandError::Usage(USAGE));
                }
            } else {
                direct_rebuild |= name.eq_ignore_ascii_case("Rebuild");
                direct_refit |= name.eq_ignore_ascii_case("Refit");
                other.push(word);
            }
        }
        let mut options = options.updated(&other)?;
        if direct_rebuild && rebuild_count.is_some() || direct_refit && refit_tolerance.is_some() {
            return Err(CommandError::Usage(USAGE));
        }
        if let Some(count) = rebuild_count {
            self.rebuild = count;
        }
        if let Some(refit) = refit_tolerance {
            self.refit = Some(refit);
        }
        if direct_rebuild && options.rebuild_point_count != 0 {
            self.rebuild = options.rebuild_point_count;
        }
        if direct_refit {
            self.refit = options.refit_tolerance.or(self.refit);
        }
        if options.rebuild_point_count != 0 {
            options.rebuild_point_count = self.rebuild;
        }
        if options.refit_tolerance.is_some() {
            options.refit_tolerance = Some(self.refit.unwrap_or(tolerance.absolute()));
        }
        Ok((options, self))
    }
}
