use super::*;
use mod_api::BlockModelData;

impl Game<'_> {
    /// Block-entity model data (#861): each client mod pushes a `mod_api::BlockModelData` for a
    /// block whose block entity hands the renderer typed values, its properties and the
    /// standalone-model variants they select. Asked once on the client, before the first store is
    /// resolved; dispatch names the mod on each entry, so a refusal names its owner.
    pub(in crate::game) fn native_register_block_model_data(
        context: &mod_api::RegistrationContext,
        entries: &mut Vec<mod_api::Answered<BlockModelData>>,
    ) {
        Self::fire_register_block_model_data(context, entries);
    }
}
