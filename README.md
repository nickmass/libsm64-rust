# libsm64 - Rust Bindings

[Documentation](https://nickmass.com/doc/libsm64/index.html)

This is a thin layer of rust bindings over the very excellent [libsm64 project](https://github.com/libsm64/libsm64).

## Usage

```rust
use std::fs::File;
use libsm64::*;

const ROM_PATH: &str = "./baserom.us.z64";
let rom = File::open(ROM_PATH).unwrap();

let mut sm64 = Sm64::new(rom).unwrap();

// Convert your existing level geometry into LevelTriangles
let level_collision_geometry = create_level_geometry();

// Load the geometry into sm64 to be used for collision detection
sm64.load_level_geometry(&level_collision_geometry);

// Create a new Mario and provide his starting position
let mut mario = sm64.create_mario(0, 0, 0).unwrap();

let input = MarioInput {
    stick_x: 0.5,
    button_a: true,
    ..MarioInput::default()
};

// For each iteration of your gameloop, tick Mario's state
let state = mario.tick(input);

println!("Mario's current health: {}", state.health);

// Mario's geometry will be updated to his new position and animation
for triangle in mario.geometry().triangles() {
    draw_triangle(&triangle, sm64.texture());
}
```
