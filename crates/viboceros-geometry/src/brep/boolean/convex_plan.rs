//! Reusable exact convex intersections without rounded intermediate operands.
use super::*;
use std::sync::Arc;

/// An exact finite convex region in one original-input plan. Regions from
/// different plans cannot be combined. Supporting faces borrow original inputs.
#[derive(Clone)]
pub struct BrepConvexRegion<'a> {
    identity: Arc<()>,
    polygons: Vec<Polygon<'a>>,
}

/// Exact convex input certification, containment, boundary interaction and
/// intersection, sharing a cumulative work budget. Only export rounds model
/// and UV coordinates. Inputs and their supporting surfaces remain unchanged.
pub struct BrepConvexBooleanPlan<'a> {
    originals: Vec<&'a Brep>,
    operands: Vec<Vec<Polygon<'a>>>,
    identity: Arc<()>,
    budget: Budget,
    tolerance: Tolerance,
    exported_faces: usize,
}

impl<'a> BrepConvexBooleanPlan<'a> {
    pub fn try_new(breps: &[&'a Brep], tolerance: Tolerance) -> Result<Self, GeometryError> {
        if breps.len() > 128 {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        let mut budget = Budget(EXACT_WORK_LIMIT);
        let operands = breps
            .iter()
            .map(|b| extract(b, &mut budget))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            originals: breps.to_vec(),
            operands,
            identity: Arc::new(()),
            budget,
            tolerance,
            exported_faces: 0,
        })
    }

    pub fn input(&mut self, index: usize) -> Result<BrepConvexRegion<'a>, GeometryError> {
        let polygons =
            self.operands
                .get(index)
                .ok_or(GeometryError::UnsupportedConvexBrepBoolean {
                    context: "plan input index out of range",
                })?;
        self.budget.spend(polygons.len())?;
        Ok(BrepConvexRegion {
            identity: self.identity.clone(),
            polygons: polygons.clone(),
        })
    }

    fn check(&self, region: &BrepConvexRegion<'_>) -> Result<(), GeometryError> {
        if !Arc::ptr_eq(&self.identity, &region.identity) {
            return Err(GeometryError::UnsupportedConvexBrepBoolean {
                context: "regions must belong to the same convex plan",
            });
        }
        Ok(())
    }

    pub fn is_empty(&mut self, region: &BrepConvexRegion<'_>) -> Result<bool, GeometryError> {
        self.check(region)?;
        self.budget.spend(1)?;
        Ok(region.polygons.is_empty())
    }

    /// Complete finite volume containment, independent of original winding.
    /// Empty regions are covered by every region; a nonempty region is never
    /// covered by an empty region.
    pub fn covered_by(
        &mut self,
        inner: &BrepConvexRegion<'_>,
        outer: &BrepConvexRegion<'_>,
    ) -> Result<bool, GeometryError> {
        self.check(inner)?;
        self.check(outer)?;
        self.budget.spend(1)?;
        if inner.polygons.is_empty() {
            return Ok(true);
        }
        if outer.polygons.is_empty() {
            return Ok(false);
        }
        intersection::contained(&inner.polygons, &outer.polygons, &mut self.budget)
    }

    /// Proper face crossings, partial coplanar overlap and opposing area
    /// contact. Equality, strict nesting and point/edge-only contacts are false.
    pub fn boundary_interacts(
        &mut self,
        a: &BrepConvexRegion<'_>,
        b: &BrepConvexRegion<'_>,
    ) -> Result<bool, GeometryError> {
        self.check(a)?;
        self.check(b)?;
        Ok(
            union::face_interaction(&a.polygons, &b.polygons, &mut self.budget)?
                || union::face_interaction(&b.polygons, &a.polygons, &mut self.budget)?,
        )
    }

    pub fn intersect(
        &mut self,
        a: &BrepConvexRegion<'a>,
        b: &BrepConvexRegion<'a>,
    ) -> Result<BrepConvexRegion<'a>, GeometryError> {
        self.check(a)?;
        self.check(b)?;
        let polygons = if a.polygons.is_empty() || b.polygons.is_empty() {
            self.budget.spend(1)?;
            Vec::new()
        } else {
            intersection::common_polygons(&[&a.polygons, &b.polygons], &mut self.budget)?
        };
        Ok(BrepConvexRegion {
            identity: self.identity.clone(),
            polygons,
        })
    }

    /// Validate the rounded material shell, retaining truthful original input
    /// and face ownership. Total exported patches are limited to 4,096.
    pub fn export(
        &mut self,
        region: &BrepConvexRegion<'a>,
    ) -> Result<Option<BrepConvexIntersection>, GeometryError> {
        self.check(region)?;
        self.budget.spend(region.polygons.len())?;
        self.exported_faces += region.polygons.len();
        if self.exported_faces > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        if region.polygons.is_empty() {
            return Ok(None);
        }
        let face_sources = source_faces(&self.originals, &region.polygons, &mut self.budget)?;
        let brep = rebuild(region.polygons.clone(), self.tolerance, &mut self.budget)?;
        if brep.edge_connected_face_components().len() != 1 {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        Ok(Some(BrepConvexIntersection { brep, face_sources }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{cube, tetra};
    use super::*;

    #[test]
    fn original_relations_and_empty_regions_are_exact_and_foreign_plans_rejected() {
        let bounds = [
            [[0., 2.]; 3],
            [[1., 3.]; 3],
            [[-1., 4.]; 3],
            [[1.2, 1.8]; 3],
            [[0., 2.]; 3],
            [[1., 2.], [0.25, 0.75], [0.25, 0.75]],
        ];
        let inputs = bounds.map(cube);
        let before = inputs.clone();
        let mut plan =
            BrepConvexBooleanPlan::try_new(&inputs.iter().collect::<Vec<_>>(), Tolerance::DEFAULT)
                .unwrap();
        let regions = (0..inputs.len())
            .map(|i| plan.input(i).unwrap())
            .collect::<Vec<_>>();
        let mut interactions = Vec::new();
        for (a, inner) in bounds.iter().enumerate() {
            for (b, outer) in bounds.iter().enumerate() {
                assert_eq!(
                    plan.covered_by(&regions[a], &regions[b]).unwrap(),
                    (0..3).all(|i| inner[i][0] >= outer[i][0] && inner[i][1] <= outer[i][1])
                );
                if a < b && plan.boundary_interacts(&regions[a], &regions[b]).unwrap() {
                    interactions.push([a, b]);
                }
            }
        }
        assert_eq!(interactions, [[0, 1], [0, 5], [1, 4], [4, 5]]);
        let empty = plan.intersect(&regions[1], &regions[5]).unwrap();
        assert!(plan.is_empty(&empty).unwrap());
        assert!(plan.covered_by(&empty, &regions[0]).unwrap());
        assert!(!plan.covered_by(&regions[0], &empty).unwrap());
        assert!(
            plan.intersect(&regions[0], &empty)
                .unwrap()
                .polygons
                .is_empty()
        );
        assert!(plan.export(&empty).unwrap().is_none());
        let mut foreign =
            BrepConvexBooleanPlan::try_new(&[&inputs[0]], Tolerance::DEFAULT).unwrap();
        let other = foreign.input(0).unwrap();
        assert!(plan.covered_by(&regions[0], &other).is_err());
        assert!(plan.intersect(&regions[0], &other).is_err());
        assert!(plan.export(&other).is_err());
        assert!(plan.input(inputs.len()).is_err());
        plan.exported_faces = MAX_OUTPUT_FACES;
        assert!(matches!(
            plan.export(&regions[0]),
            Err(GeometryError::BrepBooleanWorkLimit)
        ));
        plan.budget = Budget(0);
        assert!(matches!(
            plan.input(0),
            Err(GeometryError::BrepBooleanWorkLimit)
        ));
        assert_eq!(inputs, before);
    }

    #[test]
    fn staged_rational_cuts_round_only_at_export_and_keep_original_surfaces() {
        let stretch = AffineTransform3::try_new(
            [[3., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            Vector3::try_new(0., 0., 0.).unwrap(),
        )
        .unwrap();
        let a = tetra([0.; 3])
            .transformed(stretch, Tolerance::DEFAULT)
            .unwrap();
        let b = cube([[0.5, 10.], [0., 4.], [0., 4.]]);
        let c = cube([[1., 10.], [0., 4.], [0., 4.]]);
        let originals = [&a, &b, &c];
        let mut plan = BrepConvexBooleanPlan::try_new(&originals, Tolerance::DEFAULT).unwrap();
        let a = plan.input(0).unwrap();
        let b = plan.input(1).unwrap();
        let c = plan.input(2).unwrap();
        let first = plan.intersect(&a, &b).unwrap();
        let initial = plan.export(&first).unwrap().unwrap();
        assert!(
            (initial.brep.signed_volume(Tolerance::DEFAULT).unwrap() - 4913. / 432.).abs() < 1e-10
        );
        assert!(
            initial
                .brep
                .vertices()
                .iter()
                .any(|v| (v.point().y() - 17. / 6.).abs() < 1e-14)
        );
        let second = plan.intersect(&first, &c).unwrap();
        let result = plan.export(&second).unwrap().unwrap();
        assert!(
            (result.brep.signed_volume(Tolerance::DEFAULT).unwrap() - 256. / 27.).abs() < 1e-10
        );
        for (face, [owner, index]) in result.brep.faces().iter().zip(&result.face_sources) {
            assert_eq!(face.surface(), originals[*owner].faces()[*index].surface());
        }
    }
}
