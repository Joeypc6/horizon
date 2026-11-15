//! Checking block-entity model data (#861 part 2): the registrations once at client start, each
//! answer before it selects anything. Pure functions over API values, shared by native and wasm
//! mods since both reach the game as the same values. A refusal names its owner and is whole: a
//! registration refused draws its block's own models, an answer refused its declared defaults.
use mod_api::{
    BlockModelData, MODEL_DATA_MAX_BYTES, MODEL_DATA_MAX_ELEMENTS, MODEL_DATA_MAX_PROPERTIES,
    MODEL_DATA_MAX_VARIANTS, ModelDataEntry, ModelDataKind, ModelDataValue,
};

/// One property of a checked registration.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Property {
    pub name: String,
    pub kind: ModelDataKind,
    pub default: ModelDataValue,
}

/// One variant of a checked registration: the property indexes and values it needs, the
/// standalone model ids it draws.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Variant {
    pub when: Vec<(usize, ModelDataValue)>,
    pub models: Vec<String>,
}

/// One block's checked model data.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct Registration {
    pub owner: &'static str,
    pub block: String,
    pub properties: Vec<Property>,
    pub variants: Vec<Variant>,
}

impl Registration {
    /// Every property's default, in declaration order.
    pub fn defaults(&self) -> Vec<ModelDataValue> {
        self.properties.iter().map(|p| p.default.clone()).collect()
    }

    /// The first variant whose every condition `values` meets.
    pub fn select(&self, values: &[ModelDataValue]) -> Option<usize> {
        self.variants.iter().position(|variant| {
            variant
                .when
                .iter()
                .all(|(property, value)| values.get(*property) == Some(value))
        })
    }

    /// The values an answer gives, its properties left out keeping their defaults; `Err` names
    /// why the whole answer is refused.
    pub fn values(&self, answer: &[ModelDataEntry]) -> Result<Vec<ModelDataValue>, String> {
        let mut out = mod_api::wire::Writer::new();
        out.put(&answer.to_vec());
        if out.bytes().len() > MODEL_DATA_MAX_BYTES {
            return Err(format!(
                "{} bytes, over {MODEL_DATA_MAX_BYTES}",
                out.bytes().len()
            ));
        }
        let mut values = self.defaults();
        let mut seen = vec![false; values.len()];
        for entry in answer {
            let Some(index) = self.properties.iter().position(|p| p.name == entry.property) else {
                return Err(format!("unknown property `{}`", entry.property));
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(format!("property `{}` given twice", entry.property));
            }
            fits(&self.properties[index].kind, &entry.value)
                .map_err(|why| format!("property `{}`: {why}", entry.property))?;
            values[index] = entry.value.clone();
        }
        Ok(values)
    }
}

/// Whether `value` is one `kind` takes; `Err` says why not.
fn fits(kind: &ModelDataKind, value: &ModelDataValue) -> Result<(), String> {
    match (kind, value) {
        (ModelDataKind::Bool, ModelDataValue::Bool(_)) => Ok(()),
        (ModelDataKind::Int { min, max }, ModelDataValue::Int(v)) => {
            if (*min..=*max).contains(v) {
                Ok(())
            } else {
                Err(format!("{v} outside {min}..={max}"))
            }
        }
        (ModelDataKind::Floats { max_len }, ModelDataValue::Floats(v)) => {
            if v.len() > *max_len as usize {
                Err(format!("{} elements, over {max_len}", v.len()))
            } else if v.iter().any(|f| !f.is_finite()) {
                Err("a number that isn't finite".into())
            } else {
                Ok(())
            }
        }
        (ModelDataKind::Bool | ModelDataKind::Int { .. } | ModelDataKind::Floats { .. }, _) => {
            Err("a value of another type".into())
        }
        _ => Err("an unsupported property type".into()),
    }
}

/// The registrations kept, each checked whole; refused ones are logged with their owner.
/// `known_block` says whether a block id names a block of the registry.
pub(in crate::game) fn check(
    entries: Vec<mod_api::Answered<BlockModelData>>,
    known_block: impl Fn(&str) -> bool,
) -> Vec<Registration> {
    let mut kept: Vec<Registration> = Vec::new();
    let mut conflicted = std::collections::BTreeSet::new();
    for entry in entries {
        let owner = entry.mod_id;
        let block = entry.answer.block.to_string();
        let checked = if known_block(&block) {
            registration(owner, entry.answer)
        } else {
            Err("E_MODEL_DATA_UNKNOWN_BLOCK: no such block".into())
        };
        match checked {
            Ok(registration) => {
                if let Some(other) = kept.iter().find(|r| r.block == registration.block) {
                    log::warn!(
                        "Horizon: E_MODEL_DATA_CONFLICT: mods {} and {owner} both declare model \
                         data for {block}; neither is used",
                        other.owner
                    );
                    conflicted.insert(registration.block);
                } else {
                    kept.push(registration);
                }
            }
            Err(why) => log::warn!("Horizon: mod {owner}'s model data for {block} refused: {why}"),
        }
    }
    kept.retain(|r| !conflicted.contains(&r.block));
    kept
}

/// One registration checked whole.
fn registration(owner: &'static str, data: BlockModelData) -> Result<Registration, String> {
    if data.properties.len() > MODEL_DATA_MAX_PROPERTIES {
        return Err(format!(
            "E_MODEL_DATA_LIMIT: {} properties, over {MODEL_DATA_MAX_PROPERTIES}",
            data.properties.len()
        ));
    }
    if data.variants.len() > MODEL_DATA_MAX_VARIANTS {
        return Err(format!(
            "E_MODEL_DATA_LIMIT: {} variants, over {MODEL_DATA_MAX_VARIANTS}",
            data.variants.len()
        ));
    }
    let mut properties: Vec<Property> = Vec::new();
    for property in data.properties {
        if properties.iter().any(|p| p.name == property.name) {
            return Err(format!(
                "E_MODEL_DATA_SCHEMA: property `{}` declared twice",
                property.name
            ));
        }
        match &property.kind {
            ModelDataKind::Int { min, max } if min > max => {
                return Err(format!(
                    "E_MODEL_DATA_SCHEMA: property `{}` has min {min} over max {max}",
                    property.name
                ));
            }
            ModelDataKind::Floats { max_len } if *max_len as usize > MODEL_DATA_MAX_ELEMENTS => {
                return Err(format!(
                    "E_MODEL_DATA_LIMIT: property `{}` takes {max_len} elements, over \
                     {MODEL_DATA_MAX_ELEMENTS}",
                    property.name
                ));
            }
            _ => {}
        }
        fits(&property.kind, &property.default).map_err(|why| {
            format!(
                "E_MODEL_DATA_SCHEMA: property `{}`'s default: {why}",
                property.name
            )
        })?;
        properties.push(Property {
            name: property.name,
            kind: property.kind,
            default: property.default,
        });
    }
    let mut variants = Vec::new();
    for variant in data.variants {
        if variant.models.is_empty() {
            return Err("E_MODEL_DATA_SCHEMA: a variant draws no model".into());
        }
        let mut when: Vec<(usize, ModelDataValue)> = Vec::new();
        for ModelDataEntry {
            property: name,
            value,
            ..
        } in variant.when
        {
            let Some(index) = properties.iter().position(|p| p.name == name) else {
                return Err(format!(
                    "E_MODEL_DATA_SCHEMA: a variant names unknown property `{name}`"
                ));
            };
            if matches!(properties[index].kind, ModelDataKind::Floats { .. }) {
                return Err(format!(
                    "E_MODEL_DATA_SCHEMA: a variant selects on float property `{name}`"
                ));
            }
            if when.iter().any(|(i, _)| *i == index) {
                return Err(format!(
                    "E_MODEL_DATA_SCHEMA: a variant names property `{name}` twice"
                ));
            }
            fits(&properties[index].kind, &value)
                .map_err(|why| format!("E_MODEL_DATA_SCHEMA: a variant's `{name}`: {why}"))?;
            when.push((index, value));
        }
        variants.push(Variant {
            when,
            models: variant.models.iter().map(ToString::to_string).collect(),
        });
    }
    Ok(Registration {
        owner,
        block: data.block.to_string(),
        properties,
        variants,
    })
}
