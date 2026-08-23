//! `mapgeom` — a TM2020 map as real 3D geometry.
//!
//! Every earlier geometry result in this project was *inferred*: a deck height
//! from plumb probes, a route from a block graph, "the ice IS the road" from
//! deleting caps and watching what broke. This crate reads the shapes the game
//! itself collides against, out of the game's own data pack, and places them at
//! the map's own coordinates.
//!
//! The chain, end to end:
//!
//! ```text
//!   dedicated_TMStadium.pak        NadeoPak v18, Blowfish header + LZ4 payloads
//!     -> <Model>.EDClassic.Gbx     the block model; its REFERENCE TABLE names
//!                                  the prefab files that carry its shape
//!     -> <Name>.Prefab.Gbx         stored under a 34-hex name (`names.rs`)
//!        -> CPlugPrefab            entities, each with a position + rotation
//!        -> CPlugStaticObjectModel mesh + collision shape
//!        -> CPlugSurface           TRIANGLES, with a physics material per face
//!        -> CPlugSolid2Model       the visual mesh, via vertex streams
//!     -> .Map.Gbx                  blocks and items (read by `tmmaps`)
//!        -> world placement        `place.rs`
//!        -> glTF / OBJ             `scene.rs`
//! ```

pub mod assemble;
pub mod blame;
pub mod blowfish;
pub mod classes;
pub mod container;
pub mod corpus;
pub mod coverage;
pub mod embedded;
pub mod geom;
pub mod lz4dict;
pub mod md5;
pub mod names;
pub mod node;
pub mod pak;
pub mod pakfile;
pub mod place;
pub mod probe;
pub mod reader;
pub mod render;
pub mod scene;
pub mod store;
pub mod tables;

pub use store::{DataStore, Model};
