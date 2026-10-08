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
    // Only original operands can use their certified unsplit face polygons
    // for shell classification. Combined regions retain arrangement cells.
    origin: Option<usize>,
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

/// One connected boundary, including valid nonmanifold edge contacts. It may
/// be inward or non-solid; no material volume is promised for this topology.
#[derive(Clone, Debug)]
pub struct BrepPolyhedralBoundaryComponent {
    pub brep: Brep,
    pub face_sources: Vec<[usize; 2]>,
    /// All equivalent original faces on this connected boundary.
    pub boundary_faces: Vec<[usize; 2]>,
    /// Original inputs with exactly the same complete unoriented boundary.
    pub boundary_equal_inputs: Vec<usize>,
}

/// Finite coplanar partition with original face lineage and shared-area labels.
#[derive(Clone, Debug)]
pub struct BrepCoplanarPartitionComponent {
    pub brep: Brep,
    pub face_sources: Vec<[usize; 2]>,
    /// 1 = first only, 2 = second only, 3 = common to both inputs.
    pub face_categories: Vec<u8>,
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
    use_outer_coverage: bool,
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
            use_outer_coverage: false,
        })
    }

    /// Build regions for closed polyhedra and oriented finite planar sheets.
    /// A sheet represents the negative side of its oriented supporting plane;
    /// only its actual trimmed patches may be exported. Callers must certify
    /// complete physical intersection coverage before accepting a Boolean.
    pub fn try_with_planar_sheets(
        breps: &[&'a Brep],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if breps.is_empty() {
            return Err(unsupported("at least one operand required"));
        }
        let mut budget = Budget(EXACT_WORK_LIMIT);
        let (built, _) = surface_split::build(breps, tolerance, &mut budget)?;
        Ok(Self::from_arrangement(built, tolerance, budget))
    }

    /// Whether two operands are entirely supported on one common plane.
    /// This geometric query does not turn finite sheets into material solids.
    pub fn inputs_are_coplanar(
        &mut self,
        first: usize,
        second: usize,
    ) -> Result<bool, GeometryError> {
        if first >= self.built.operands.len() || second >= self.built.operands.len() {
            return Err(unsupported("plan input index out of range"));
        }
        let reference = &self.built.operands[first][0];
        for polygon in self.built.operands[first]
            .iter()
            .chain(&self.built.operands[second])
        {
            self.budget.spend(polygon.ring.len() + 1)?;
            if !zero(&cross(&reference.normal, &polygon.normal))
                || polygon
                    .ring
                    .iter()
                    .any(|p| !reference.plane_side(p).is_zero())
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Orientation agreement of coplanar sheet inputs.
    pub fn coplanar_input_normals_agree(
        &mut self,
        first: usize,
        second: usize,
    ) -> Result<bool, GeometryError> {
        if !self.inputs_are_coplanar(first, second)? {
            return Err(unsupported("coplanar physical sheets required"));
        }
        Ok(dot(
            &self.built.operands[first][0].normal,
            &self.built.operands[second][0].normal,
        )
        .is_positive())
    }

    /// Positive physical area overlap, excluding edge and point contact.
    pub fn coplanar_inputs_share_area(
        &mut self,
        first: usize,
        second: usize,
    ) -> Result<bool, GeometryError> {
        if !self.inputs_are_coplanar(first, second)? {
            return Err(unsupported("coplanar physical sheets required"));
        }
        let mut first_keys = BTreeSet::new();
        let mut second_keys = BTreeSet::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if !cell.source_covers_cell {
                continue;
            }
            if cell.source[0] == first {
                first_keys.insert(canonical_ring(&cell.polygon.ring));
            }
            if cell.source[0] == second {
                second_keys.insert(canonical_ring(&cell.polygon.ring));
            }
        }
        Ok(first_keys.iter().any(|k| second_keys.contains(k)))
    }

    /// Finite-area Boolean on two coplanar physical sheet boundaries.
    /// Patch coverage, including trim holes, is exact in the shared arrangement.
    /// The first input supplies coincident supporting faces and orientation.
    pub fn export_coplanar_sheet_boolean(
        &mut self,
        operation: BrepBooleanOperation,
        first: usize,
        second: usize,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        if !self.inputs_are_coplanar(first, second)? {
            return Err(unsupported("coplanar physical sheets required"));
        }
        let mut patches =
            BTreeMap::<Vec<ExactPoint>, [Option<(Polygon<'a>, [usize; 2])>; 2]>::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if !cell.source_covers_cell {
                continue;
            }
            let side = if cell.source[0] == first {
                0
            } else if cell.source[0] == second {
                1
            } else {
                continue;
            };
            patches
                .entry(canonical_ring(&cell.polygon.ring))
                .or_insert_with(|| [None, None])[side] = Some((cell.polygon.clone(), cell.source));
        }
        let selected = patches
            .into_values()
            .filter_map(|[a, b]| match operation {
                BrepBooleanOperation::Union => a.or(b),
                BrepBooleanOperation::Intersection => {
                    if b.is_some() {
                        a
                    } else {
                        None
                    }
                }
                BrepBooleanOperation::Difference => {
                    if b.is_none() {
                        a
                    } else {
                        None
                    }
                }
            })
            .collect();
        self.export_open_patches(selected)
    }

    /// Finite-area set Boolean on coplanar sheet inputs in selection order.
    /// The first input's supporting plane/orientation supplies every patch;
    /// no half-space membership or virtual planning face supplies coverage.
    pub fn export_coplanar_sheet_set_boolean(
        &mut self,
        operation: BrepBooleanOperation,
        inputs: &[usize],
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        let Some(&first) = inputs.first() else {
            return Err(unsupported("at least one sheet required"));
        };
        if inputs.len() > 128 {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        for &index in inputs {
            if !self.inputs_are_coplanar(first, index)? {
                return Err(unsupported("coplanar physical sheets required"));
            }
        }
        let mut patches =
            BTreeMap::<Vec<ExactPoint>, (Polygon<'a>, [usize; 2], BTreeSet<usize>)>::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if !cell.source_covers_cell || !inputs.contains(&cell.source[0]) {
                continue;
            }
            let entry = patches
                .entry(canonical_ring(&cell.polygon.ring))
                .or_insert_with(|| (cell.polygon.clone(), cell.source, BTreeSet::new()));
            entry.2.insert(cell.source[0]);
        }
        let reference = &self.built.operands[first][0];
        let mut selected = Vec::new();
        for (mut polygon, source, owners) in patches.into_values() {
            self.budget.spend(inputs.len() + 1)?;
            let include = match operation {
                BrepBooleanOperation::Union => true,
                BrepBooleanOperation::Intersection => inputs.iter().all(|i| owners.contains(i)),
                BrepBooleanOperation::Difference => {
                    owners.contains(&first) && inputs.iter().skip(1).all(|i| !owners.contains(i))
                }
            };
            if include {
                if dot(&polygon.normal, &reference.normal).is_negative() {
                    polygon = polygon.reverse();
                }
                selected.push((polygon, source));
            }
        }
        self.export_open_patches(selected)
    }

    /// Export a common/exclusive partition of two finite coplanar sheets.
    /// Distinct category labels retain overlap seams even when one original
    /// supporting face contributes both common and exclusive pieces.
    pub fn export_coplanar_sheet_partition(
        &mut self,
        first: usize,
        second: usize,
    ) -> Result<Vec<BrepCoplanarPartitionComponent>, GeometryError> {
        if !self.inputs_are_coplanar(first, second)? {
            return Err(unsupported("coplanar physical sheets required"));
        }
        let mut patches =
            BTreeMap::<Vec<ExactPoint>, [Option<(Polygon<'a>, [usize; 2])>; 2]>::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if !cell.source_covers_cell {
                continue;
            }
            let side = if cell.source[0] == first {
                0
            } else if cell.source[0] == second {
                1
            } else {
                continue;
            };
            patches
                .entry(canonical_ring(&cell.polygon.ring))
                .or_insert_with(|| [None, None])[side] = Some((cell.polygon.clone(), cell.source));
        }
        let mut categories = BTreeMap::new();
        let mut originals = Vec::new();
        let mut selected = Vec::new();
        for [a, b] in patches.into_values() {
            let category = usize::from(a.is_some()) + 2 * usize::from(b.is_some());
            let (polygon, source) = b.or(a).unwrap();
            let next = categories.len();
            let index = *categories.entry((category, source)).or_insert_with(|| {
                originals.push((source, category as u8));
                next
            });
            selected.push((polygon, [0, index]));
        }
        self.export_open_patches(selected)?
            .into_iter()
            .map(|body| {
                let labels = body
                    .face_sources
                    .iter()
                    .map(|s| originals[s[1]])
                    .collect::<Vec<_>>();
                Ok(BrepCoplanarPartitionComponent {
                    brep: body.brep,
                    face_sources: labels.iter().map(|l| l.0).collect(),
                    face_categories: labels.iter().map(|l| l.1).collect(),
                })
            })
            .collect()
    }

    /// Whether another operand has both inside and outside states on the
    /// finite physical boundary of an original input. Missing virtual patches
    /// do not count as physical crossings.
    pub fn input_boundary_crosses_region(
        &mut self,
        input: usize,
        other: usize,
    ) -> Result<bool, GeometryError> {
        let own = self.input(input)?;
        let other = self.input(other)?;
        let mut inside = false;
        let mut outside = false;
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if cell.source[0] != input || !cell.source_covers_cell {
                continue;
            }
            let sides = cell.sides.map(|i| own.mask[i]);
            if sides[0] == sides[1] {
                continue;
            }
            inside |= cell.sides.iter().any(|&i| other.mask[i]);
            outside |= cell.sides.iter().all(|&i| !other.mask[i]);
        }
        Ok(inside && outside)
    }

    /// Whether an operand's physical sheets completely cover the crossing
    /// section of another original operand, using exact interval coverage.
    pub fn planar_sheet_covers_input_section(
        &mut self,
        sheet: usize,
        operand: usize,
    ) -> Result<bool, GeometryError> {
        let region = self.input(operand)?;
        if sheet >= self.built.operands.len() {
            return Err(unsupported("plan input index out of range"));
        }
        self.sheet_covers_boundary_section(&region, &[sheet])
    }

    /// Export the physical part of an oriented region boundary, allowing
    /// naked edges where finite input sheets end. Planning patches are omitted.
    pub fn export_physical_boundary(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        let patches = self.physical_patches(region)?;
        self.export_open_patches(patches)
    }

    pub(super) fn from_arrangement(
        built: arrangement::Arrangement<'a>,
        tolerance: Tolerance,
        budget: Budget,
    ) -> Self {
        Self {
            built,
            identity: Arc::new(()),
            tolerance,
            budget,
            exported_faces: 0,
            use_outer_coverage: false,
        }
    }

    /// Connected material regions before any output rounding. Nested islands
    /// are separated from enclosing material; inward cavity shells stay attached.
    pub fn components(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralRegion>, GeometryError> {
        let shells = self.shells(region)?;
        let outer = shells
            .into_iter()
            .filter(|s| !s.inward)
            .map(|s| s.region)
            .collect::<Vec<_>>();
        let mut result = Vec::new();
        for (i, shell) in outer.iter().enumerate() {
            let mut nested = Vec::new();
            for (j, other) in outer.iter().enumerate() {
                if i != j && self.covered_by(other, shell)? {
                    if self.covered_by(shell, other)? {
                        return Err(GeometryError::UnrepresentableBrepBoolean);
                    }
                    nested.push(other);
                }
            }
            let enclosed = self.combine(BrepBooleanOperation::Intersection, &[region, shell])?;
            let piece = if nested.is_empty() {
                enclosed
            } else {
                let islands = self.combine(BrepBooleanOperation::Union, &nested)?;
                self.combine(BrepBooleanOperation::Difference, &[&enclosed, &islands])?
            };
            if !self.is_empty(&piece)? {
                result.push(piece);
            }
        }
        Ok(result)
    }

    pub(super) fn components_with_sheet_holes(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralRegion>, GeometryError> {
        self.use_outer_coverage = true;
        let result = self.components(region);
        self.use_outer_coverage = false;
        result
    }
    pub(super) fn sheet_outer_covers_region(
        &mut self,
        region: &BrepPolyhedralRegion,
        group: &[usize],
    ) -> Result<bool, GeometryError> {
        self.check(region)?;
        let mut patches = BTreeMap::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if group.contains(&cell.source[0]) && cell.sides.iter().all(|&i| region.mask[i]) {
                *patches
                    .entry(canonical_ring(&cell.polygon.ring))
                    .or_insert(false) |= cell.outer_covers_cell;
            }
        }
        Ok(!patches.is_empty() && patches.values().all(|v| *v))
    }
    /// All planning patches on the group's plane must be physically covered
    /// wherever they separate material in the current bounded region.
    pub(super) fn sheet_covers_region(
        &mut self,
        region: &BrepPolyhedralRegion,
        group: &[usize],
    ) -> Result<bool, GeometryError> {
        self.check(region)?;
        let mut patches = BTreeMap::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if group.contains(&cell.source[0]) && cell.sides.iter().all(|&i| region.mask[i]) {
                let entry = patches
                    .entry(canonical_ring(&cell.polygon.ring))
                    .or_insert(false);
                *entry |= cell.source_covers_cell;
            }
        }
        Ok(!patches.is_empty() && patches.values().all(|covered| *covered))
    }

    /// Complete segment coverage on every physical boundary crossing of a
    /// finite sheet. This also applies to an open, unbounded half-space region.
    pub(super) fn sheet_covers_boundary_section(
        &mut self,
        region: &BrepPolyhedralRegion,
        group: &[usize],
    ) -> Result<bool, GeometryError> {
        let (mut polygons, _) = self.boundary(region)?;
        subdivide(&mut polygons, &mut self.budget)?;
        let plane = &self.built.operands[group[0]][0];
        let mut edges = BTreeMap::<[ExactPoint; 2], [bool; 2]>::new();
        for polygon in &polygons {
            let center = mean(&polygon.ring, &mut self.budget)?;
            let sign = plane.plane_side(&center);
            if sign.is_zero() {
                continue;
            }
            for i in 0..polygon.ring.len() {
                self.budget.spend(1)?;
                let a = &polygon.ring[i];
                let b = &polygon.ring[(i + 1) % polygon.ring.len()];
                if plane.plane_side(a).is_zero() && plane.plane_side(b).is_zero() {
                    let key = if a < b {
                        [a.clone(), b.clone()]
                    } else {
                        [b.clone(), a.clone()]
                    };
                    edges.entry(key).or_default()[usize::from(sign.is_positive())] = true;
                }
            }
        }
        let mut found = false;
        for (edge, sides) in edges {
            if !sides.iter().all(|v| *v) {
                continue;
            }
            found = true;
            let mut intervals = Vec::new();
            for &owner in group {
                for polygon in &self.built.operands[owner] {
                    if let Some(interval) = segment_interval(&edge, polygon, &mut self.budget)? {
                        intervals.push(interval);
                    }
                }
            }
            intervals.sort();
            let mut end = Rational::zero();
            for [lo, hi] in intervals {
                if lo > end {
                    return Ok(false);
                }
                end = end.max(hi);
            }
            if end < rational(1.) {
                return Ok(false);
            }
        }
        Ok(found)
    }

    pub fn input(&mut self, index: usize) -> Result<BrepPolyhedralRegion, GeometryError> {
        if index >= self.built.operands.len() {
            return Err(unsupported("plan input index out of range"));
        }
        self.budget.spend(self.built.samples.len())?;
        let mut region = self.region(self.built.samples.iter().map(|s| s.inside[index]).collect());
        region.origin = Some(index);
        Ok(region)
    }

    fn region(&self, mask: Vec<bool>) -> BrepPolyhedralRegion {
        BrepPolyhedralRegion {
            plan: self.identity.clone(),
            mask,
            origin: None,
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
            if !c.source_covers_cell {
                continue;
            }
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
            if !c.source_covers_cell {
                continue;
            }
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
            if !c.source_covers_cell {
                continue;
            }
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
            if !c.source_covers_cell {
                continue;
            }
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
            if !c.source_covers_cell {
                continue;
            }
            if a.mask[c.sides[0]] != a.mask[c.sides[1]] && b.mask[c.sides[0]] != b.mask[c.sides[1]]
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Positive-length common boundary, including edge-only contact. Isolated
    /// point contacts are excluded. Equality is not implicitly excluded.
    pub fn boundaries_share_line(
        &mut self,
        a: &BrepPolyhedralRegion,
        b: &BrepPolyhedralRegion,
    ) -> Result<bool, GeometryError> {
        let (left, _) = self.boundary(a)?;
        let (right, _) = self.boundary(b)?;
        interactions::edge_contact(&left, &right, &mut self.budget)
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
            if !(c.source_covers_cell || self.use_outer_coverage && c.outer_covers_cell) {
                continue;
            }
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
        let (mut polygons, sources) = if let Some(index) = region.origin {
            self.original_boundary(region, index)?
        } else {
            self.boundary(region)?
        };
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

    fn original_boundary(
        &mut self,
        region: &BrepPolyhedralRegion,
        index: usize,
    ) -> Result<Boundary<'a>, GeometryError> {
        self.check(region)?;
        // Input embedding is already certified. Its material side is constant
        // across each original face, including decomposed faces with holes.
        // Check all cells before using unsplit polygons for exact shell rays.
        let mut faces = BTreeMap::new();
        for cell in &self.built.cells {
            self.budget.spend(1)?;
            if cell.source[0] != index {
                continue;
            }
            let sides = cell.sides.map(|i| region.mask[i]);
            if sides[0] == sides[1] {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
            let key = std::ptr::from_ref(cell.polygon.source);
            let value = (cell.source, sides[0]);
            if faces.insert(key, value).is_some_and(|old| old != value) {
                return Err(GeometryError::UnrepresentableBrepBoolean);
            }
        }
        let mut polygons = Vec::new();
        let mut sources = Vec::new();
        for polygon in &self.built.operands[index] {
            self.budget.spend(polygon.ring.len())?;
            let &(source, forward) = faces
                .get(&std::ptr::from_ref(polygon.source))
                .ok_or(GeometryError::UnrepresentableBrepBoolean)?;
            polygons.push(if forward {
                polygon.clone()
            } else {
                polygon.clone().reverse()
            });
            sources.push(source);
        }
        Ok((polygons, sources))
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

    /// Export separate connected boundaries, retaining inward orientation and
    /// intentional nonmanifold contacts. Every face is geometrically validated
    /// after the sole rounding step. Unlike `export`, this does not require a
    /// manifold material solid or attach cavities to their enclosing body.
    pub fn export_boundary(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<BrepPolyhedralBoundaryComponent>, GeometryError> {
        self.export_boundary_from_faces(region, None, false)
    }

    /// Boundary export with complete original-face coverage checked before
    /// rebuilding topology. See `export_with_boundary_faces` for ownership.
    pub fn export_boundary_with_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
        faces: &[[usize; 2]],
    ) -> Result<Vec<BrepPolyhedralBoundaryComponent>, GeometryError> {
        if faces.len() > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        self.budget.spend(faces.len())?;
        self.export_boundary_from_faces(region, Some(&faces.iter().copied().collect()), false)
    }

    pub(super) fn outer_patches(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<(Polygon<'a>, [usize; 2])>, GeometryError> {
        self.use_outer_coverage = true;
        let result = self.physical_patches(region);
        self.use_outer_coverage = false;
        result
    }
    pub(super) fn physical_patches(
        &mut self,
        region: &BrepPolyhedralRegion,
    ) -> Result<Vec<(Polygon<'a>, [usize; 2])>, GeometryError> {
        let (polygons, sources) = self.boundary(region)?;
        Ok(polygons.into_iter().zip(sources).collect())
    }
    pub(super) fn export_open_patches(
        &mut self,
        patches: Vec<(Polygon<'a>, [usize; 2])>,
    ) -> Result<Vec<BrepPolyhedralBooleanComponent>, GeometryError> {
        self.budget.spend(patches.len())?;
        self.exported_faces += patches.len();
        if self.exported_faces > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        if patches.is_empty() {
            return Ok(vec![]);
        }
        let sources = patches.iter().map(|(_, s)| *s).collect::<Vec<_>>();
        let built = rebuild_open_boundary(
            patches.into_iter().map(|(p, _)| p).collect(),
            self.tolerance,
            &mut self.budget,
        )?;
        built
            .edge_connected_face_components()
            .into_iter()
            .map(|faces| {
                Ok(BrepPolyhedralBooleanComponent {
                    brep: built.duplicate_faces(&faces, self.tolerance)?,
                    face_sources: faces.into_iter().map(|i| sources[i]).collect(),
                })
            })
            .collect()
    }

    fn export_boundary_from_faces(
        &mut self,
        region: &BrepPolyhedralRegion,
        allowed: Option<&BTreeSet<[usize; 2]>>,
        allow_open: bool,
    ) -> Result<Vec<BrepPolyhedralBoundaryComponent>, GeometryError> {
        let (polygons, sources) = self.boundary_from_faces(region, allowed)?;
        self.exported_faces += polygons.len();
        if self.exported_faces > MAX_OUTPUT_FACES {
            return Err(GeometryError::BrepBooleanWorkLimit);
        }
        if polygons.is_empty() {
            return Ok(Vec::new());
        }
        let keys = polygons
            .iter()
            .map(|p| canonical_ring(&p.ring))
            .collect::<Vec<_>>();
        let mut aliases = BTreeMap::<Vec<ExactPoint>, BTreeSet<[usize; 2]>>::new();
        let mut originals = vec![BTreeSet::new(); self.built.operands.len()];
        for cell in &self.built.cells {
            self.budget.spend(originals.len() + 1)?;
            if !cell.source_covers_cell {
                continue;
            }
            let key = canonical_ring(&cell.polygon.ring);
            if region.mask[cell.sides[0]] != region.mask[cell.sides[1]]
                && allowed.is_none_or(|a| a.contains(&cell.source))
            {
                aliases.entry(key.clone()).or_default().insert(cell.source);
            }
            for (i, own) in originals.iter_mut().enumerate() {
                if self.built.samples[cell.sides[0]].inside[i]
                    != self.built.samples[cell.sides[1]].inside[i]
                {
                    own.insert(key.clone());
                }
            }
        }
        let built = if allow_open {
            rebuild_open_boundary(polygons, self.tolerance, &mut self.budget)?
        } else {
            rebuild_boundary(polygons, self.tolerance, &mut self.budget)?
        };
        let mut result = Vec::new();
        for faces in built.edge_connected_face_components() {
            self.budget.spend(faces.len() + originals.len())?;
            let own = faces
                .iter()
                .map(|&f| keys[f].clone())
                .collect::<BTreeSet<_>>();
            let boundary_faces = faces
                .iter()
                .flat_map(|&f| aliases[&keys[f]].iter())
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            result.push(BrepPolyhedralBoundaryComponent {
                brep: built.duplicate_faces(&faces, self.tolerance)?,
                face_sources: faces.iter().map(|&f| sources[f]).collect(),
                boundary_faces,
                boundary_equal_inputs: originals
                    .iter()
                    .enumerate()
                    .filter_map(|(i, keys)| (keys == &own).then_some(i))
                    .collect(),
            });
        }
        Ok(result)
    }
}

fn segment_interval(
    edge: &[ExactPoint; 2],
    polygon: &Polygon<'_>,
    budget: &mut Budget,
) -> Result<Option<[Rational; 2]>, GeometryError> {
    let (mut lo, mut hi) = (Rational::zero(), rational(1.));
    for i in 0..polygon.ring.len() {
        budget.spend(1)?;
        let a = &polygon.ring[i];
        let b = &polygon.ring[(i + 1) % polygon.ring.len()];
        let side = |p: &ExactPoint| dot(&polygon.normal, &cross(&sub(b, a), &sub(p, a)));
        let x = side(&edge[0]);
        let y = side(&edge[1]);
        if x.is_negative() && y.is_negative() {
            return Ok(None);
        }
        if x.is_negative() || y.is_negative() {
            let t = &x / (&x - &y);
            check_scalar(&t)?;
            if x.is_negative() {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
            if lo > hi {
                return Ok(None);
            }
        }
    }
    Ok(Some([lo, hi]))
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
