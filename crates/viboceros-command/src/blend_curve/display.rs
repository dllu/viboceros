//! Transient combs share the document graph sampler without editing its sources.
use super::*;
use crate::curvature_graph::{Graph, Options};
use std::sync::{Arc, Mutex};
use viboceros_geometry::{CurvatureComb, CurvatureHair, curvature_comb_with_subdivision};

type Sampling = (u8, u8, bool, bool);
type CachedResult = Result<Arc<CurvatureData>, Arc<CommandError>>;
#[derive(Debug)]
struct Entry {
    sampling: Sampling,
    data: CachedResult,
    scale: Option<(u16, Result<(), Arc<CommandError>>)>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Cache(Arc<Mutex<Option<Entry>>>);

#[derive(Debug)]
pub struct CurvatureData {
    pub sources: Vec<Graph>,
    pub blend: CurvatureComb,
    /// Whole document parents temporarily participating in this analysis.
    pub objects: std::collections::BTreeSet<ObjectId>,
}
#[derive(Clone, Debug)]
pub struct CurvatureDisplay {
    pub options: Options,
    pub data: Arc<CurvatureData>,
}
impl CurvatureDisplay {
    pub fn tip(&self, hair: &CurvatureHair) -> Result<Point3, viboceros_geometry::GeometryError> {
        hair.tip(self.options.factor())
    }
}

impl Input {
    /// A host with existing graph analysis enables the new getter's preview.
    pub fn inherit_curvature_display(&mut self, enabled: bool) {
        self.display_curvature |= enabled;
    }
    pub fn curvature_display_enabled(&self) -> bool {
        self.display_curvature
    }
    /// Display is local to the getter and never enters serialized model commands.
    pub fn set_curvature_display(&mut self, enabled: bool) -> Result<(), CommandError> {
        if !self.ready() {
            return Err(CommandError::Usage(
                "Pick both blend sources before displaying curvature",
            ));
        }
        self.display_curvature = enabled;
        Ok(())
    }
    pub fn curvature_display(
        &self,
        options: &Options,
    ) -> Result<Option<CurvatureDisplay>, Arc<CommandError>> {
        if !self.display_curvature || self.preview.is_none() {
            return Ok(None);
        }
        options.validate().map_err(Arc::new)?;
        let mut cache = self
            .curvature_cache
            .0
            .lock()
            .expect("blend curvature cache");
        let sampling = options.sampling();
        if cache.as_ref().is_none_or(|e| e.sampling != sampling) {
            *cache = Some(Entry {
                sampling,
                data: self
                    .build_curvature(options)
                    .map(Arc::new)
                    .map_err(Arc::new),
                scale: None,
            });
        }
        let entry = cache.as_mut().unwrap();
        let data = entry.data.clone()?;
        if entry
            .scale
            .as_ref()
            .is_none_or(|(scale, _)| *scale != options.scale)
        {
            let result = data
                .sources
                .iter()
                .flat_map(|g| &g.comb.hairs)
                .chain(&data.blend.hairs)
                .try_for_each(|hair| {
                    hair.tip(options.factor())
                        .map(|_| ())
                        .map_err(CommandError::from)
                })
                .map_err(Arc::new);
            entry.scale = Some((options.scale, result));
        }
        entry.scale.as_ref().unwrap().1.clone()?;
        Ok(Some(CurvatureDisplay {
            options: options.clone(),
            data,
        }))
    }
    fn build_curvature(&self, options: &Options) -> Result<CurvatureData, CommandError> {
        let mut remaining = crate::curvature_graph::MAX_SAMPLES;
        let mut sources = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for guide in self.endpoints.iter().filter_map(Endpoint::curve) {
            // A private portion is drawn in its own chart, without enabling
            // analysis on its original parent. Whole sources are deduplicated.
            if !guide.temporary() && !seen.insert(guide.source.object_id()) {
                continue;
            }
            let portion = guide
                .temporary()
                .then(|| Geometry::NurbsCurve(guide.display().clone()));
            let geometry = portion.as_ref().unwrap_or(guide.source.source().0);
            let graphs = crate::curvature_graph::gather(
                geometry,
                guide.source.object_id(),
                options,
                self.tolerance,
            )?;
            for graph in graphs {
                remaining = remaining
                    .checked_sub(graph.comb.hairs.len() + graph.comb.singular_samples)
                    .ok_or(viboceros_geometry::GeometryError::CurvatureCombLimit)?;
                sources.push(graph);
            }
        }
        let blend = curvature_comb_with_subdivision(
            self.preview.as_ref().unwrap(),
            options.density,
            remaining,
            &mut crate::curve_segments::trim,
        )?;
        Ok(CurvatureData {
            sources,
            blend,
            objects: seen,
        })
    }
}
