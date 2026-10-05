//! Ordered native common-intersection policy over exact original-face regions.
use super::*;
use viboceros_geometry::{
    BrepBooleanOperation, BrepConvexBooleanPlan, BrepConvexRegion, BrepPolyhedralBooleanPlan,
    BrepPolyhedralRegion, BrepSolidOrientation,
};

trait Plan {
    type Region: Clone;
    fn input(&mut self, index: usize) -> Result<Self::Region, GeometryError>;
    fn covered_by(&mut self, a: &Self::Region, b: &Self::Region) -> Result<bool, GeometryError>;
    fn boundary_interacts(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<bool, GeometryError>;
    fn intersect(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<Self::Region, GeometryError>;
    fn is_empty(&mut self, region: &Self::Region) -> Result<bool, GeometryError>;
    fn export(
        &mut self,
        region: &Self::Region,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError>;
}

impl<'a> Plan for BrepConvexBooleanPlan<'a> {
    type Region = BrepConvexRegion<'a>;
    fn input(&mut self, index: usize) -> Result<Self::Region, GeometryError> {
        self.input(index)
    }
    fn covered_by(&mut self, a: &Self::Region, b: &Self::Region) -> Result<bool, GeometryError> {
        self.covered_by(a, b)
    }
    fn boundary_interacts(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<bool, GeometryError> {
        self.boundary_interacts(a, b)
    }
    fn intersect(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<Self::Region, GeometryError> {
        self.intersect(a, b)
    }
    fn is_empty(&mut self, r: &Self::Region) -> Result<bool, GeometryError> {
        self.is_empty(r)
    }
    fn export(
        &mut self,
        r: &Self::Region,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        Ok(self
            .export(r)?
            .into_iter()
            .map(|r| BrepPolyhedralBooleanComponent {
                brep: r.brep,
                face_sources: r.face_sources,
            })
            .collect())
    }
}
impl Plan for BrepPolyhedralBooleanPlan<'_> {
    type Region = BrepPolyhedralRegion;
    fn input(&mut self, index: usize) -> Result<Self::Region, GeometryError> {
        self.input(index)
    }
    fn covered_by(&mut self, a: &Self::Region, b: &Self::Region) -> Result<bool, GeometryError> {
        self.covered_by(a, b)
    }
    fn boundary_interacts(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<bool, GeometryError> {
        self.boundary_interacts(a, b)
    }
    fn intersect(
        &mut self,
        a: &Self::Region,
        b: &Self::Region,
    ) -> Result<Self::Region, GeometryError> {
        self.combine(BrepBooleanOperation::Intersection, &[a, b])
    }
    fn is_empty(&mut self, r: &Self::Region) -> Result<bool, GeometryError> {
        self.is_empty(r)
    }
    fn export(
        &mut self,
        r: &Self::Region,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        Ok(self
            .export_boundary(r)?
            .into_iter()
            .map(|r| BrepPolyhedralBooleanComponent {
                brep: r.brep,
                face_sources: r.face_sources,
            })
            .collect())
    }
}

pub(super) fn intersection(
    breps: &[&Brep],
    tolerance: Tolerance,
) -> Result<Vec<ShellIntersection>, GeometryError> {
    if breps
        .iter()
        .any(|b| b.edge_connected_face_components().len() > 1)
    {
        return compound::common(breps, tolerance);
    }
    match BrepConvexBooleanPlan::try_new(breps, tolerance) {
        Ok(plan) => reduce(plan, breps.len()),
        Err(GeometryError::UnsupportedConvexBrepBoolean { .. }) => reduce(
            BrepPolyhedralBooleanPlan::try_new(breps, tolerance)?,
            breps.len(),
        ),
        Err(error) => Err(error),
    }
}

fn reduce<P: Plan>(mut plan: P, count: usize) -> Result<Vec<ShellIntersection>, GeometryError> {
    let inputs = (0..count)
        .map(|i| plan.input(i))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(first) = inputs.first() else {
        return Ok(Vec::new());
    };
    let mut region = first.clone();
    let mut owners = vec![0];
    let mut interacted = false;
    for next in 1..count {
        let input = &inputs[next];
        let current_inside = plan.covered_by(&region, input)?;
        let input_inside = plan.covered_by(input, &region)?;
        if current_inside && input_inside {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        let crossing = plan.boundary_interacts(&region, input)?;
        if !crossing {
            if input_inside {
                // A smaller, untouched input replaces the intermediate result,
                // retaining its own attributes and geometry user text.
                region = input.clone();
                owners = vec![next];
            } else if !current_inside {
                region = plan.intersect(&region, input)?;
                if plan.is_empty(&region)? {
                    return Ok(Vec::new());
                }
            }
            // Strict enclosing inputs leave the current result and owners alone.
            continue;
        }
        let mut retained = Vec::new();
        for owner in owners {
            let keep = if input_inside {
                plan.boundary_interacts(&inputs[owner], input)?
            } else {
                !plan.covered_by(input, &inputs[owner])?
            };
            if keep {
                retained.push(owner);
            }
        }
        retained.push(next);
        owners = retained;
        region = plan.intersect(&region, input)?;
        if plan.is_empty(&region)? {
            return Ok(Vec::new());
        }
        interacted = true;
    }
    if !interacted {
        return Ok(Vec::new());
    }
    let mut results = plan.export(&region)?;
    let geometry_owner = if results.len() == 1 {
        owners.last().copied()
    } else {
        None
    };
    let owner = *owners
        .first()
        .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
    for r in &mut results {
        if r.brep.solid_orientation()? == BrepSolidOrientation::Inward {
            r.brep = r.brep.reversed();
        }
    }
    Ok(results
        .into_iter()
        .map(|component| ShellIntersection {
            component,
            owner,
            geometry_owner,
        })
        .collect())
}
