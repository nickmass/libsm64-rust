use std::env;
use std::path::PathBuf;
use std::process::Command;

const C_FILES: &[&str] = &[
    "libsm64/src/debug_print.c",
    "libsm64/src/decomp/audio/copt/seq_channel_layer_process_script_copt.inc.c",
    "libsm64/src/decomp/audio/data.c",
    "libsm64/src/decomp/audio/effects.c",
    "libsm64/src/decomp/audio/external.c",
    "libsm64/src/decomp/audio/globals_start.c",
    "libsm64/src/decomp/audio/heap.c",
    "libsm64/src/decomp/audio/load.c",
    "libsm64/src/decomp/audio/load_dat.c",
    "libsm64/src/decomp/audio/load_sh.c",
    "libsm64/src/decomp/audio/playback.c",
    "libsm64/src/decomp/audio/port_eu.c",
    "libsm64/src/decomp/audio/port_sh.c",
    "libsm64/src/decomp/audio/seqplayer.c",
    "libsm64/src/decomp/audio/shindou_debug_prints.c",
    "libsm64/src/decomp/audio/synthesis.c",
    "libsm64/src/decomp/audio/synthesis_sh.c",
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
    "libsm64/src/decomp/game/sound_init.c",
    "libsm64/src/decomp/global_state.c",
    "libsm64/src/decomp/mario/geo.inc.c",
    "libsm64/src/decomp/mario/model.inc.c",
    "libsm64/src/decomp/memory.c",
    "libsm64/src/decomp/pc/alBnkfNew.c",
    "libsm64/src/decomp/pc/mixer.c",
    "libsm64/src/decomp/pc/ultra_reimplementation.c",
    "libsm64/src/decomp/tools/convUtils.c",
    "libsm64/src/decomp/tools/libmio0.c",
    "libsm64/src/decomp/tools/n64graphics.c",
    "libsm64/src/decomp/tools/utils.c",
    "libsm64/src/fake_interaction.c",
    "libsm64/src/gfx_adapter.c",
    "libsm64/src/libsm64.c",
    "libsm64/src/load_anim_data.c",
    "libsm64/src/load_audio_data.c",
    "libsm64/src/load_surfaces.c",
    "libsm64/src/load_tex_data.c",
    "libsm64/src/obj_pool.c",
    "libsm64/src/play_sound.c",
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
        .flags(["-fno-strict-aliasing", "-fPIC", "-fvisibility=hidden"])
        .warnings(false)
        .define("GBI_FLOATS", None)
        .define("VERSION_US", None)
        .define("NO_SEGMENTED_MEMORY", None)
        .files(C_FILES)
        .include("libsm64/src/decomp/include")
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
