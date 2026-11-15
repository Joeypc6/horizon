//! Block-entity model data (#861 part 2): typed values a mod block entity hands the renderer,
//! which pick one of a block's declared baked variants for its terrain model.
//!
//! A mod declares, once at client startup, each block's properties (typed, bounded, with
//! defaults) and the variants their values select (`register_block_model_data`). Its block entity
//! answers its current values in `block_entity_model_data`, which the game asks on the client's
//! world thread, never on a meshing worker; the mesher then reads the frozen selection. A block
//! entity whose data changed asks again with `ClientHost::refresh_model_data`.
use crate::Identifier;

/// Most properties one block declares.
pub const MODEL_DATA_MAX_PROPERTIES: usize = 64;
/// Most elements one [`ModelDataValue::Floats`] holds.
pub const MODEL_DATA_MAX_ELEMENTS: usize = 256;
/// Most encoded bytes one block entity's answer takes.
pub const MODEL_DATA_MAX_BYTES: usize = 16 * 1024;
/// Most variants one block declares.
pub const MODEL_DATA_MAX_VARIANTS: usize = 256;

/// One property's value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum ModelDataValue {
    /// A flag.
    Bool(bool),
    /// A whole number within its property's range.
    Int(i32),
    /// Finite numbers, at most the property's length.
    Floats(Vec<f32>),
}

/// One property's value: in an answer to `block_entity_model_data`, or a variant's condition.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub struct ModelDataEntry {
    /// The property's name.
    pub property: String,
    /// Its value.
    pub value: ModelDataValue,
}

impl ModelDataEntry {
    /// Property `property` at `value`.
    pub fn new(property: impl Into<String>, value: ModelDataValue) -> Self {
        Self {
            property: property.into(),
            value,
        }
    }
}

/// What values a property takes.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum ModelDataKind {
    /// [`ModelDataValue::Bool`].
    Bool,
    /// [`ModelDataValue::Int`] from `min` to `max`, both included.
    Int {
        /// Least value.
        min: i32,
        /// Greatest value.
        max: i32,
    },
    /// [`ModelDataValue::Floats`] of at most `max_len` finite numbers, up to
    /// [`MODEL_DATA_MAX_ELEMENTS`]. A variant cannot select on it.
    Floats {
        /// Most elements.
        max_len: u32,
    },
}

/// One typed property a block's model data declares.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub struct ModelDataProperty {
    /// Its name, unique within the block (`lit`, `fill`).
    pub name: String,
    /// The values it takes.
    pub kind: ModelDataKind,
    /// Its value until the block entity answers, and when an answer leaves it out.
    pub default: ModelDataValue,
}

impl ModelDataProperty {
    /// Property `name` of `kind`, `default` until answered.
    pub fn new(name: impl Into<String>, kind: ModelDataKind, default: ModelDataValue) -> Self {
        Self {
            name: name.into(),
            kind,
            default,
        }
    }
}

/// One baked variant a block's model data selects: drawn, in place of the block state's own
/// models, where every `when` property has its value.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub struct ModelDataVariant {
    /// The properties and the values they must have; `Bool` and `Int` properties only, each
    /// at most once.
    pub when: Vec<ModelDataEntry>,
    /// Standalone models (`standalone_models`) drawn in its place, in order, at least one.
    pub models: Vec<Identifier>,
}

impl ModelDataVariant {
    /// Draws `models` where every `when` property has its value.
    pub fn new(when: Vec<ModelDataEntry>, models: Vec<Identifier>) -> Self {
        Self { when, models }
    }
}

/// One block's model data: what its block entity may answer and which variant each answer
/// draws. The first variant whose `when` matches is drawn; none matching keeps the block
/// state's own models.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub struct BlockModelData {
    /// The mod block whose block entities answer.
    pub block: Identifier,
    /// Its properties, at most [`MODEL_DATA_MAX_PROPERTIES`].
    pub properties: Vec<ModelDataProperty>,
    /// Its variants, at most [`MODEL_DATA_MAX_VARIANTS`], tried in order.
    pub variants: Vec<ModelDataVariant>,
}

impl BlockModelData {
    /// Block `block`'s model data.
    pub fn new(
        block: Identifier,
        properties: Vec<ModelDataProperty>,
        variants: Vec<ModelDataVariant>,
    ) -> Self {
        Self {
            block,
            properties,
            variants,
        }
    }
}

/// Why `ClientHost::refresh_model_data` asked nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum ModelDataRefreshError {
    /// No mod block entity with registered model data is loaded at that position.
    NoModelData,
}
