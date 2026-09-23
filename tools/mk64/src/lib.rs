//! Mario Kart 64 courses → Trackmania 2020 maps.
//!
//! `cdata` reads the decomp's C data files, `course` holds one course and
//! walks its display lists, `texture` reads the ROM's textures, `mesh` puts
//! the geometry in Trackmania's frame, `render` draws it from above, `tm`
//! writes the items and the map.

pub mod cdata;
pub mod course;
pub mod mesh;
pub mod render;
pub mod texture;
pub mod tm;
