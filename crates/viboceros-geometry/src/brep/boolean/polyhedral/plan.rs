//! Reusable exact regions; intermediate shells never become rounded operands.
use super::*;
use std::sync::Arc;
type Boundary<'a> = (Vec<Polygon<'a>>, Vec<[usize; 2]>);

/// An opaque bounded region in one certified original-face arrangement.
/// Regions from different plans cannot be mixed. Cloning retains exact membership.
#[derive(Clone, Debug)]
pub struct BrepPolyhedralRegion {
    plan: Arc<()>,
    mask: Vec<bool>,
}

/// One connected boundary shell before output rounding. Its region denotes the
/// finite enclosed volume independently of the shell's original orientation.
#[derive(Clone, Debug)]
pub struct BrepPolyhedralShell {
    pub region: BrepPolyhedralRegion,
    pub inward: bool,
    /// Original operand/face indices of the exact unmerged boundary patches.
    pub face_sources: Vec<[usize; 2]>,
}

/// Reusable polyhedral Boolean expressions over one original-face arrangement.
///
/// Build once, combine bounded regions, inspect shell participation and exact
/// containment, then export. Original supporting surfaces and face ownership
/// survive all intermediate operations. Only export rounds Cartesian/UV values.
/// Input, fragment, rational and cumulative work bounds match the polyhedral
/// API. Total exported boundary patches are additionally limited to 4,096.
/// Native interaction/metadata policies belong in command adapters.
pub struct BrepPolyhedralBooleanPlan<'a> {
    built: arrangement::Arrangement<'a>,
    identity: Arc<()>,
    tolerance: Tolerance,
    budget: Budget,
    exported_faces: usize,
}

impl<'a> BrepPolyhedralBooleanPlan<'a> {
    pub fn try_new(breps: &[&'a Brep], tolerance: Tolerance) -> Result<Self, GeometryError> {
        let mut budget = Budget(EXACT_WORK_LIMIT);
        let built = arrangement::build(breps, tolerance, &mut budget)?;
        Ok(Self {
            built,
            identity: Arc::new(()),
            tolerance,
            budget,
            exported_faces: 0,
        })
    }

    pub fn input(&mut self, index: usize) -> Result<BrepPolyhedralRegion, GeometryError> {
        if index >= self.built.operands.len() {
            return Err(unsupported("plan input index out of range"));
        }
        self.budget.spend(self.built.samples.len())?;
        Ok(self.region(self.built.samples.iter().map(|s| s.inside[index]).collect()))
    }

    fn region(&self, mask: Vec<bool>) -> BrepPolyhedralRegion {
        BrepPolyhedralRegion {
            plan: self.identity.clone(),
            mask,
        }
    }
    fn check(&self, region: &BrepPolyhedralRegion) -> Result<(), GeometryError> {
        if !Arc::ptr_eq(&self.identity, &region.plan)
            || region.mask.len() != self.built.samples.len()
        {
            return Err(unsupported(
                "regions must belong to the same polyhedral plan",
            ));
        }
        Ok(())
    }

    pub fn is_empty(&mut self, region: &BrepPolyhedralRegion) -> Result<bool, GeometryError> {
        self.check(region)?;
        self.budget.spend(region.mask.len())?;
        Ok(!region.mask.iter().any(|v| *v))
    }

    pub fn combine(
        &mut self,
        operation: BrepBooleanOperation,
        regions: &[&BrepPolyhedralRegion],
    ) -> Result<BrepPolyhedralRegion, GeometryError> {
        if regions.len() > 128 {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        for r in regions {
            self.check(r)?;
        }
        let mut mask = Vec::with_capacity(self.built.samples.len());
        for i in 0..self.built.samples.len() {
            self.budget.spend(regions.len() + 1)?;
            mask.push(match operation {
                BrepBooleanOperation::Union => regions.iter().any(|r| r.mask[i]),
                BrepBooleanOperation::Intersection => {
                    !regions.is_empty() && regions.iter().all(|r| r.mask[i])
                }
                BrepBooleanOperation::Difference => {
                    !regions.is_empty()
                        && regions[0].mask[i]
                        && !regions[1..].iter().any(|r| r.mask[i])
                }
            });
        }
        Ok(self.region(mask))
    }

    /// Complete exact volume containment, including cavities and disconnected
    /// parts. Both adjacent membership states of every original patch are kept.
    pub fn covered_by(
        &mut self,
        inner: &BrepPolyhedralRegion,
        outer: &BrepPolyhedralRegion,
    ) -> Result<bool, GeometryError> {
        self.check(inner)?;
        self.check(outer)?;
        self.budget.spend(inner.mask.len())?;
        Ok(inner.mask.iter().zip(&outer.mask).all(|(a, b)| !*a || *b))
    }

    /// Whether every positive-area boundary patch has its material-side state
    /// inside another region. This is different from volume containment when
    /// the enclosed volume surrounds an isolated cavity of the other region.
    pub fn boundary_covered_by(
        &mut self,
        inner: &BrepPolyhedralRegion,
        outer: &BrepPolyhedralRegion,
    ) -> Result<bool, GeometryError> {
        self.check(inner)?;
        self.check(outer)?;
        let mut found = false;
        for c in &self.built.cells {
            self.budget.spend(1)?;
            let own = c.sides.map(|i| inner.mask[i]);
            if own[0] != own[1] {
                found = true;
                if !outer.mask[c.sides[usize::from(!own[0])]] {
                    return Ok(false);
                }
            }
        }
        Ok(found)
    }

    /// Proper boundary crossing, partial coplanar overlap, or opposing area
    /// contact. Equality, strict nesting, and edge/point-only contacts are false.
    pub fn boundary_interacts(
        &mut self,
        a: &BrepPolyhedralRegion,
        b: &BrepPolyhedralRegion,
    ) -> Result<bool, GeometryError> {
        self.check(a)?;
        self.check(b)?;
        self.budget.spend(a.mask.len())?;
        if a.mask == b.mask {
            return Ok(false);
        }
        let mut flags = BTreeMap::<(usize, [usize; 2]), [bool; 2]>::new();
        let mut contact = false;
        for c in &self.built.cells {
            self.budget.spend(1)?;
            for (side, own, other) in [(0, a, b), (1, b, a)] {
                let x = c.sides.map(|i| own.mask[i]);
                let y = c.sides.map(|i| other.mask[i]);
                if x[0] == x[1] {
                    continue;
                }
                let flag = flags.entry((side, c.source)).or_default();
                flag[0] |= y[0] || y[1];
                flag[1] |= !y[0] && !y[1];
                contact |= y[0] != y[1] && x[0] != y[0];
            }
        }
        Ok(contact || flags.values().any(|f| f[0] && f[1]))
    }

    /// Every original operand with a positive-area patch on this boundary,
    /// including equivalent patches assigned canonically to another operand.
    pub fn boundary_sources(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<usize>, GeometryError> {
        self.check(region)?;
        let mut result = BTreeSet::new();
        for c in &self.built.cells {
            self.budget.spend(1)?;
            if region.mask[c.sides[0]] != region.mask[c.sides[1]] {
                result.insert(c.source[0]);
            }
        }
        Ok(result.into_iter().collect())
    }

    /// All original faces on this boundary, including equivalent coplanar
    /// patches owned canonically by another operand during export.
    pub fn boundary_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<[usize; 2]>, GeometryError> {
        self.check(region)?;
        let mut result = BTreeSet::new();
        for c in &self.built.cells {
            self.budget.spend(1)?;
            if region.mask[c.sides[0]] != region.mask[c.sides[1]] {
                result.insert(c.source);
            }
        }
        Ok(result.into_iter().collect())
    }

    /// Positive-area common boundary, independent of either orientation.
    pub fn boundaries_overlap(
        &mut self,
        a: &BrepPolyhedralRegion,
        b: &BrepPolyhedralRegion,
    ) -> Result<bool, GeometryError> {
        self.check(a)?;
        self.check(b)?;
        for c in &self.built.cells {
            self.budget.spend(1)?;
            if a.mask[c.sides[0]] != a.mask[c.sides[1]] && b.mask[c.sides[0]] != b.mask[c.sides[1]]
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn boundary(&mut self, region: &BrepPolyhedralRegion) -> Result<Boundary<'a>, GeometryError> {
        self.boundary_from_faces(region, None)
    }

    fn boundary_from_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
        allowed: Option<&BTreeSet<[usize; 2]>>,
    ) -> Result<Boundary<'a>, GeometryError> {
        self.check(region)?;
        let mut polygons = Vec::new();
        let mut sources = Vec::new();
        let mut unique = BTreeSet::new();
        let mut required = BTreeSet::new();
        for c in &self.built.cells {
            self.budget.spend(1)?;
            let x = c.sides.map(|i| region.mask[i]);
            if x[0] == x[1] {
                continue;
            }
            let key = canonical_ring(&c.polygon.ring);
            if allowed.is_some() {
                required.insert(key.clone());
            }
            if allowed.is_some_and(|a| !a.contains(&c.source)) || !unique.insert(key) {
                continue;
            }
            polygons.push(if x[0] {
                c.polygon.clone()
            } else {
                c.polygon.clone().reverse()
            });
            sources.push(c.source);
        }
        if allowed.is_some() && required != unique {
            return Err(unsupported(
                "allowed original faces do not cover the region boundary",
            ));
        }
        Ok((polygons, sources))
    }

    pub fn shells(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralShell>, GeometryError> {
        let (mut polygons, sources) = self.boundary(region)?;
        let groups = exact_shells(&mut polygons, &mut self.budget)?;
        // A bounded region's membership is the XOR of its finite shell
        // enclosures. Classify all but the largest shell, then recover its
        // membership exactly from that identity. A single shell needs no rays.
        let derived = groups
            .iter()
            .enumerate()
            .max_by_key(|(_, faces)| faces.len())
            .map(|(i, _)| i);
        self.budget.spend(region.mask.len())?;
        let mut remaining = region.mask.clone();
        let mut result = Vec::new();
        for (index, faces) in groups.into_iter().enumerate() {
            let mut volume = Rational::zero();
            for &f in &faces {
                let ring = &polygons[f].ring;
                self.budget.spend(ring.len())?;
                for i in 1..ring.len() - 1 {
                    volume += dot(&ring[0], &cross(&ring[i], &ring[i + 1]));
                    check_scalar(&volume)?;
                }
            }
            if volume.is_zero() {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
            let mut mask = Vec::new();
            if Some(index) != derived {
                mask.reserve(self.built.samples.len());
                for (i, s) in self.built.samples.iter().enumerate() {
                    let inside = union::contains(&s.point, &faces, &polygons, &mut self.budget)?;
                    remaining[i] ^= inside;
                    mask.push(inside);
                }
            }
            result.push(BrepPolyhedralShell {
                region: self.region(mask),
                inward: volume.is_negative(),
                face_sources: faces.iter().map(|&f| sources[f]).collect(),
            });
        }
        if let Some(index) = derived {
            result[index].region.mask = remaining;
        }
        Ok(result)
    }

    pub fn export(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        self.export_from_faces(region, None)
    }

    /// Export using only these original supporting faces. Every boundary patch
    /// must be covered by an allowed face; omission is an error, never a hole.
    /// This keeps staged expressions from acquiring coplanar faces belonging
    /// to an operand or shell that did not participate in the expression.
    pub fn export_with_boundary_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
        faces: &[[usize; 2]],
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        if faces.len() > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        self.budget.spend(faces.len())?;
        let allowed = faces.iter().copied().collect();
        self.export_from_faces(region, Some(&allowed))
    }

    fn export_from_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
        allowed: Option<&BTreeSet<[usize; 2]>>,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        let (polygons, sources) = self.boundary_from_faces(region, allowed)?;
        self.exported_faces += polygons.len();
        if self.exported_faces > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        if polygons.is_empty() {
            return Ok(Vec::new());
        }
        let bodies = union::material_components(&polygons, self.tolerance, &mut self.budget)?;
        let mut result = Vec::new();
        for body in bodies {
            embedding::certify_vertex_links(&body.brep, &mut self.budget).map_err(|e| match e {
                GeometryError::UnsupportedPolyhedralBrepBoolean { .. } => {
                    GeometryError::UnrepresentableBrepBoolean
                }
                other => other,
            })?;
            result.push(BrepPolyhedralBooleanComponent {
                brep: body.brep,
                face_sources: body.faces.iter().map(|&f| sources[f]).collect(),
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;

/// Shared exact subdivisions define edge adjacency without model rounding.
fn exact_shells(
    polygons: &mut [Polygon<'_>],
    budget: &mut Budget,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    subdivide(polygons, budget)?;
    let mut edges = BTreeMap::<[ExactPoint; 2], Vec<(usize, bool)>>::new();
    for (face, p) in polygons.iter().enumerate() {
        for i in 0..p.ring.len() {
            budget.spend(1)?;
            let a = &p.ring[i];
            let b = &p.ring[(i + 1) % p.ring.len()];
            let forward = a < b;
            let key = if forward {
                [a.clone(), b.clone()]
            } else {
                [b.clone(), a.clone()]
            };
            edges.entry(key).or_default().push((face, forward));
        }
    }
    let mut adjacent = vec![Vec::new(); polygons.len()];
    for uses in edges.values() {
        if uses.len() != 2 || uses[0].1 == uses[1].1 {
            return Err(GeometryError::UnrepresentableBrepBoolean);
        }
        adjacent[uses[0].0].push(uses[1].0);
        adjacent[uses[1].0].push(uses[0].0);
    }
    let mut seen = vec![false; polygons.len()];
    let mut result = Vec::new();
    for first in 0..polygons.len() {
        if seen[first] {
            continue;
        }
        let mut pending = vec![first];
        seen[first] = true;
        let mut faces = Vec::new();
        while let Some(i) = pending.pop() {
            budget.spend(1)?;
            faces.push(i);
            for &j in &adjacent[i] {
                if !seen[j] {
                    seen[j] = true;
                    pending.push(j);
                }
            }
        }
        faces.sort_unstable();
        result.push(faces);
    }
    Ok(result)
}
