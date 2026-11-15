//! The `register_block_model_data` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// Block-entity model data (#861): push a `BlockModelData` for each mod block whose block
    /// entity hands the renderer typed values, with its properties (typed, bounded, with
    /// defaults) and the baked standalone-model variants those values select. Asked once on the
    /// client before the first resource load, never on a server. A registration that breaks a
    /// limit, names an unknown block or property, or that another mod also makes for the same
    /// block is refused whole, logged with its mod; its block draws its own models.
    #[notes = r#"## Block-entity model data

`register_block_model_data` (the registration half of NeoForge's `ModelData`) declares, once on the client, what a mod
block's block entity may tell the renderer: up to 64 typed properties (`Bool`, `Int` in a range, or up to 256 finite
`Floats`), each with a default, and up to 256 variants. A variant names `Bool`/`Int` values and the standalone models
(`standalone_models`) drawn instead of the block state's own where they hold; the first that matches wins and none
matching keeps the state's models. The block entity answers its values in `block_entity_model_data`, which the game
asks on the client's world thread, and asks again after `ClientHost::refresh_model_data`. Meshing workers read the
frozen selection and never call a mod."#]
    #[hook(
        side = "client",
        contract = "collect",
        out = "entries",
        answered_by = "mod",
        context = "registration",
        neoforge = "none (ModelData is supplied per block entity; its schema has no registration event)"
    )]
    fn register_block_model_data();
}
