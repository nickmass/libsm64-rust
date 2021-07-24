use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha::sha1;
use sha::utils::{Digest, DigestExt};

const VALID_HASH: &str = "9bef1128717f958171a4afac3ed78ee2bb4e86ce";

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

    pub fn geometry(&self) -> &MarioGeometry {
        &self.geometry
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
pub struct Point3<T>
where
    T: Copy,
{
    pub x: T,
    pub y: T,
    pub z: T,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Point2<T>
where
    T: Copy,
{
    pub x: T,
    pub y: T,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[repr(u16)]
#[derive(Copy, Clone, Debug)]
pub enum Surface {
    Default = 0x0000,
}

#[repr(u16)]
#[derive(Copy, Clone, Debug)]
pub enum Terrain {
    Grass = 0x0000,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct LevelTriangle {
    pub kind: Surface,
    pub force: i16,
    pub terrain: Terrain,
    pub vertices: (Point3<i16>, Point3<i16>, Point3<i16>),
}

#[derive(Copy, Clone, Debug, Default)]
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
    pub position: Point3<f32>,
    pub velocity: Point3<f32>,
    pub face_angle: f32,
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

    pub fn vertcies(&self) -> impl Iterator<Item = MarioVertex> + '_ {
        let positions = self.position.iter().copied();
        let normals = self.normal.iter().copied();
        let color = self.color.iter().copied();
        let uv = self.uv.iter().copied();

        positions
            .zip(normals)
            .zip(color)
            .zip(uv)
            .take(self.num_triangles / 3)
            .map(|(((position, normal), color), uv)| MarioVertex {
                position,
                normal,
                color,
                uv,
            })
    }

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

#[derive(Debug, Copy, Clone)]
pub struct MarioVertex {
    pub position: Point3<f32>,
    pub normal: Point3<f32>,
    pub color: Color,
    pub uv: Point2<f32>,
}

#[test]
fn basic_loading() {
    let rom = std::env::var("SM64_ROM_PATH")
        .expect("Path to SM64 rom must be proivided in 'SM64_ROM_PATH' env var");
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
