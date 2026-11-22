// SPDX-License-Identifier: GPL-3.0-or-later
//! The undo stack (`UndoStack`, `blenkernel/intern/undo_system.cc`, Blender 5.2.4,
//! GPL-2.0-or-later): an ordered list of named steps with an active one, generic over the step
//! data so the algorithm is tested on its own (`undo/tests.rs`). Ported from
//! blender/source/blender/blenkernel/intern/undo_system.cc with its compile-time options
//! `WITH_GLOBAL_UNDO_KEEP_ONE` and `WITH_GLOBAL_UNDO_CORRECT_ORDER` (the third,
//! `WITH_GLOBAL_UNDO_ENSURE_UPDATED`, needs the encoder and lives in `undo.rs`).
//!
//! Steps are kept in a `Vec` (Blender's `ListBase`, at most `edit.undo_steps` + 1 entries), so
//! `active` and `active_memfile` are indices that move when the oldest steps are freed.

use smol_str::SmolStr;

/// `eUndoStepDir`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    /// `STEP_UNDO` (-1).
    Undo,
    /// `STEP_REDO` (1).
    Redo,
}

/// What the stack needs to know about a step's data.
pub(crate) trait StepData {
    /// `us->type == BKE_UNDOSYS_TYPE_MEMFILE`: a global (whole `Main`) step. Every other type
    /// has `step_foreach_ID_ref` (edit mesh, edit armature), so decoding one first loads the
    /// memfile step it was made on (`WITH_GLOBAL_UNDO_CORRECT_ORDER`).
    fn is_memfile(&self) -> bool;
}

/// `UndoStep`.
#[derive(Clone, Debug)]
pub(crate) struct Step<D> {
    /// `us->name` (`BKE_UNDO_STR_MAX` 64: at most 63 bytes).
    pub(crate) name: SmolStr,
    /// The encoded state.
    pub(crate) data: D,
    /// `us->skip`: never a user-visible stop (internal memfile steps, the memfile kept below
    /// the step limit).
    pub(crate) skip: bool,
}

/// `UndoStack`.
#[derive(Clone, Debug)]
pub(crate) struct Stack<D> {
    /// `steps`, oldest first.
    pub(crate) steps: Vec<Step<D>>,
    /// `step_active`.
    pub(crate) active: Option<usize>,
    /// `step_active_memfile`: the memfile step the current `Main` was last loaded from or
    /// written to.
    pub(crate) active_memfile: Option<usize>,
}

impl<D> Default for Stack<D> {
    fn default() -> Self {
        Stack {
            steps: Vec::new(),
            active: None,
            active_memfile: None,
        }
    }
}

/// `BKE_UNDO_STR_MAX - 1`: the longest step name, cut on a character boundary.
pub(crate) fn step_name(name: &str) -> SmolStr {
    let mut end = name.len().min(63);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    SmolStr::new(&name[..end])
}

impl<D: StepData> Stack<D> {
    /// `BKE_undosys_stack_clear`.
    pub(crate) fn clear(&mut self) {
        self.steps.clear();
        self.active = None;
        self.active_memfile = None;
    }

    /// `step_active->next == nullptr` (also true for an empty stack).
    pub(crate) fn active_is_last(&self) -> bool {
        match self.active {
            Some(a) => a + 1 == self.steps.len(),
            None => true,
        }
    }

    /// `ed_undo_poll`: the active step has a previous one.
    pub(crate) fn can_undo(&self) -> bool {
        self.active.is_some_and(|a| a > 0)
    }

    /// `ed_redo_poll` / `BKE_undosys_stack_has_redo`: the active step has a next one.
    pub(crate) fn can_redo(&self) -> bool {
        self.active.is_some_and(|a| a + 1 < self.steps.len())
    }

    /// `BKE_undosys_step_find_by_name`: the last step with this name.
    pub(crate) fn find_by_name(&self, name: &str) -> Option<usize> {
        self.steps.iter().rposition(|s| s.name == name)
    }

    /// `BKE_undosys_stack_has_undo(ustack, name)`: a step of that name with a previous one.
    pub(crate) fn has_undo_named(&self, name: &str) -> bool {
        self.find_by_name(name).is_some_and(|i| i > 0)
    }

    /// Frees step `i` (`undosys_step_free_and_unlink`), shifting the indices after it.
    fn remove(&mut self, i: usize) {
        self.steps.remove(i);
        let fix = |slot: &mut Option<usize>| {
            *slot = match *slot {
                Some(a) if a == i => None,
                Some(a) if a > i => Some(a - 1),
                other => other,
            };
        };
        fix(&mut self.active);
        fix(&mut self.active_memfile);
    }

    /// `BKE_undosys_step_push_with_type`'s first loop: removes every step after the active
    /// one (every step when there is no active one).
    pub(crate) fn truncate_after_active(&mut self) {
        let keep = self.active.map_or(0, |a| a + 1);
        while self.steps.len() > keep {
            self.remove(self.steps.len() - 1);
        }
    }

    /// `BKE_undosys_stack_clear_active`: removes the active step and every later one; the
    /// previous step becomes active (`ED_undo_grouped_push`).
    pub(crate) fn clear_active(&mut self) {
        if let Some(a) = self.active {
            self.active = a.checked_sub(1);
            while self.steps.len() > a {
                self.remove(self.steps.len() - 1);
            }
        }
    }

    /// Appends an encoded step and makes it active (the tail of
    /// `BKE_undosys_step_push_with_type`; `undosys_step_encode` makes a memfile step the
    /// active memfile).
    pub(crate) fn append(&mut self, name: SmolStr, data: D, skip: bool) -> usize {
        let memfile = data.is_memfile();
        self.steps.push(Step { name, data, skip });
        let i = self.steps.len() - 1;
        self.active = Some(i);
        if memfile {
            self.active_memfile = Some(i);
        }
        i
    }

    /// `BKE_undosys_stack_limit_steps_and_memory(ustack, steps, 0)`: keeps the newest `steps`
    /// non-skipped steps below the last one (and the first step), freeing the older ones; the
    /// memfile step below a kept non-memfile step survives as a skipped step
    /// (`WITH_GLOBAL_UNDO_KEEP_ONE`).
    pub(crate) fn limit_steps(&mut self, steps: usize) {
        if self.steps.is_empty() {
            return;
        }
        // `for (us = last; us && us->prev; us = us->prev)`.
        let mut us = self.steps.len() - 1;
        let mut count = 0usize;
        while us > 0 {
            if count == steps {
                break;
            }
            if !self.steps[us].skip {
                count += 1;
            }
            us -= 1;
        }
        let mut exclude = None;
        if !self.steps[us].data.is_memfile() {
            exclude = (0..us).rev().find(|&i| self.steps[i].data.is_memfile());
            if let Some(x) = exclude {
                self.steps[x].skip = true;
            }
        }
        // `undosys_stack_clear_all_first(ustack, us->prev, us_exclude)`: frees from the first
        // step through `us->prev`, keeping `us_exclude`.
        let Some(mut last) = us.checked_sub(1) else {
            return;
        };
        if Some(last) == exclude {
            let Some(l) = last.checked_sub(1) else {
                return;
            };
            last = l;
        }
        // Every step at or below `last` except `exclude` goes (newest first, so the lower
        // indices stay valid).
        for i in (0..=last).rev() {
            if Some(i) != exclude {
                self.remove(i);
            }
        }
    }

    /// `BKE_undosys_step_calc_direction(ustack, target, reference)`.
    fn direction(target: usize, reference: usize) -> Dir {
        if target <= reference {
            Dir::Undo
        } else {
            Dir::Redo
        }
    }

    /// `BKE_undosys_step_load_data_ex(ustack, C, target, reference, use_skip)`: decodes the
    /// steps from `reference` (exclusive) towards `target`, stepping over skipped steps when
    /// `use_skip`, and makes the last one decoded active. `decode(data, dir, is_final)` loads
    /// one step; a non-memfile step first has the memfile step below it loaded when that is
    /// not the active memfile (`WITH_GLOBAL_UNDO_CORRECT_ORDER`, #56163). False when no step
    /// is left after stepping over skipped ones (the undo limit).
    pub(crate) fn load(
        &mut self,
        target: usize,
        reference: usize,
        use_skip: bool,
        decode: &mut dyn FnMut(&D, Dir, bool),
    ) -> bool {
        let dir = Self::direction(target, reference);
        let step = |i: usize| -> Option<usize> {
            match dir {
                Dir::Undo => i.checked_sub(1),
                Dir::Redo => Some(i + 1),
            }
        };
        let n = self.steps.len();
        let mut target_active = Some(target);
        if use_skip {
            while let Some(t) = target_active {
                if t >= n || !self.steps[t].skip {
                    break;
                }
                target_active = step(t);
            }
            if target_active.is_none_or(|t| t >= n) {
                // "undo/redo did not find a step after stepping over skip-steps (undo limit
                // exceeded)".
                return false;
            }
        }
        let Some(target_active) = target_active else {
            return false;
        };
        if target_active == reference {
            return true;
        }
        let mut it = step(reference);
        while let Some(i) = it {
            if i >= n {
                break;
            }
            let is_final = i == target_active;
            if !self.steps[i].data.is_memfile()
                && let Some(m) = (0..i).rev().find(|&j| self.steps[j].data.is_memfile())
                && Some(m) != self.active_memfile
            {
                decode(&self.steps[m].data, dir, false);
                self.active_memfile = Some(m);
            }
            decode(&self.steps[i].data, dir, is_final);
            if self.steps[i].data.is_memfile() {
                self.active_memfile = Some(i);
            }
            self.active = Some(i);
            if is_final {
                return true;
            }
            it = step(i);
        }
        false
    }

    /// `BKE_undosys_step_undo`: to the step before the active one, past skipped steps.
    pub(crate) fn undo(&mut self, decode: &mut dyn FnMut(&D, Dir, bool)) -> bool {
        match self.active {
            Some(a) if a > 0 => self.load(a - 1, a, true, decode),
            _ => false,
        }
    }

    /// `BKE_undosys_step_redo`: to the step after the active one, past skipped steps.
    pub(crate) fn redo(&mut self, decode: &mut dyn FnMut(&D, Dir, bool)) -> bool {
        match self.active {
            Some(a) if a + 1 < self.steps.len() => self.load(a + 1, a, true, decode),
            _ => false,
        }
    }

    /// `ed_undo_step_by_name` (`ED_undo_pop_op`): loads the step before the last one named
    /// `name`. False (CANCELLED) when there is none.
    pub(crate) fn undo_by_name(
        &mut self,
        name: &str,
        decode: &mut dyn FnMut(&D, Dir, bool),
    ) -> bool {
        let Some(found) = self.find_by_name(name) else {
            return false;
        };
        let Some(target) = found.checked_sub(1) else {
            return false;
        };
        let Some(reference) = self.active else {
            return false;
        };
        self.load(target, reference, true, decode)
    }
}
