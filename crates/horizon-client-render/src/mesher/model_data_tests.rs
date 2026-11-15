//! Block-entity model data in the real mesher (#861 part 2): no plan meshes exactly as today,
//! and a plan's selection replaces only the blocks it names. Needs prepared resources, like the
//! other mesher geometry tests.
use super::*;
use crate::{
    assets::{Assets, RenderAssets},
    biome::Biomes,
    blocks::BlockFacts,
    chunk::Chunk,
    model_data::{Entry, ModelDataSections},
    models,
};
use std::sync::Arc;

struct Fixture {
    assets: Assets,
    biomes: Biomes,
    render: RenderAssets,
    visibility: horizon_world::block_render::FaceVisibility,
    tables: MeshTables,
}

fn fixture() -> Fixture {
    let root = std::env::var_os("HORIZON_MESH_TEST_RESOURCES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../.local/horizon-client/26.3")
        });
    let assets = Assets::load(&root).unwrap();
    let facts = BlockFacts::load(&root, assets.registry).unwrap();
    let resources = horizon_assets::resource_source::ResourceRoot::directory(root.clone());
    let biomes = Biomes::load(&resources).unwrap();
    let render = assets.reload(&resources).unwrap();
    let visibility = models::load_visibility(&root, assets.registry).unwrap();
    let tables = MeshTables::new(&assets, &render, &facts, visibility.clone(), None).unwrap();
    Fixture {
        assets,
        biomes,
        render,
        visibility,
        tables,
    }
}

fn state(f: &Fixture, name: &str) -> u32 {
    f.assets.registry.default_state(name).unwrap().0
}

fn mesh(
    f: &Fixture,
    chunk: &Chunk,
    plan: Option<&crate::model_data::ModelDataPlan>,
) -> SectionMesh {
    let region = [[Some(chunk); 3]; 3];
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &f.assets,
        render: &f.render,
        tables: &f.tables,
        biomes: &f.biomes,
        visibility: f.visibility.clone(),
        zoom: BiomeZoom::new(0),
        blend_radius: 0,
        smooth_lighting: true,
        cutout_leaves: true,
        opaque_ice: false,
        translucent_layers: &[],
        leaf_culling: LeafCullingMode::Off,
        hidden_states: &[],
    };
    mesher.mesh_planned(&region, 0, SectionMesh::default(), &mut MeshScratch::default(), plan)
}

fn bytes(vertices: &[TerrainVertex]) -> &[u8] {
    bytemuck::cast_slice(vertices)
}

const BASE: i32 = MIN_Y + 4;

fn chunk(f: &Fixture) -> Chunk {
    let mut chunk = Chunk::empty(0, 0);
    chunk.set(4, BASE, 4, state(f, "minecraft:stone"));
    chunk.set(8, BASE, 8, state(f, "minecraft:stone"));
    chunk.set(12, BASE, 4, state(f, "minecraft:glass"));
    chunk
}

#[test]
#[ignore = "daily"]
fn no_plan_meshes_byte_for_byte_as_the_mesher_always_has() {
    let f = fixture();
    let chunk = chunk(&f);
    let region = [[Some(&chunk); 3]; 3];
    let mut mesher = Mesher {
        nether_shading: false,
        shader_pack_lighting: false,
        separate_ao: false,
        shader_pack: false,
        assets: &f.assets,
        render: &f.render,
        tables: &f.tables,
        biomes: &f.biomes,
        visibility: f.visibility.clone(),
        zoom: BiomeZoom::new(0),
        blend_radius: 0,
        smooth_lighting: true,
        cutout_leaves: true,
        opaque_ice: false,
        translucent_layers: &[],
        leaf_culling: LeafCullingMode::Off,
        hidden_states: &[],
    };
    let today = mesher.mesh(&region, 0);
    let planned = mesh(&f, &chunk, None);
    assert_eq!(bytes(&today.solid), bytes(&planned.solid));
    assert_eq!(bytes(&today.translucent), bytes(&planned.translucent));
    // An empty table gives no plan at all.
    assert!(ModelDataSections::default().plan(0, 0, 0).is_none());
}

#[test]
#[ignore = "daily"]
fn a_selection_replaces_only_the_block_it_names() {
    let f = fixture();
    let chunk = chunk(&f);
    let dirt = f
        .render
        .model_catalog
        .state_slots(horizon_world::registry::BlockStateId(state(&f, "minecraft:dirt")))
        .unwrap();
    let dirt_layer = f.render.model_catalog.model_by_slot(dirt[0]).unwrap().faces[0].layer;
    let stone_layer = f.render.sprite_layers["minecraft:block/stone"];
    let mut table = ModelDataSections::default();
    table.reset(f.render.generation, Arc::from([]));
    // The stone at (8, BASE, 8) draws dirt's model; the one at (4, BASE, 4) stays stone.
    let sy = (BASE - MIN_Y) >> 4;
    assert!(table.set([8, BASE, 8], sy, Entry::Selected(Some(Arc::from(&dirt[..])))));
    let plan = table.plan(0, sy, 0).unwrap();
    let planned = mesh(&f, &chunk, Some(&plan));
    let today = mesh(&f, &chunk, None);
    assert_eq!(planned.solid.len(), today.solid.len(), "a cube for a cube");
    let layers = |mesh: &SectionMesh| -> Vec<u32> {
        mesh.solid
            .iter()
            .map(|v| u32::from(v.layer & !OPAQUE_LAYER))
            .collect()
    };
    let (planned, today) = (layers(&planned), layers(&today));
    assert_eq!(today.iter().filter(|l| **l == dirt_layer).count(), 0);
    assert_eq!(planned.iter().filter(|l| **l == dirt_layer).count(), 6 * 4);
    assert_eq!(planned.iter().filter(|l| **l == stone_layer).count(), 6 * 4);
}
