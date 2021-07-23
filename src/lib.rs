use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha::sha1;
use sha::utils::{Digest, DigestExt};

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    InvalidMarioPosition,
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
                "Invalid Super Mario 64 rom: found hash '{}', expected hash '9bef1128717f958171a4afac3ed78ee2bb4e86ce'", hash 
            )
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

pub struct Sm64 {
    texture_data: Vec<u8>,
    rom_data: Vec<u8>,
}

impl Sm64 {
    pub fn new<P: AsRef<Path>>(rom_path: P) -> Result<Self, Error> {
        let mut rom_file = BufReader::new(File::open(rom_path.as_ref())?);
        let mut rom_data = Vec::new();
        rom_file.read_to_end(&mut rom_data)?;

        let rom_hash = sha1::Sha1::default().digest(&*rom_data).to_hex();

        if rom_hash != "9bef1128717f958171a4afac3ed78ee2bb4e86ce" {
            return Err(Error::InvalidRom(rom_hash));
        }

        let mut texture_data =
            vec![
                0;
                (libsm64_sys::SM64_TEXTURE_WIDTH * libsm64_sys::SM64_TEXTURE_HEIGHT) as usize * 3
            ];

        unsafe {
            libsm64_sys::sm64_global_init(rom_data.as_mut_ptr(), texture_data.as_mut_ptr(), None);
        }

        Ok(Self {
            texture_data,
            rom_data,
        })
    }

    pub fn texture(&self) -> Texture<'_> {
        Texture {
            data: &*self.texture_data,
            width: libsm64_sys::SM64_TEXTURE_WIDTH,
            height: libsm64_sys::SM64_TEXTURE_HEIGHT,
        }
    }

    pub fn create_mario<'ctx>(&'ctx self, x: i16, y: i16, z: i16) -> Result<Mario<'ctx>, Error> {
        let mario_id = unsafe { libsm64_sys::sm64_mario_create(x, y, z) };

        if mario_id < 0 {
            Err(Error::InvalidMarioPosition)
        } else {
            Ok(Mario::new(self, mario_id))
        }
    }

    pub fn load_level_geometry(geometry: &[LevelTriangle]) {
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

    pub fn tick(&mut self, input: MarioInput) -> MarioState {
        let input = input.into();
        let mut state = libsm64_sys::SM64MarioState {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            faceAngle: 0.0,
            health: 0,
        };

        unsafe {
            let mut geometry = (&mut self.geometry).into();
            libsm64_sys::sm64_mario_tick(
                self.id as u32,
                &input as *const _,
                &mut state as *mut _,
                &mut geometry as *mut _,
            )
        }

        state.into()
    }
}

impl<'ctx> Drop for Mario<'ctx> {
    fn drop(&mut self) {
        unsafe { libsm64_sys::sm64_mario_delete(self.id) }
    }
}

pub struct Texture<'data> {
    pub data: &'data [u8],
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Point3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Point2 {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[repr(C)]
pub enum Surface {
    Default = 0x0000,
}

#[repr(C)]
pub enum Terrain {
    Grass = 0x0000,
}

#[repr(C)]
pub struct LevelTriangle {
    pub kind: Surface,
    pub force: i16,
    pub terrain: Terrain,
    pub vertices: (Point3, Point3, Point3),
}

#[derive(Copy, Clone, Debug)]
pub struct MarioInput {
    pub cam_look_x: f32,
    pub cam_look_z: f32,
    pub stick_x: f32,
    pub stick_y: f32,
    pub button_a: bool,
    pub button_b: bool,
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

#[derive(Debug, Default, Copy, Clone)]
pub struct MarioState {
    position: Point3,
    velocity: Point3,
    face_angle: f32,
    health: i16,
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

pub struct MarioGeometry {
    position: Vec<Point3>,
    normal: Vec<Point3>,
    color: Vec<Color>,
    uv: Vec<Point2>,
}

impl MarioGeometry {
    fn new() -> Self {
        Self {
            position: vec![Point3::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            normal: vec![Point3::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            color: vec![Color::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
            uv: vec![Point2::default(); libsm64_sys::SM64_GEO_MAX_TRIANGLES as usize * 3],
        }
    }
}

impl<'a> From<&'a mut MarioGeometry> for libsm64_sys::SM64MarioGeometryBuffers {
    fn from(geo: &'a mut MarioGeometry) -> libsm64_sys::SM64MarioGeometryBuffers {
        libsm64_sys::SM64MarioGeometryBuffers {
            position: geo.position.as_mut_ptr() as *mut _,
            normal: geo.normal.as_mut_ptr() as *mut _,
            color: geo.color.as_mut_ptr() as *mut _,
            uv: geo.uv.as_mut_ptr() as *mut _,
            numTrianglesUsed: geo.position.len() as u16 / 32,
        }
    }
}
