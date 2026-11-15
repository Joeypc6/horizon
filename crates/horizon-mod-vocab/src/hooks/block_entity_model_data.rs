//! The `block_entity_model_data` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The mod block entity at `pos` hands the renderer its model data
    /// (`IBlockEntityExtension.getModelData`): push a `ModelDataEntry` for each property its
    /// block declared in `register_block_model_data` whose value isn't its default. Asked on
    /// the client's world thread, never on a meshing worker: when the block entity loads, after
    /// `ClientHost::refresh_model_data` (call it when its data changes, such as in
    /// `block_entity_data_received`) and after every resource reload. An answer over 16 KiB, with an unknown or repeated property, a value of
    /// the wrong type, out of range or not finite is refused whole: the defaults are drawn.
    #[hook(
        side = "client",
        contract = "collect",
        out = "data",
        scope = "block_entity",
        key = "block",
        neoforge = "IBlockEntityExtension.getModelData"
    )]
    fn block_entity_model_data();
}
