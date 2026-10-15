use super::*;

impl VibocerosApp {
    /// Freeze the actual parallel camera direction, independently of its CPlane.
    pub(super) fn silhouette_input<'a>(
        &self,
        input: &'a str,
    ) -> Result<std::borrow::Cow<'a, str>, &'static str> {
        let mut words = input.split_whitespace();
        if !words.next().is_some_and(|s| {
            s.trim_start_matches(['_', '-'])
                .eq_ignore_ascii_case("Silhouette")
        }) {
            return Ok(std::borrow::Cow::Borrowed(input));
        }
        if words.any(|s| {
            s.split_once('=')
                .is_some_and(|(k, _)| k.trim_start_matches('_').eq_ignore_ascii_case("Direction"))
        }) {
            return Ok(std::borrow::Cow::Borrowed(input));
        }
        let direction = self.viewports[self.active_viewport]
            .parallel_view_direction()
            .ok_or(
                "Silhouette currently requires a parallel viewport or an explicit Direction=x,y,z",
            )?;
        let [x, y, z] = direction.to_array();
        Ok(std::borrow::Cow::Owned(format!(
            "{input} Direction={x},{y},{z}"
        )))
    }
}
