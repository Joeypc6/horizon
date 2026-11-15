//! Block-entity model data (#861 part 2) on the client's world thread: the registrations
//! (`register_block_model_data`), their variants resolved for each published model store, and
//! each block entity's answer (`block_entity_model_data`) checked and frozen into the selection
//! table the mesh jobs read (`horizon_client::model_data`). Nothing here runs on a meshing worker.
//!
//! With no registration, `ModelData::active` is false: nothing is collected per block entity,
//! queued or evaluated, and the world's table stays empty so the mesher keeps its own path.
use super::Game;
use super::model_data_check::{Registration, check};
use horizon_client::model_data::{Entry, Selection};
use horizon_world::registry::BlockStateId;
use mod_api::{ModelDataEntry, ModelDataRefreshError};
use std::{collections::BTreeSet, sync::Arc};

/// The game's model data: registrations once per process, resolutions once per model store.
#[derive(Default)]
pub(in crate::game) struct ModelData {
    collected: bool,
    registrations: Vec<Registration>,
    /// The model store generation `by_state` and `variants` were resolved for.
    generation: u64,
    /// By state id: its registration's index.
    by_state: Vec<Option<u32>>,
    /// By registration: each variant's model slots, or `None` while that registration is off
    /// for this generation (a model it names isn't baked, or its provider failed).
    variants: Vec<Option<Vec<Selection>>>,
    /// Positions whose block entity asked to be evaluated again, once each.
    pending: Vec<[i32; 3]>,
    /// Positions with an evaluated selection, evaluated again after a resource reload.
    evaluated: horizon_client::fast_hash::FxHashSet<[i32; 3]>,
    /// Registrations already diagnosed this generation, by index.
    warned: BTreeSet<u32>,
}

impl ModelData {
    /// Whether any mod's model data is in use.
    pub(in crate::game) fn active(&self) -> bool {
        !self.registrations.is_empty()
    }
}

impl Game<'_> {
    /// Collects the mods' model data once, on the client, before the first store is resolved.
    pub(in crate::game) fn collect_model_data(&mut self) {
        if self.model_data.collected || !self.options.is_client_start() {
            return;
        }
        self.model_data.collected = true;
        if !mod_api::__private::listened(mod_api::HookId::RegisterBlockModelData) {
            return;
        }
        use mod_api::__private::game::RegistrationContextNew;
        let mut entries = Vec::new();
        Self::native_register_block_model_data(&mod_api::RegistrationContext::new(), &mut entries);
        let registry = self.pack.assets.registry;
        self.model_data.registrations =
            check(entries, |block| registry.default_state(block).is_some());
    }

    /// Resolves the registrations against the published store when it changed, installs the
    /// states' defaults in the world and queues every evaluated block entity again. Then
    /// evaluates the queued block entities. Once per world update; nothing without registrations.
    pub(in crate::game) fn update_model_data(&mut self) {
        if !self.model_data.active() {
            return;
        }
        if self.model_data.generation != self.pack.render.generation
            || self.world.model_data.generation() != self.pack.render.generation
        {
            self.resolve_model_data();
        }
        if self.model_data.pending.is_empty() {
            return;
        }
        for pos in std::mem::take(&mut self.model_data.pending) {
            self.evaluate_model_data(pos);
        }
    }

    /// Queues the block entity at `pos` to answer its model data again before the next mesh.
    pub(in crate::game) fn refresh_model_data(
        &mut self,
        pos: [i32; 3],
    ) -> Result<(), ModelDataRefreshError> {
        if !self.model_data.active() || self.model_data_registration(pos).is_none() {
            return Err(ModelDataRefreshError::NoModelData);
        }
        self.queue_model_data(pos);
        Ok(())
    }

    /// Queues `pos` once, when model data is in use.
    pub(in crate::game) fn queue_model_data(&mut self, pos: [i32; 3]) {
        if self.model_data.active() && !self.model_data.pending.contains(&pos) {
            self.model_data.pending.push(pos);
        }
    }

    /// Drops the selections of the unloaded column `(x, z)`.
    pub(in crate::game) fn unload_model_data(&mut self, x: i32, z: i32) {
        if !self.model_data.active() {
            return;
        }
        self.model_data
            .evaluated
            .retain(|pos| (pos[0] >> 4, pos[2] >> 4) != (x, z));
        self.world.model_data.remove_column(x, z);
    }

    /// The registration index of the mod block entity at `pos`, when its state has one.
    fn model_data_registration(&self, pos: [i32; 3]) -> Option<u32> {
        if !matches!(
            self.world.block_entities.get(&(pos[0], pos[1], pos[2])),
            Some(horizon_client::containers::BlockEntity::Mod(_))
        ) {
            return None;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        *self.model_data.by_state.get(state.0 as usize)?
    }

    fn resolve_model_data(&mut self) {
        let data = &mut self.model_data;
        let render = &self.pack.render;
        let registry = self.pack.assets.registry;
        data.generation = render.generation;
        data.warned.clear();
        data.by_state = vec![None; registry.len()];
        data.variants = data
            .registrations
            .iter()
            .map(|registration| {
                registration
                    .variants
                    .iter()
                    .map(|variant| {
                        variant
                            .models
                            .iter()
                            .map(|id| render.model_catalog.standalone_slot(id))
                            .collect::<Option<Vec<u32>>>()
                            .map(Selection::from)
                    })
                    .collect::<Option<Vec<_>>>()
                    .or_else(|| {
                        log::warn!(
                            "Horizon: E_MODEL_DATA_UNKNOWN_MODEL: mod {}'s model data for {} names \
                             a model that isn't a baked standalone model; its block draws its own \
                             models until the next resource load",
                            registration.owner,
                            registration.block
                        );
                        None
                    })
            })
            .collect();
        // Each registration's default selection, then every state of its block, in one pass.
        let blocks: std::collections::BTreeMap<&str, (u32, Option<Selection>)> = data
            .registrations
            .iter()
            .enumerate()
            .map(|(index, registration)| {
                let default = data.variants[index].as_ref().and_then(|variants| {
                    registration
                        .select(&registration.defaults())
                        .map(|variant| variants[variant].clone())
                });
                (registration.block.as_str(), (index as u32, default))
            })
            .collect();
        let mut defaults = vec![None; registry.len()];
        for id in 0..registry.len() as u32 {
            let Some(state) = registry.state(BlockStateId(id)) else {
                continue;
            };
            if let Some((index, default)) = blocks.get(state.name.as_str()) {
                data.by_state[id as usize] = Some(*index);
                defaults[id as usize] = default.clone();
            }
        }
        let defaults: Arc<[Option<Selection>]> = if defaults.iter().all(Option::is_none) {
            Arc::from([])
        } else {
            defaults.into()
        };
        self.world.model_data.reset(render.generation, defaults);
        // Every block entity answers again for the new store. A reload already remeshes every
        // section and a new world has none meshed, so nothing else is marked here.
        let evaluated = std::mem::take(&mut self.model_data.evaluated);
        self.model_data.pending.extend(evaluated);
    }

    /// Asks the block entity at `pos` for its model data, checks the answer and freezes the
    /// variant it selects; marks the section for remeshing when that changed.
    fn evaluate_model_data(&mut self, pos: [i32; 3]) {
        let Some(index) = self.model_data_registration(pos) else {
            self.model_data.evaluated.remove(&pos);
            self.set_model_data(pos, Entry::Default);
            return;
        };
        let Some(variants) = self.model_data.variants[index as usize].clone() else {
            return;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let block = self.res.facts().get(state).name.clone();
        let mut answer: Vec<ModelDataEntry> = Vec::new();
        let asked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(horizon_client::containers::BlockEntity::Mod(entity)) =
                self.world.block_entities.get(&(pos[0], pos[1], pos[2]))
            {
                self.api_block_entity_model_data(pos, &block, state, entity, &mut answer);
            }
        }));
        let registration = &self.model_data.registrations[index as usize];
        let owner = registration.owner;
        if asked.is_err() {
            log::warn!(
                "Horizon: E_MODEL_DATA_PANIC: mod {owner}'s block_entity_model_data for {block} \
                 panicked; its blocks draw their own models until the next resource load"
            );
            self.model_data.variants[index as usize] = None;
            self.set_model_data(pos, Entry::Default);
            return;
        }
        let (values, refused) = match registration.values(&answer) {
            Ok(values) => (values, None),
            Err(why) => (registration.defaults(), Some(why)),
        };
        let selection = registration
            .select(&values)
            .map(|variant| variants[variant].clone());
        if let Some(why) = refused
            && self.model_data.warned.insert(index)
        {
            log::warn!(
                "Horizon: E_MODEL_DATA_REFUSED: mod {owner}'s model data for {block} at {pos:?} \
                 refused whole ({why}); its defaults are drawn"
            );
        }
        self.model_data.evaluated.insert(pos);
        self.set_model_data(pos, Entry::Selected(selection));
    }

    fn set_model_data(&mut self, pos: [i32; 3], entry: Entry) {
        let sy = (pos[1] - self.world.shape.min_y) >> 4;
        if self.world.model_data.set(pos, sy, entry) {
            self.world
                .dirty_sections
                .insert((pos[0] >> 4, sy, pos[2] >> 4));
        }
    }

    fn api_block_entity_model_data(
        &self,
        pos: [i32; 3],
        block: &str,
        state: BlockStateId,
        entity: &crate::containers::ModBlockEntity,
        answer: &mut Vec<ModelDataEntry>,
    ) {
        use crate::game::api_values::TryIntoApi;
        if !mod_api::__private::has_part_hook(mod_api::HookId::BlockEntityModelData) {
            return;
        }
        if let Ok(api) = entity.try_into_api() {
            self.native_block_entity_model_data(pos, block, state, &api, answer);
        }
    }
}
