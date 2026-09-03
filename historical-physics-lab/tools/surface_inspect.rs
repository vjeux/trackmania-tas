use std::{env, fs};

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn u16(&mut self) -> u16 {
        let v = u16::from_le_bytes(self.bytes[self.offset..self.offset + 2].try_into().unwrap());
        self.offset += 2;
        v
    }
    fn u32(&mut self) -> u32 {
        let v = u32::from_le_bytes(self.bytes[self.offset..self.offset + 4].try_into().unwrap());
        self.offset += 4;
        v
    }
    fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }
    fn vec3(&mut self) -> [f32; 3] {
        [self.f32(), self.f32(), self.f32()]
    }
}

fn read_surf(reader: &mut Reader<'_>, version: i32, depth: usize) {
    let start = reader.offset;
    let kind = reader.i32();
    let indent = "  ".repeat(depth);
    match kind {
        0 => {
            let size = reader.f32();
            let index = if version >= 1 { reader.u16() } else { 0 };
            let main = if version >= 2 {
                reader.vec3()
            } else {
                [0.0; 3]
            };
            println!(
                "{indent}offset=0x{start:X} Sphere radius={size:.9} surface={index} main={main:?}"
            );
        }
        1 => {
            let size = reader.vec3();
            let index = if version >= 1 { reader.u16() } else { 0 };
            let main = if version >= 2 {
                reader.vec3()
            } else {
                [0.0; 3]
            };
            println!(
                "{indent}offset=0x{start:X} Ellipsoid size={size:?} surface={index} main={main:?}"
            );
        }
        6 => {
            let transform = [
                reader.f32(),
                reader.f32(),
                reader.f32(),
                reader.f32(),
                reader.f32(),
                reader.f32(),
            ];
            let index = if version >= 1 { reader.u16() } else { 0 };
            let main = if version >= 2 {
                reader.vec3()
            } else {
                [0.0; 3]
            };
            println!("{indent}offset=0x{start:X} Box transform={transform:?} surface={index} main={main:?}");
        }
        13 => {
            let count = reader.i32();
            println!("{indent}offset=0x{start:X} Compound count={count}");
            for _ in 0..count {
                read_surf(reader, version, depth + 1);
            }
        }
        _ => panic!("unsupported surface kind {kind} at 0x{start:X}"),
    }
}

fn main() {
    let path = env::args().nth(1).expect("surface GBX");
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..3], b"GBX");
    let mut reader = Reader {
        bytes: &bytes,
        offset: 3,
    };
    let version = reader.u16();
    let format = reader.bytes[reader.offset];
    reader.offset += 1;
    let ref_comp = reader.bytes[reader.offset];
    reader.offset += 1;
    let body_comp = reader.bytes[reader.offset];
    reader.offset += 1;
    if version >= 4 {
        reader.offset += 1;
    }
    let class_id = reader.u32();
    if version >= 6 {
        let n = reader.u32() as usize;
        reader.offset += n;
    }
    let nodes = reader.u32();
    let refs = reader.u32();
    assert_eq!(refs, 0, "surface fixture unexpectedly has external refs");
    println!("version={version} format={} ref_comp={} body_comp={} class=0x{class_id:08X} nodes={nodes} body=0x{:X}", format as char, ref_comp as char, body_comp as char, reader.offset);
    let chunk = reader.u32();
    let chunk_version = reader.i32();
    let surf_version = reader.i32();
    println!("chunk=0x{chunk:08X} chunk_version={chunk_version} surf_version={surf_version}");
    read_surf(&mut reader, surf_version, 0);
    println!("surface_tree_end=0x{:X}", reader.offset);
}
