//! Block-entity model data selections (#861 part 2), as the client's world thread froze them for
//! the mesher: which baked models a block draws instead of its state's own.
//!
//! The game evaluates every block entity's model data on its own thread and writes the chosen
//! model slots here. A mesh job takes the plan of its one section when it is scheduled (an `Arc`
//! clone each), so a worker reads arrays and never asks a mod. With no mod registering model data
//! the table stays empty, every plan is `None` and the mesher keeps its own path.
use crate::fast_hash::FxHashMap;
use std::sync::Arc;

/// Baked model slots, in draw order, that replace a block state's own models.
pub type Selection = Arc<[u32]>;

/// What one mesh job reads: the per-state defaults and its own section's selections.
#[derive(Clone, Default)]
pub struct ModelDataPlan {
    /// By state id: the selection its declared defaults make, none for a state without one.
    defaults: Arc<[Option<Selection>]>,
    /// By cell (`y << 8 | z << 4 | x` in the section): a block entity's evaluated selection.
    /// `None` inside keeps the state's own models even where its defaults select a variant.
    cells: Option<Arc<FxHashMap<u16, Option<Selection>>>>,
}

impl ModelDataPlan {
    /// The models the block of `state` at cell `x, y, z` of the section draws, when model data
    /// replaces its state's own: its evaluated selection, else its state's default.
    #[inline]
    pub fn at(&self, state: u32, [x, y, z]: [i32; 3]) -> Option<&[u32]> {
        if let Some(cells) = &self.cells
            && let Some(cell) = cells.get(&((y as u16) << 8 | (z as u16) << 4 | x as u16))
        {
            return cell.as_deref();
        }
        self.defaults.get(state as usize)?.as_deref()
    }
}

/// The world's model data selections, by section (`x`, section index, `z`).
#[derive(Default)]
pub struct ModelDataSections {
    /// The model store generation the defaults were resolved for; 0 before any.
    generation: u64,
    defaults: Arc<[Option<Selection>]>,
    sections: FxHashMap<(i32, i32, i32), Arc<FxHashMap<u16, Option<Selection>>>>,
}

impl ModelDataSections {
    /// Whether no state has a default and no block entity a selection: the mesher's own path.
    pub fn is_empty(&self) -> bool {
        self.defaults.is_empty() && self.sections.is_empty()
    }

    /// The model store generation the table was last reset for; 0 before any.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Replaces the defaults by state (empty for none) for store `generation` and drops every
    /// evaluated selection: a resource load resolves every variant again.
    pub fn reset(&mut self, generation: u64, defaults: Arc<[Option<Selection>]>) {
        self.generation = generation;
        self.defaults = defaults;
        self.sections.clear();
    }

    /// The plan of section `(x, sy, z)`, or `None` with nothing to select there or anywhere.
    pub fn plan(&self, x: i32, sy: i32, z: i32) -> Option<ModelDataPlan> {
        if self.is_empty() {
            return None;
        }
        Some(ModelDataPlan {
            defaults: self.defaults.clone(),
            cells: self.sections.get(&(x, sy, z)).cloned(),
        })
    }

    /// Sets what the block at `pos` (in section index `sy`, counted from the level's bottom)
    /// draws. Returns whether that changed: the caller marks the section dirty then.
    pub fn set(&mut self, pos: [i32; 3], sy: i32, selection: Entry) -> bool {
        let key = (pos[0] >> 4, sy, pos[2] >> 4);
        let cell = ((pos[1] & 15) as u16) << 8 | ((pos[2] & 15) as u16) << 4 | (pos[0] & 15) as u16;
        let current = self.sections.get(&key).and_then(|cells| cells.get(&cell));
        match selection {
            Entry::Default => {
                if current.is_none() {
                    return false;
                }
                let cells = Arc::make_mut(self.sections.get_mut(&key).expect("present"));
                cells.remove(&cell);
                if cells.is_empty() {
                    self.sections.remove(&key);
                }
                true
            }
            Entry::Selected(selection) => {
                if current == Some(&selection) {
                    return false;
                }
                Arc::make_mut(self.sections.entry(key).or_default()).insert(cell, selection);
                true
            }
        }
    }

    /// Drops the selections of column `(x, z)`, unloaded.
    pub fn remove_column(&mut self, x: i32, z: i32) {
        self.sections.retain(|&(sx, _, sz), _| (sx, sz) != (x, z));
    }
}

/// What a block entity's evaluated model data makes its block draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// Its state's default, as if never evaluated.
    Default,
    /// These models (`Some`), or the state's own (`None`) even where the default selects one.
    Selected(Option<Selection>),
}

#[cfg(test)]
#[path = "model_data_tests.rs"]
mod tests;
