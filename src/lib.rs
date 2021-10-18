/*!
This crate provides bindings and a rust friendly wrapper around the C API of [libsm64](https://github.com/libsm64/libsm64).
libsm64 extracts the logic for the movement and control of Mario from the Super Mario
64 ROM providing a interface to implement your own Mario in your own 3D engine.

**Note:** You will be required to provide your own copy of a Super Mario 64 (USA) ROM,
the correct ROM has a SHA1 hash of '9bef1128717f958171a4afac3ed78ee2bb4e86ce'.

# Usage:

```rust
use std::fs::File;
use libsm64::*;

const ROM_PATH: &str = "./baserom.us.z64";
let rom = File::open(ROM_PATH).unwrap();

let sm64 = Sm64::new(rom).unwrap();

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

# fn draw_triangle(_triangle: &(MarioVertex, MarioVertex, MarioVertex), _texture: Texture) {}
# fn create_level_geometry() -> Vec<LevelTriangle> {
# let tri = LevelTriangle {
#    kind: Surface::Default,
#    force: 0,
#    terrain: Terrain::Grass,
#    vertices: (Point3{x: 10, y: -1, z: 10}, Point3{x: 10, y: -1, z: -10}, Point3{x: -10, y: -1, z: -10}),
# };
# let mut level = Vec::new();
# level.push(tri);
# level
# }
```
*/

use std::io::{BufReader, Read};

use sha::sha1;
use sha::utils::{Digest, DigestExt};

const VALID_HASH: &str = "9bef1128717f958171a4afac3ed78ee2bb4e86ce";

/// An error that can occur
#[derive(Debug)]
pub enum Error {
    /// An IO error
    Io(std::io::Error),
    /// When creating Mario he must be positioned above a surface
    InvalidMarioPosition,
    /// The rom proivided must be Super Mario 64 (USA), with a SHA1 hash of '9bef1128717f958171a4afac3ed78ee2bb4e86ce'
    InvalidRom(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "{}", err),
            Error::InvalidMarioPosition => write!(
                f,
                "Invalid Mario position, ensure coordinates are above ground"
            ),
            Error::InvalidRom(hash) => write!(
                f,
                "Invalid Super Mario 64 rom: found hash '{}', expected hash '{}'",
                hash, VALID_HASH
            ),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

/// The core interface to libsm64
pub struct Sm64 {
    texture_data: Vec<u8>,
    rom_data: Vec<u8>,
}

impl Sm64 {
    /// Create a new instance of Sm64, requires a Super Mario 64 rom to extra Mario's texture and animation data from
    pub fn new<R: Read>(rom: R) -> Result<Self, Error> {
        let mut rom_file = BufReader::new(rom);
        let mut rom_data = Vec::new();
        rom_file.read_to_end(&mut rom_data)?;

        let rom_hash = sha1::Sha1::default().digest(&*rom_data).to_hex();

        if rom_hash != VALID_HASH {
            return Err(Error::InvalidRom(rom_hash));
        }

        let mut texture_data =
            vec![
                0;
                (libsm64_sys::SM64_TEXTURE_WIDTH * libsm64_sys::SM64_TEXTURE_HEIGHT) as usize * 4
            ];

        unsafe {
            libsm64_sys::sm64_global_init(rom_data.as_mut_ptr(), texture_data.as_mut_ptr(), None);
        }

        Ok(Self {
            texture_data,
            rom_data,
        })
    }

    /// A texture atlas that can be applied to the Mario geometry
    pub fn texture(&self) -> Texture<'_> {
        Texture {
            data: &*self.texture_data,
            width: libsm64_sys::SM64_TEXTURE_WIDTH,
            height: libsm64_sys::SM64_TEXTURE_HEIGHT,
        }
    }

    /// Create a new instancec of Mario that spawns at the point indicated by x/y/z, he must be placed above a surface or an error will be returned
    pub fn create_mario<'ctx>(&'ctx self, x: i16, y: i16, z: i16) -> Result<Mario<'ctx>, Error> {
        let mario_id = unsafe { libsm64_sys::sm64_mario_create(x, y, z) };

        if mario_id < 0 {
            Err(Error::InvalidMarioPosition)
        } else {
            Ok(Mario::new(self, mario_id))
        }
    }

    /// Create a dynamic surface that can have its position and rotation updated at runtime, good for moving platforms
    pub fn create_dynamic_surface<'ctx>(
        &'ctx self,
        geometry: &[LevelTriangle],
        transform: SurfaceTransform,
    ) -> DynamicSurface<'ctx> {
        let id = unsafe {
            let surface_object = libsm64_sys::SM64SurfaceObject {
                transform: transform.into(),
                surfaceCount: geometry.len() as u32,
                surfaces: geometry.as_ptr() as *mut _,
            };
            libsm64_sys::sm64_surface_object_create(&surface_object as *const _)
        };

        DynamicSurface::new(self, id)
    }

    /// Load the static level geometry, used for collision detection
    pub fn load_level_geometry(&self, geometry: &[LevelTriangle]) {
        unsafe {
            libsm64_sys::sm64_static_surfaces_load(
                geometry.as_ptr() as *const _,
                geometry.len() as u32,
            )
        }
    }
}

impl Drop for Sm64 {
    fn drop(&mut self) {
        unsafe { libsm64_sys::sm64_global_terminate() }
    }
}

/// A instance of Mario that can be controlled
pub struct Mario<'ctx> {
    id: i32,
    geometry: MarioGeometry,
    ctx: &'ctx Sm64,
}

impl<'ctx> Mario<'ctx> {
    fn new(ctx: &'ctx Sm64, id: i32) -> Self {
        let geometry = MarioGeometry::new();
        Self { id, geometry, ctx }
    }

    /// Advance the Mario simulation ahead by 1 frame, should be called 30 times per second
    pub fn tick(&mut self, input: MarioInput) -> MarioState {
        let input = input.into();
        let mut state = libsm64_sys::SM64MarioState {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            faceAngle: 0.0,
            health: 0,
        };

        let tris = unsafe {
            let mut geometry: libsm64_sys::SM64MarioGeometryBuffers = (&mut self.geometry).into();
            libsm64_sys::sm64_mario_tick(
                self.id,
                &input as *const _,
                &mut state as *mut _,
                &mut geometry as *mut _,
            );
            geometry.numTrianglesUsed
        };

        self.geometry.num_triangles = tris as usize;

        state.into()
    }

    /// Mario's geometry as of the current tick
    pub fn geometry(&self) -> &MarioGeometry {
        &self.geometry
    }
}

impl<'ctx> Drop for Mario<'ctx> {
    fn drop(&mut self) {
        unsafe { libsm64_sys::sm64_mario_delete(self.id) }
    }
}

/// A dynamic surface that can have its position and rotation updated at runtime, good for moving platforms
pub struct DynamicSurface<'ctx> {
    id: u32,
    ctx: &'ctx Sm64,
}

impl<'ctx> DynamicSurface<'ctx> {
    fn new(ctx: &'ctx Sm64, id: u32) -> Self {
        Self { id, ctx }
    }

    /// Reposition or rotate the surface
    pub fn transform(&mut self, transform: SurfaceTransform) {
        unsafe {
            let transform = transform.into();
            libsm64_sys::sm64_surface_object_move(self.id, &transform as *const _)
        }
    }
}

impl<'ctx> Drop for DynamicSurface<'ctx> {
    fn drop(&mut self) {
        unsafe { libsm64_sys::sm64_surface_object_delete(self.id) }
    }
}

/// Representions a transform that can be applied to a dynamic surface
#[derive(Copy, Clone, Debug)]
pub struct SurfaceTransform {
    /// The x/y/z coordinates of the surface
    pub position: Point3<f32>,
    /// The rotation of the surface on each axis, the units should be in degrees
    pub euler_rotation: Point3<f32>,
}

impl From<SurfaceTransform> for libsm64_sys::SM64ObjectTransform {
    fn from(transform: SurfaceTransform) -> Self {
        Self {
            position: [
                transform.position.x,
                transform.position.y,
                transform.position.z,
            ],
            eulerRotation: [
                transform.euler_rotation.x,
                transform.euler_rotation.y,
                transform.euler_rotation.z,
            ],
        }
    }
}

/// A texture atlas that can be applied to the Mario geometry
pub struct Texture<'data> {
    /// 8-bit RGBA values
    pub data: &'data [u8],
    /// The width of the texture
    pub width: u32,
    /// The height of the texture
    pub height: u32,
}

/// A point in 3D space
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Point3<T>
where
    T: Copy,
{
    pub x: T,
    pub y: T,
    pub z: T,
}

/// A point in 2D space
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Point2<T>
where
    T: Copy,
{
    pub x: T,
    pub y: T,
}

/// A color
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// A level triangle, the main building block of the collision geometry
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct LevelTriangle {
    /// The type of surface
    pub kind: Surface,
    pub force: i16,
    /// The type of terrain
    pub terrain: Terrain,
    /// The verticies of the triangle. Super Mario 64 using integer math for its collision detection expect to have to scale your vertexes appropriately to use them for level geometry
    ///
    /// **Note:** The order of the verticies is important, Mario will only collide with the front face of the geometry
    pub vertices: (Point3<i16>, Point3<i16>, Point3<i16>),
}

/// The input for a frame of Mario's logic
#[derive(Copy, Clone, Debug, Default)]
pub struct MarioInput {
    ///  The position of the camera on the x-axis, used to adjust the movement of mario based on his postion relative to the camera
    pub cam_look_x: f32,
    ///  The position of the camera on the z-axis, used to adjust the movement of mario based on his postion relative to the camera
    pub cam_look_z: f32,
    /// The input of the analog control on the x-axis (-1.0..1.0)
    pub stick_x: f32,
    /// The input of the analog control on the y-axis (-1.0..1.0)
    pub stick_y: f32,
    /// Is the A button pressed
    pub button_a: bool,
    /// Is the B button pressed
    pub button_b: bool,
    /// Is the Z button pressed
    pub button_z: bool,
}

impl From<MarioInput> for libsm64_sys::SM64MarioInputs {
    fn from(input: MarioInput) -> Self {
        libsm64_sys::SM64MarioInputs {
            camLookX: input.cam_look_x,
            camLookZ: input.cam_look_z,
            stickX: input.stick_x,
            stickY: input.stick_y,
            buttonA: input.button_a as u8,
            buttonB: input.button_b as u8,
            buttonZ: input.button_z as u8,
        }
    }
}

/// Mario's state after a tick of logic
#[derive(Debug, Default, Copy, Clone)]
pub struct MarioState {
    /// The position of Mario in 3D space
    pub position: Point3<f32>,
    /// The velocity of Mario on each axis
    pub velocity: Point3<f32>,
    /// The direction Mario is facing
    pub face_angle: f32,
    /// Mario's current health
    pub health: i16,
}

impl From<libsm64_sys::SM64MarioState> for MarioState {
    fn from(state: libsm64_sys::SM64MarioState) -> Self {
        let position = Point3 {
            x: state.position[0],
            y: state.position[1],
            z: state.position[2],
        };
        let velocity = Point3 {
            x: state.velocity[0],
            y: state.velocity[1],
            z: state.velocity[2],
        };
        MarioState {
            position,
            velocity,
            face_angle: state.faceAngle,
            health: state.health,
        }
    }
}

/// Mario's geometry
pub struct MarioGeometry {
    position: Vec<Point3<f32>>,
    normal: Vec<Point3<f32>>,
    color: Vec<Color>,
    uv: Vec<Point2<f32>>,
    num_triangles: usize,
}

impl MarioGeometry {
    fn new() -> Self {
        Self {
            position: vec![Point3::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            normal: vec![Point3::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            color: vec![Color::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            uv: vec![Point2::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            num_triangles: 0,
        }
    }

    /// The geometry represented as a series of vertices, every 3 verticies is a new triangle. Includes position, normal, color, and texture coordinates
    pub fn vertcies(&self) -> impl Iterator<Item = MarioVertex> + '_ {
        let positions = self.position.iter().copied();
        let normals = self.normal.iter().copied();
        let color = self.color.iter().copied();
        let uv = self.uv.iter().copied();

        positions
            .zip(normals)
            .zip(color)
            .zip(uv)
            .take(self.num_triangles * 3)
            .map(|(((position, normal), color), uv)| MarioVertex {
                position,
                normal,
                color,
                uv,
            })
    }

    /// The geometry represented as a series of triangles. Includes position, normal, color, and texture coordinates
    pub fn triangles(&self) -> impl Iterator<Item = (MarioVertex, MarioVertex, MarioVertex)> + '_ {
        let positions = self.position.chunks_exact(3);
        let normals = self.normal.chunks_exact(3);
        let color = self.color.chunks_exact(3);
        let uv = self.uv.chunks_exact(3);

        positions
            .zip(normals)
            .zip(color)
            .zip(uv)
            .take(self.num_triangles)
            .map(|(((positions, normals), colors), uvs)| {
                let a = MarioVertex {
                    position: positions[0],
                    normal: normals[0],
                    color: colors[0],
                    uv: uvs[0],
                };
                let b = MarioVertex {
                    position: positions[1],
                    normal: normals[1],
                    color: colors[1],
                    uv: uvs[1],
                };
                let c = MarioVertex {
                    position: positions[2],
                    normal: normals[2],
                    color: colors[2],
                    uv: uvs[2],
                };
                (a, b, c)
            })
    }

    /// The position elements of Mario's verticies
    pub fn positions(&self) -> &[Point3<f32>] {
        &self.position[0..self.num_triangles * 3]
    }

    /// The normal elements of Mario's verticies
    pub fn normals(&self) -> &[Point3<f32>] {
        &self.normal[0..self.num_triangles * 3]
    }

    /// The color elements of Mario's verticies
    pub fn colors(&self) -> &[Color] {
        &self.color[0..self.num_triangles * 3]
    }

    /// The texture coordinate elements of Mario's verticies
    pub fn uvs(&self) -> &[Point2<f32>] {
        &self.uv[0..self.num_triangles * 3]
    }
}

impl<'a> From<&'a mut MarioGeometry> for libsm64_sys::SM64MarioGeometryBuffers {
    fn from(geo: &'a mut MarioGeometry) -> libsm64_sys::SM64MarioGeometryBuffers {
        libsm64_sys::SM64MarioGeometryBuffers {
            position: geo.position.as_mut_ptr() as *mut _,
            normal: geo.normal.as_mut_ptr() as *mut _,
            color: geo.color.as_mut_ptr() as *mut _,
            uv: geo.uv.as_mut_ptr() as *mut _,
            numTrianglesUsed: geo.position.len() as u16 / 3,
        }
    }
}

/// A vertex that makes up Mario's model
#[derive(Debug, Copy, Clone)]
pub struct MarioVertex {
    /// The position of the vertex
    pub position: Point3<f32>,
    /// The normal of the vertex
    pub normal: Point3<f32>,
    /// The color of the vertex
    pub color: Color,
    /// The texture coordinate of the vertex
    pub uv: Point2<f32>,
}

/// The surface terrain of a triangle
#[repr(u16)]
#[derive(Copy, Clone, Debug)]
pub enum Terrain {
    Grass = 0x0000,
    Stone = 0x0001,
    Snow = 0x0002,
    Sand = 0x0003,
    Spooky = 0x0004,
    Water = 0x0005,
    Slide = 0x0006,
    Mask = 0x0007,
}

/// The surface type of a triangle
#[repr(u16)]
#[derive(Copy, Clone, Debug)]
pub enum Surface {
    Default = 0x0000,
    Burning = 0x0001,
    _0004 = 0x0004,
    Hangable = 0x0005,
    Slow = 0x0009,
    DeathPlane = 0x000A,
    CloseCamera = 0x000B,
    Water = 0x000D,
    FlowingWater = 0x000E,
    Intangible = 0x0012,
    VerySlippery = 0x0013,
    Slippery = 0x0014,
    NotSlippery = 0x0015,
    TtmVines = 0x0016,
    MgrMusic = 0x001A,
    InstantWarp1b = 0x001B,
    InstantWarp1c = 0x001C,
    InstantWarp1d = 0x001D,
    InstantWarp1e = 0x001E,
    ShallowQuicksand = 0x0021,
    DeepQuicksand = 0x0022,
    InstantQuicksand = 0x0023,
    DeepMovingQuicksand = 0x0024,
    ShallowMovingQuicksand = 0x0025,
    Quicksand = 0x0026,
    MovingQuicksand = 0x0027,
    WallMisc = 0x0028,
    NoiseDefault = 0x0029,
    NoiseSlippery = 0x002A,
    HorizontalWind = 0x002C,
    InstantMovingQuicksand = 0x002D,
    Ice = 0x002E,
    LookUpWarp = 0x002F,
    Hard = 0x0030,
    Warp = 0x0032,
    TimerStart = 0x0033,
    TimerEnd = 0x0034,
    HardSlippery = 0x0035,
    HardVerySlippery = 0x0036,
    HardNotSlippery = 0x0037,
    VerticalWind = 0x0038,
    BossFightCamera = 0x0065,
    CameraFreeRoam = 0x0066,
    Thi3Wallkick = 0x0068,
    CameraPlatform = 0x0069,
    CameraMiddle = 0x006E,
    CameraRotateRight = 0x006F,
    CameraRotateLeft = 0x0070,
    CameraBoundary = 0x0072,
    NoiseVerySlippery73 = 0x0073,
    NoiseVerySlippery74 = 0x0074,
    NoiseVerySlippery = 0x0075,
    NoCamCollision = 0x0076,
    NoCamCollision77 = 0x0077,
    NoCamColVerySlippery = 0x0078,
    NoCamColSlippery = 0x0079,
    Switch = 0x007A,
    VanishCapWalls = 0x007B,
    PaintingWobbleA6 = 0x00A6,
    PaintingWobbleA7 = 0x00A7,
    PaintingWobbleA8 = 0x00A8,
    PaintingWobbleA9 = 0x00A9,
    PaintingWobbleAA = 0x00AA,
    PaintingWobbleAB = 0x00AB,
    PaintingWobbleAC = 0x00AC,
    PaintingWobbleAD = 0x00AD,
    PaintingWobbleAE = 0x00AE,
    PaintingWobbleAF = 0x00AF,
    PaintingWobbleB0 = 0x00B0,
    PaintingWobbleB1 = 0x00B1,
    PaintingWobbleB2 = 0x00B2,
    PaintingWobbleB3 = 0x00B3,
    PaintingWobbleB4 = 0x00B4,
    PaintingWobbleB5 = 0x00B5,
    PaintingWobbleB6 = 0x00B6,
    PaintingWobbleB7 = 0x00B7,
    PaintingWobbleB8 = 0x00B8,
    PaintingWobbleB9 = 0x00B9,
    PaintingWobbleBA = 0x00BA,
    PaintingWobbleBB = 0x00BB,
    PaintingWobbleBC = 0x00BC,
    PaintingWobbleBD = 0x00BD,
    PaintingWobbleBE = 0x00BE,
    PaintingWobbleBF = 0x00BF,
    PaintingWobbleC0 = 0x00C0,
    PaintingWobbleC1 = 0x00C1,
    PaintingWobbleC2 = 0x00C2,
    PaintingWobbleC3 = 0x00C3,
    PaintingWobbleC4 = 0x00C4,
    PaintingWobbleC5 = 0x00C5,
    PaintingWobbleC6 = 0x00C6,
    PaintingWobbleC7 = 0x00C7,
    PaintingWobbleC8 = 0x00C8,
    PaintingWobbleC9 = 0x00C9,
    PaintingWobbleCA = 0x00CA,
    PaintingWobbleCB = 0x00CB,
    PaintingWobbleCC = 0x00CC,
    PaintingWobbleCD = 0x00CD,
    PaintingWobbleCE = 0x00CE,
    PaintingWobbleCF = 0x00CF,
    PaintingWobbleD0 = 0x00D0,
    PaintingWobbleD1 = 0x00D1,
    PaintingWobbleD2 = 0x00D2,
    PaintingWarpD3 = 0x00D3,
    PaintingWarpD4 = 0x00D4,
    PaintingWarpD5 = 0x00D5,
    PaintingWarpD6 = 0x00D6,
    PaintingWarpD7 = 0x00D7,
    PaintingWarpD8 = 0x00D8,
    PaintingWarpD9 = 0x00D9,
    PaintingWarpDA = 0x00DA,
    PaintingWarpDB = 0x00DB,
    PaintingWarpDC = 0x00DC,
    PaintingWarpDD = 0x00DD,
    PaintingWarpDE = 0x00DE,
    PaintingWarpDF = 0x00DF,
    PaintingWarpE0 = 0x00E0,
    PaintingWarpE1 = 0x00E1,
    PaintingWarpE2 = 0x00E2,
    PaintingWarpE3 = 0x00E3,
    PaintingWarpE4 = 0x00E4,
    PaintingWarpE5 = 0x00E5,
    PaintingWarpE6 = 0x00E6,
    PaintingWarpE7 = 0x00E7,
    PaintingWarpE8 = 0x00E8,
    PaintingWarpE9 = 0x00E9,
    PaintingWarpEA = 0x00EA,
    PaintingWarpEB = 0x00EB,
    PaintingWarpEC = 0x00EC,
    PaintingWarpED = 0x00ED,
    PaintingWarpEE = 0x00EE,
    PaintingWarpEF = 0x00EF,
    PaintingWarpF0 = 0x00F0,
    PaintingWarpF1 = 0x00F1,
    PaintingWarpF2 = 0x00F2,
    PaintingWarpF3 = 0x00F3,
    TtcPainting1 = 0x00F4,
    TtcPainting2 = 0x00F5,
    TtcPainting3 = 0x00F6,
    PaintingWarpF7 = 0x00F7,
    PaintingWarpF8 = 0x00F8,
    PaintingWarpF9 = 0x00F9,
    PaintingWarpFA = 0x00FA,
    PaintingWarpFB = 0x00FB,
    PaintingWarpFC = 0x00FC,
    WobblingWarp = 0x00FD,
    Trapdoor = 0x00FF,
}

#[test]
fn basic_loading() {
    let rom = std::env::var("SM64_ROM_PATH")
        .expect("Path to SM64 rom must be proivided in 'SM64_ROM_PATH' env var");
    let rom = std::fs::File::open(rom).unwrap();
    let sm64 = Sm64::new(rom).unwrap();
    let mario = sm64.create_mario(1, 2, 3);

    match mario {
        Err(Error::InvalidMarioPosition) => (),
        _ => panic!("Expected InvalidMarioPosition error"),
    }
}

#[test]
fn correct_repr() {
    assert_eq!(
        std::mem::size_of::<LevelTriangle>(),
        std::mem::size_of::<libsm64_sys::SM64Surface>()
    );

    let tri = LevelTriangle {
        kind: Surface::Default,
        force: 333,
        terrain: Terrain::Grass,
        vertices: (
            Point3 { x: 1, y: 2, z: 3 },
            Point3 { x: 4, y: 5, z: 6 },
            Point3 { x: 7, y: 8, z: 9 },
        ),
    };

    let c_tri = libsm64_sys::SM64Surface {
        type_: 0,
        force: 333,
        terrain: 0,
        vertices: [[1, 2, 3], [4, 5, 6], [7, 8, 9]],
    };

    let my_c_tri = unsafe { std::mem::transmute::<_, libsm64_sys::SM64Surface>(tri) };

    assert_eq!(c_tri.type_, my_c_tri.type_);
    assert_eq!(c_tri.force, my_c_tri.force);
    assert_eq!(c_tri.terrain, my_c_tri.terrain);
    assert_eq!(c_tri.vertices, my_c_tri.vertices);
}
