//! Readonly paired fitting and one atomic acceptance for MatchSrf.
use super::*;
use viboceros_document::ReplacementHistory;
#[derive(Clone, Debug)]
pub struct Prepared {
    edges: [Edge; 2],
    result: Brep,
    target_result: Option<Brep>,
}
pub fn prepare(
    doc: &Document,
    edges: [Edge; 2],
    options: Options,
) -> Result<Prepared, CommandError> {
    if edges[0].object() == edges[1].object() || edges.iter().any(|e| !e.current(doc)) {
        return Err(CommandError::Usage(
            "Pick current edges on two different surfaces",
        ));
    }
    let mut policy = options.policy;
    let reversed = edges[0].picked_high()? ^ edges[1].picked_high()?;
    policy.reverse_match = reversed
        ^ matches!(
            edges[1].side,
            SurfaceExtensionEdge::West | SurfaceExtensionEdge::North
        );
    let (surface, target) = if options.average {
        #[cfg(feature = "native-smlib")]
        let operation = viboceros_smlib::average_match_surface_supports;
        #[cfg(not(feature = "native-smlib"))]
        let operation = viboceros_geometry::try_average_match_nurbs_surfaces;
        let values = operation(
            edges[0].surface(),
            edges[0].side,
            edges[1].surface(),
            edges[1].side,
            viboceros_geometry::SurfaceAverageOptions {
                matching: policy,
                reverse_target: reversed
                    ^ matches!(
                        edges[0].side,
                        SurfaceExtensionEdge::West | SurfaceExtensionEdge::North
                    ),
            },
            doc.tolerance(),
        )?;
        let [a, b] = values;
        (a, Some(b))
    } else {
        #[cfg(feature = "native-smlib")]
        let operation = viboceros_smlib::match_surface_supports;
        #[cfg(not(feature = "native-smlib"))]
        let operation = viboceros_geometry::try_match_nurbs_surfaces;
        (
            operation(
                edges[0].surface(),
                edges[0].side,
                edges[1].surface(),
                edges[1].side,
                policy,
                doc.tolerance(),
            )?,
            None,
        )
    };
    let make_brep = |surface: NurbsSurface, reversed| {
        Brep::try_rectangular_surface_face_with_surface_edge_domains(
            surface.clone(),
            surface.domain_u(),
            surface.domain_v(),
            reversed,
            doc.tolerance(),
        )
    };
    let result = make_brep(surface, edges[0].reversed)?;
    let target_result = target
        .map(|s| make_brep(s, edges[1].reversed))
        .transpose()?;
    Ok(Prepared {
        edges,
        result,
        target_result,
    })
}
impl Prepared {
    pub fn geometry(&self) -> &Brep {
        &self.result
    }
    pub fn target_geometry(&self) -> Option<&Brep> {
        self.target_result.as_ref()
    }
    pub fn current(&self, doc: &Document) -> bool {
        self.edges.iter().all(|e| e.current(doc))
    }
    pub fn commit(&self, doc: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(doc, "MatchSrf", |d| self.apply(d))
    }
    pub(super) fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.current(doc) {
            return Err(CommandError::Usage("MatchSrf prepared inputs changed"));
        }
        doc.clear_selection();
        let id = self.edges[0].object();
        let mut replacements = vec![(id, Geometry::Brep(self.result.clone()))];
        if let Some(target) = &self.target_result {
            replacements.push((self.edges[1].object(), Geometry::Brep(target.clone())));
        }
        let order = replacements.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        doc.replace_object_geometries_with_history(
            replacements,
            ReplacementHistory::EveryReplacement,
        )?;
        doc.move_objects_to_end_in_order(order)?;
        Ok(if self.target_result.is_some() {
            "Matched surface edges"
        } else {
            "Matched surface edge"
        }
        .into())
    }
}
