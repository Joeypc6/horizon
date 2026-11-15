use super::*;
use mod_api::ModBlockEntity;

impl Game<'_> {
    /// `IBlockEntityExtension.getModelData`: the mod block entity at `pos` pushes the
    /// `mod_api::ModelDataEntry`s its block declared in `register_block_model_data`. Asked on the
    /// client's world thread when it loads, after `refresh_model_data` and after a resource
    /// reload; never on a meshing worker. The game checks the answer whole before it selects.
    pub(in crate::game) fn native_block_entity_model_data(
        &self,
        pos: [i32; 3],
        block: &str,
        state: BlockStateId,
        entity: &ModBlockEntity,
        data: &mut Vec<ModelDataEntry>,
    ) {
        self.fire_block_entity_model_data(pos, block, state, entity, data);
    }
}
