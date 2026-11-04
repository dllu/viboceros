//! Deterministic proposals stay outside committed component selection.
use super::*;
impl Session {
    fn append(
        &self,
        state: &mut State,
        side: usize,
        next: ChainContinuation,
        journal: &mut Vec<UndoFrame>,
    ) -> Result<(), CommandError> {
        journal.push(UndoFrame::new(state, next.index));
        state.selected.insert(next.index);
        state.fronts[side] = Some((next.index, !next.reversed));
        if let Some(other) = state.fronts[1 - side]
            && self.index.ends_meet(
                (next.index, !next.reversed),
                other,
                self.options.gap_tolerance,
            )?
        {
            state.fronts = [None, None];
        }
        Ok(())
    }
    pub(super) fn propose(
        &self,
        base: &State,
        side: usize,
        candidates: Vec<ChainContinuation>,
        choice: usize,
    ) -> Result<Pending, CommandError> {
        let mut state = base.clone();
        let mut journal = Vec::new();
        self.append(&mut state, side, candidates[choice], &mut journal)?;
        self.grow(&mut state, false, &mut journal, false)?;
        Ok(Pending {
            state,
            journal,
            candidates,
        })
    }
    pub(super) fn grow(
        &self,
        state: &mut State,
        one: bool,
        journal: &mut Vec<UndoFrame>,
        allow_proposal: bool,
    ) -> Result<Option<Pending>, CommandError> {
        for side in self.sides() {
            while let Some((source, at_end)) = state.fronts[side] {
                let mut next = self.index.continuations(
                    source,
                    at_end,
                    self.options,
                    &state.selected,
                    &self.classes,
                )?;
                let preferred = state.preferred.unwrap_or(self.classes[source]);
                if next.iter().any(|c| self.classes[c.index] == preferred) {
                    next.retain(|c| self.classes[c.index] == preferred);
                }
                if next.is_empty() {
                    break;
                }
                if next.len() > 1 && allow_proposal {
                    return Ok(Some(self.propose(state, side, next, 0)?));
                }
                self.append(state, side, next[0], journal)?;
                if one {
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }
}
