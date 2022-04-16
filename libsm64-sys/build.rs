use std::env;
use std::path::PathBuf;
use std::process::Command;

const C_FILES: &[&str] = &[
    "libsm64/src/debug_print.c",
    "libsm64/src/decomp/engine/geo_layout.c",
    "libsm64/src/decomp/engine/graph_node.c",
    "libsm64/src/decomp/engine/graph_node_manager.c",
    "libsm64/src/decomp/engine/guMtxF2L.c",
    "libsm64/src/decomp/engine/math_util.c",
    "libsm64/src/decomp/engine/surface_collision.c",
    "libsm64/src/decomp/game/behavior_actions.c",
    "libsm64/src/decomp/game/interaction.c",
    "libsm64/src/decomp/game/mario_actions_airborne.c",
    "libsm64/src/decomp/game/mario_actions_automatic.c",
    "libsm64/src/decomp/game/mario_actions_cutscene.c",
    "libsm64/src/decomp/game/mario_actions_moving.c",
    "libsm64/src/decomp/game/mario_actions_object.c",
    "libsm64/src/decomp/game/mario_actions_stationary.c",
    "libsm64/src/decomp/game/mario_actions_submerged.c",
    "libsm64/src/decomp/game/mario.c",
    "libsm64/src/decomp/game/mario_misc.c",
    "libsm64/src/decomp/game/mario_step.c",
    "libsm64/src/decomp/game/object_stuff.c",
    "libsm64/src/decomp/game/platform_displacement.c",
    "libsm64/src/decomp/game/rendering_graph_node.c",
    "libsm64/src/decomp/global_state.c",
    "libsm64/src/decomp/mario/geo.inc.c",
    "libsm64/src/decomp/mario/model.inc.c",
    "libsm64/src/decomp/memory.c",
    "libsm64/src/decomp/tools/libmio0.c",
    "libsm64/src/decomp/tools/n64graphics.c",
    "libsm64/src/decomp/tools/utils.c",
    "libsm64/src/gfx_adapter.c",
    "libsm64/src/libsm64.c",
    "libsm64/src/load_anim_data.c",
    "libsm64/src/load_surfaces.c",
    "libsm64/src/load_tex_data.c",
    "libsm64/src/obj_pool.c",
];

const MARIO_GEO: &str = "libsm64/src/decomp/mario/geo.inc.c";

fn main() {
    if !PathBuf::from(MARIO_GEO).exists() {
        Command::new("python3")
            .arg("import-mario-geo.py")
            .current_dir("libsm64")
            .output()
            .expect("Unable to download mario geometry");
    }

    cc::Build::new()
        .files(C_FILES)
        .warnings(false)
        .compile("sm64");

    let bindings = bindgen::Builder::default()
        .header("libsm64/src/libsm64.h")
        .generate()
        .expect("Unable to generate libsm64 bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Could not write C bindings");
}
