# Block-entity model data

A mod block entity can tell the renderer what its block looks like: a lamp that is lit, a pot
that holds soup, a cable with a facade. It hands over typed values, the *model data*, and they
pick which baked models the block's terrain mesh draws. This is part 2 of #861 (the
[design](https://github.com/arismoko/horizon/blob/main/docs/design/mod-api/baked-models.md)),
NeoForge's `ModelData` and `requestModelDataUpdate`.

## Declaring it

Once on the client, before the first resource load, a mod declares each block's properties and
the variants they select in `register_block_model_data`:

```rust,ignore
fn register_block_model_data(
    &self,
    _: &RegistrationContext,
    event: &mut RegisterBlockModelDataEvent<'_>,
) {
    let lit = |on| vec![ModelDataEntry::new("lit", ModelDataValue::Bool(on))];
    event.entries.push(BlockModelData::new(
        Identifier::parse("lamps:lamp").unwrap(),
        vec![ModelDataProperty::new("lit", ModelDataKind::Bool, ModelDataValue::Bool(false))],
        vec![
            ModelDataVariant::new(lit(false), vec![Identifier::parse("lamps:block/lamp_off").unwrap()]),
            ModelDataVariant::new(lit(true), vec![Identifier::parse("lamps:block/lamp_on").unwrap()]),
        ],
    ));
}
```

A variant's models are standalone models (`standalone_models`), baked with every resource load.
Where a variant's conditions hold, its models are drawn instead of the block state's own; the first
variant that matches wins, and none matching keeps the state's own models.

| Limit | Value |
| --- | --- |
| Properties per block | 64 (`MODEL_DATA_MAX_PROPERTIES`) |
| Elements of a `Floats` value | 256 (`MODEL_DATA_MAX_ELEMENTS`) |
| Variants per block | 256 (`MODEL_DATA_MAX_VARIANTS`) |
| One answer, encoded | 16 KiB (`MODEL_DATA_MAX_BYTES`) |

A property is `Bool`, `Int { min, max }` or `Floats { max_len }`, with a default of its own type.
Variants select on `Bool` and `Int` properties only. A registration that breaks a limit, repeats
a property, names an unknown block or property, or that two mods make for the same block is
refused whole, logged with the mod (`E_MODEL_DATA_*`): its block draws its own models.

## Answering it

The block entity's Part answers `block_entity_model_data` with the values that aren't defaults:

```rust,ignore
fn block_entity_model_data(&self, event: &mut BlockEntityModelDataEvent<'_>) {
    if self.lit {
        event.data.push(ModelDataEntry::new("lit", ModelDataValue::Bool(true)));
    }
}
```

The game asks on the client's own world thread, never on a meshing worker: when the block entity
loads, after `ClientHost::refresh_model_data(pos)`, and after every resource reload. Until it has
answered, the defaults are drawn. The answer is frozen into the section's mesh plan; the meshing
workers only read it.

Call `refresh_model_data` when the data changes, for example from `block_entity_data_received`.
Requests are gathered and asked once each before the next sections mesh; only a section whose
selection changed is meshed again. `ModelDataRefreshError::NoModelData` means no mod block entity
with registered model data is loaded there.

An answer that is over 16 KiB, names an unknown property or one twice, or gives a value of the
wrong type, out of its range or not finite is refused whole (`E_MODEL_DATA_REFUSED`, once per
block per load): the defaults are drawn. A provider that panics is turned off until the next
resource load (`E_MODEL_DATA_PANIC`).

With no mod declaring model data nothing is asked, kept or queued, and meshes are those of
today. Native and wasm mods share these types and checks.
