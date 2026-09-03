use std::{
    env,
    ffi::{c_char, c_int, c_void},
    fs,
};

const HEADER_KEY: [u8; 16] = [
    0x56, 0xee, 0xcb, 0xbb, 0xde, 0xb6, 0xbc, 0x90, 0xa1, 0x7d, 0xfc, 0xeb, 0x76, 0x1d, 0x59, 0xce,
];

struct Blowfish18 {
    p: Vec<u32>,
    s: Vec<u32>,
}

impl Blowfish18 {
    fn new(key: &[u8], constants_source: &str) -> Self {
        let constants = parse_constants(constants_source);
        assert_eq!(constants.len(), 18 + 4 * 256);
        let mut state = Self {
            p: constants[..18].to_vec(),
            s: constants[18..].to_vec(),
        };
        let mut key_index = 0usize;
        for index in 0..10 {
            let mut value = 0u32;
            for byte_index in 0..4 {
                value |= (key[key_index] as u32) << (byte_index * 8);
                key_index = (key_index + 1) % key.len();
            }
            state.p[index] ^= value;
        }
        let (mut left, mut right) = (0u32, 0u32);
        for index in (0..10).step_by(2) {
            (left, right) = state.encrypt_words(left, right);
            state.p[index] = left;
            state.p[index + 1] = right;
        }
        for index in (0..state.s.len()).step_by(2) {
            (left, right) = state.encrypt_words(left, right);
            state.s[index] = left;
            state.s[index + 1] = right;
        }
        state.p[..10].reverse();
        state
    }

    fn f(&self, x: u32) -> u32 {
        let a = (x >> 24) as usize;
        let b = ((x >> 16) & 0xff) as usize;
        let c = ((x >> 8) & 0xff) as usize;
        let d = (x & 0xff) as usize;
        self.s[a]
            .wrapping_add(self.s[256 + b])
            .bitxor(self.s[512 + c])
            .wrapping_add(self.s[768 + d])
    }

    fn encrypt_words(&self, mut left: u32, mut right: u32) -> (u32, u32) {
        for index in 0..8 {
            left ^= self.p[index];
            right ^= self.f(left);
            std::mem::swap(&mut left, &mut right);
        }
        std::mem::swap(&mut left, &mut right);
        right ^= self.p[8];
        left ^= self.p[9];
        (left, right)
    }

    fn encrypt_block(&self, block: &mut [u8]) {
        let left = u32::from_le_bytes(block[..4].try_into().unwrap());
        let right = u32::from_le_bytes(block[4..8].try_into().unwrap());
        let (left, right) = self.encrypt_words(left, right);
        block[..4].copy_from_slice(&left.to_le_bytes());
        block[4..8].copy_from_slice(&right.to_le_bytes());
    }

    fn decrypt_words(&self, output_left: u32, output_right: u32) -> (u32, u32) {
        let mut left = output_right ^ self.p[8];
        let mut right = output_left ^ self.p[9];
        for index in (0..8).rev() {
            let previous_left = right ^ self.p[index];
            let previous_right = left ^ self.f(right);
            left = previous_left;
            right = previous_right;
        }
        (left, right)
    }

    fn decrypt_block(&self, block: &mut [u8]) {
        let left = u32::from_le_bytes(block[..4].try_into().unwrap());
        let right = u32::from_le_bytes(block[4..8].try_into().unwrap());
        let (left, right) = self.decrypt_words(left, right);
        block[..4].copy_from_slice(&left.to_le_bytes());
        block[4..8].copy_from_slice(&right.to_le_bytes());
    }
}

trait BitXorExt {
    fn bitxor(self, rhs: Self) -> Self;
}
impl BitXorExt for u32 {
    fn bitxor(self, rhs: Self) -> Self {
        self ^ rhs
    }
}

fn parse_constants(source: &str) -> Vec<u32> {
    let source = source
        .split("private readonly uint[] _p")
        .nth(1)
        .expect("P-array marker");
    let bytes = source.as_bytes();
    let mut values = Vec::new();
    let mut index = 0usize;
    while index + 2 < bytes.len() {
        if bytes[index] == b'0' && bytes[index + 1] == b'x' {
            let start = index + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            values.push(u32::from_str_radix(&source[start..end], 16).unwrap());
            index = end;
        } else {
            index += 1;
        }
    }
    values
}

struct CryptoCursor<'a> {
    data: &'a [u8],
    position: usize,
    blowfish: Blowfish18,
    iv: u64,
    iv_xor: u64,
    block: [u8; 8],
    block_index: usize,
    cycle_index: usize,
    recorded: Vec<u8>,
}

impl<'a> CryptoCursor<'a> {
    fn new(data: &'a [u8], key: &[u8], iv: u64, constants: &str) -> Self {
        Self {
            data,
            position: 0,
            blowfish: Blowfish18::new(key, constants),
            iv,
            iv_xor: 0,
            block: [0; 8],
            block_index: 8,
            cycle_index: 0,
            recorded: Vec::new(),
        }
    }

    fn initialize_iv_xor(&mut self, data: &[u8]) {
        for byte in data {
            let low = self.iv_xor as u32;
            let high = (self.iv_xor >> 32) as u32;
            let new_low = (*byte as u32 | 0xaa) ^ ((low << 13) | (high >> 19));
            let new_high = (self.iv_xor << 13 >> 32) as u32;
            self.iv_xor = ((new_high as u64) << 32) | new_low as u64;
        }
    }

    fn fill_block(&mut self) {
        if self.cycle_index == 0x100 {
            self.iv ^= self.iv_xor;
            self.iv_xor = 0;
            self.cycle_index = 0;
        }
        self.block
            .copy_from_slice(&self.data[self.position..self.position + 8]);
        self.position += 8;
        let next_iv = u64::from_le_bytes(self.block);
        self.blowfish.encrypt_block(&mut self.block);
        let plain = u64::from_le_bytes(self.block) ^ self.iv;
        self.block = plain.to_le_bytes();
        self.iv = (self.iv >> 47) ^ self.iv.wrapping_mul(9) ^ next_iv;
        self.block_index = 0;
    }

    fn read_u8(&mut self) -> u8 {
        if self.block_index == 8 {
            self.fill_block();
        }
        let value = self.block[self.block_index];
        self.block_index += 1;
        self.cycle_index += 1;
        self.recorded.push(value);
        value
    }

    fn read_bytes(&mut self, count: usize) -> Vec<u8> {
        (0..count).map(|_| self.read_u8()).collect()
    }

    fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes(self.read_bytes(4).try_into().unwrap())
    }

    fn read_i32(&mut self) -> i32 {
        self.read_u32() as i32
    }

    fn read_u64(&mut self) -> u64 {
        u64::from_le_bytes(self.read_bytes(8).try_into().unwrap())
    }

    fn read_string(&mut self) -> String {
        let length = self.read_u32() as usize;
        assert!(length < 1_000_000, "implausible string length {length}");
        String::from_utf8_lossy(&self.read_bytes(length)).into_owned()
    }
}

fn accumulate_iv_xor(iv_xor: &mut u64, data: &[u8]) {
    for byte in data {
        let low = *iv_xor as u32;
        let high = (*iv_xor >> 32) as u32;
        let new_low = (*byte as u32 | 0xaa) ^ ((low << 13) | (high >> 19));
        let new_high = (*iv_xor << 13 >> 32) as u32;
        *iv_xor = ((new_high as u64) << 32) | new_low as u64;
    }
}

fn encrypt_header(
    plain: &[u8],
    key: &[u8],
    initial_iv: u64,
    constants: &str,
    dummy_position: usize,
    dummy_data: &[u8],
) -> Vec<u8> {
    assert_eq!(plain.len() % 8, 0);
    let blowfish = Blowfish18::new(key, constants);
    let mut probe = [0x13, 0x37, 0x42, 0x99, 0x01, 0x02, 0x03, 0x04];
    let original = probe;
    blowfish.encrypt_block(&mut probe);
    blowfish.decrypt_block(&mut probe);
    assert_eq!(probe, original, "Blowfish inverse self-check failed");

    let mut output = Vec::with_capacity(plain.len());
    let mut iv = initial_iv;
    let mut iv_xor = 0u64;
    let mut cycle_index = 0usize;
    let mut dummy_applied = false;
    for start in (0..plain.len()).step_by(8) {
        if !dummy_applied && dummy_position <= start {
            accumulate_iv_xor(&mut iv_xor, dummy_data);
            dummy_applied = true;
        }
        if cycle_index == 0x100 {
            iv ^= iv_xor;
            iv_xor = 0;
            cycle_index = 0;
        }
        let mut block =
            (u64::from_le_bytes(plain[start..start + 8].try_into().unwrap()) ^ iv).to_le_bytes();
        blowfish.decrypt_block(&mut block);
        let cipher = u64::from_le_bytes(block);
        output.extend_from_slice(&block);
        iv = (iv >> 47) ^ iv.wrapping_mul(9) ^ cipher;
        cycle_index += 8;
        if !dummy_applied && dummy_position < start + 8 {
            accumulate_iv_xor(&mut iv_xor, dummy_data);
            dummy_applied = true;
        }
    }
    assert!(dummy_applied);
    output
}

#[derive(Debug)]
struct Folder {
    parent: i32,
    name: String,
}

fn u32le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn parse_unencrypted_prefix(pack: &[u8]) -> (u32, usize, usize, u64) {
    assert_eq!(&pack[..8], b"NadeoPak");
    assert_eq!(u32le(pack, 8), 18);
    let flags = u32le(pack, 44);
    let header_max = u32le(pack, 48) as usize;
    let mut cursor = 52usize;
    cursor += 4;
    for _ in 0..4 {
        let length = u32le(pack, cursor) as usize;
        cursor += 4 + length;
    }
    for _ in 0..2 {
        let length = u32le(pack, cursor) as usize;
        cursor += 4 + length;
    }
    cursor += 8;
    for _ in 0..5 {
        let length = u32le(pack, cursor) as usize;
        cursor += 4 + length;
    }
    cursor += 16;
    let included = u32le(pack, cursor);
    cursor += 4;
    assert_eq!(
        included, 0,
        "official pack unexpectedly includes another pack"
    );
    let iv = u64::from_le_bytes(pack[cursor..cursor + 8].try_into().unwrap());
    cursor += 8;
    (flags, header_max, cursor, iv)
}

fn folder_path(index: i32, folders: &[Folder]) -> String {
    if index < 0 {
        return String::new();
    }
    let folder = &folders[index as usize];
    let parent = folder_path(folder.parent, folders);
    if parent.is_empty() {
        folder.name.trim_end_matches(['\\', '/']).to_string()
    } else {
        format!("{}\\{}", parent, folder.name.trim_end_matches(['\\', '/']))
    }
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|byte| format!("{byte:02X}")).collect()
}

#[link(name = "lz4")]
unsafe extern "C" {
    fn LZ4_createStreamDecode() -> *mut c_void;
    fn LZ4_freeStreamDecode(stream: *mut c_void) -> c_int;
    fn LZ4_setStreamDecode(stream: *mut c_void, dictionary: *const c_char, size: c_int) -> c_int;
    fn LZ4_decompress_safe_continue(
        stream: *mut c_void,
        source: *const c_char,
        destination: *mut c_char,
        source_size: c_int,
        destination_capacity: c_int,
    ) -> c_int;
}

fn parse_lz4_dictionary(source: &str) -> Vec<u8> {
    let body = source
        .split("LZ4_DICTIONARY")
        .nth(1)
        .expect("LZ4 dictionary marker")
        .split("];")
        .next()
        .expect("LZ4 dictionary terminator");
    let bytes = body.as_bytes();
    let mut output = Vec::new();
    let mut index = 0usize;
    while index + 2 < bytes.len() {
        if bytes[index] == b'0' && bytes[index + 1] == b'x' {
            let start = index + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            output.push(u8::from_str_radix(&body[start..end], 16).unwrap());
            index = end;
        } else {
            index += 1;
        }
    }
    output
}

fn decompress_lz4_blocks(data: &[u8], output_size: usize, dictionary_source: &str) -> Vec<u8> {
    const RING_SIZE: usize = 73_728;
    let dictionary = parse_lz4_dictionary(dictionary_source);
    let mut ring = vec![0u8; RING_SIZE];
    let stream = unsafe { LZ4_createStreamDecode() };
    assert!(!stream.is_null(), "LZ4_createStreamDecode failed");
    assert_eq!(
        unsafe {
            LZ4_setStreamDecode(
                stream,
                dictionary.as_ptr().cast::<c_char>(),
                dictionary.len() as c_int,
            )
        },
        1,
        "LZ4_setStreamDecode failed",
    );
    let mut input = 0usize;
    let mut output = Vec::with_capacity(output_size);
    while output.len() < output_size {
        assert!(input + 2 <= data.len(), "truncated LZ4 block length");
        let compressed = u16::from_le_bytes(data[input..input + 2].try_into().unwrap()) as usize;
        input += 2;
        assert!(compressed <= 4_128 && input + compressed <= data.len());
        let ring_offset = output.len() % RING_SIZE;
        assert!(ring_offset + 4_096 <= ring.len());
        let written = unsafe {
            LZ4_decompress_safe_continue(
                stream,
                data[input..input + compressed].as_ptr().cast::<c_char>(),
                ring[ring_offset..].as_mut_ptr().cast::<c_char>(),
                compressed as c_int,
                4_096,
            )
        };
        eprintln!(
            "lz4 block output_offset={} input_offset={} compressed={} written={}",
            output.len(),
            input - 2,
            compressed,
            written,
        );
        if written < 0 {
            eprintln!("LZ4 stream stopped at the first dummy-write boundary");
            break;
        }
        output.extend_from_slice(&ring[ring_offset..ring_offset + written as usize]);
        input += compressed;
    }
    unsafe { LZ4_freeStreamDecode(stream) };
    output
}

pub fn patch_snow_header(pack_path: &str, constants_path: &str, key_hex: &str, output: &str) {
    let pack = fs::read(pack_path).expect("pack");
    let constants = fs::read_to_string(constants_path).expect("Blowfish.cs");
    let supplied = (0..key_hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&key_hex[index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(supplied.len(), 16);
    let (flags, header_max, encrypted_offset, iv) = parse_unencrypted_prefix(&pack);
    let mut header_key = supplied;
    if flags & 2 != 0 {
        for (byte, default) in header_key.iter_mut().zip(HEADER_KEY) {
            *byte ^= default;
        }
    }
    let encrypted_original = &pack[encrypted_offset..header_max];
    let mut reader = CryptoCursor::new(encrypted_original, &header_key, iv, &constants);
    let stored_header_md5 = reader.read_bytes(16);
    reader.read_u32();
    reader.read_i32();
    reader.read_i32();
    reader.read_bytes(16);
    reader.read_u32();
    reader.read_bytes(16);
    reader.read_u32();
    let folder_count = reader.read_i32();
    assert!((0..100_000).contains(&folder_count));
    let mut folders = Vec::with_capacity(folder_count as usize);
    for _ in 0..folder_count {
        folders.push(Folder {
            parent: reader.read_i32(),
            name: reader.read_string(),
        });
    }
    assert!(folders.len() > 2 && folders[2].name.len() > 4);
    let utf16 = folders[2]
        .name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    let dummy_data = utf16[4..8].to_vec();
    let dummy_position = reader.recorded.len();
    reader.initialize_iv_xor(&dummy_data);
    let file_count = reader.read_i32();
    assert!((0..10_000_000).contains(&file_count));

    #[derive(Clone)]
    struct Update {
        label: &'static str,
        uncompressed_offset: usize,
        compressed_offset: usize,
        checksum_offset: usize,
        old_uncompressed: i32,
        old_compressed: i32,
        old_checksum: [u8; 16],
        new_uncompressed: i32,
        new_compressed: i32,
        new_checksum: [u8; 16],
    }
    fn bytes16(hex: &str) -> [u8; 16] {
        let values = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        values.try_into().unwrap()
    }

    let mut updates = Vec::new();
    for _ in 0..file_count {
        reader.read_i32();
        let name = reader.read_string();
        reader.read_i32();
        let uncompressed_offset = reader.recorded.len();
        let old_uncompressed = reader.read_i32();
        let compressed_offset = reader.recorded.len();
        let old_compressed = reader.read_i32();
        reader.read_u32();
        reader.read_u32();
        reader.read_i32();
        let checksum_offset = reader.recorded.len();
        let old_checksum: [u8; 16] = reader.read_bytes(16).try_into().unwrap();
        reader.read_u64();
        let update = if name == "0212AB465CB86A374B5B0CA3D868ADE971" {
            Some(Update {
                label: "PhyModelSnow",
                uncompressed_offset,
                compressed_offset,
                checksum_offset,
                old_uncompressed,
                old_compressed,
                old_checksum,
                new_uncompressed: 595,
                new_compressed: 595,
                new_checksum: bytes16("59A4229C79CBC0BF21927113D5383A9D"),
            })
        } else if name == "E100BAE2DC5C0D433D51DBB611406ACB81" {
            Some(Update {
                label: "TuningsSnow",
                uncompressed_offset,
                compressed_offset,
                checksum_offset,
                old_uncompressed,
                old_compressed,
                old_checksum,
                new_uncompressed: 10_210,
                new_compressed: 3_875,
                new_checksum: bytes16("2536FFB44D607C199ADDD332E74575D6"),
            })
        } else {
            None
        };
        if let Some(update) = update {
            updates.push(update);
        }
    }
    assert_eq!(updates.len(), 2, "expected exactly two Snow descriptors");
    let table_end = reader.recorded.len();
    let padding = (8 - (reader.recorded.len() % 8)) % 8;
    reader.read_bytes(padding);
    let original_plain = reader.recorded;
    let roundtrip = encrypt_header(
        &original_plain,
        &header_key,
        iv,
        &constants,
        dummy_position,
        &dummy_data,
    );
    assert_eq!(
        roundtrip,
        encrypted_original[..original_plain.len()],
        "header roundtrip mismatch"
    );
    fs::write(format!("{output}.original-plain"), &original_plain)
        .expect("write original plain header");
    eprintln!(
        "stored_header_md5={} table_end={table_end} plain_bytes={}",
        hex(&stored_header_md5),
        original_plain.len()
    );

    let mut patched_plain = original_plain;
    for update in updates {
        match update.label {
            "PhyModelSnow" => {
                assert_eq!(update.old_uncompressed, 635);
                assert_eq!(update.old_compressed, 635);
                assert_eq!(
                    update.old_checksum,
                    bytes16("CEC64DB882FAD5181C9B8DCA3C92F6F6")
                );
            }
            "TuningsSnow" => {
                assert_eq!(update.old_uncompressed, 10_170);
                assert_eq!(update.old_compressed, 3_853);
                assert_eq!(
                    update.old_checksum,
                    bytes16("DBB87952EF72948D8880C088D3E686BF")
                );
            }
            _ => unreachable!(),
        }
        patched_plain[update.uncompressed_offset..update.uncompressed_offset + 4]
            .copy_from_slice(&update.new_uncompressed.to_le_bytes());
        patched_plain[update.compressed_offset..update.compressed_offset + 4]
            .copy_from_slice(&update.new_compressed.to_le_bytes());
        patched_plain[update.checksum_offset..update.checksum_offset + 16]
            .copy_from_slice(&update.new_checksum);
        eprintln!(
            "patched {} sizes {}/{} -> {}/{}",
            update.label,
            update.old_uncompressed,
            update.old_compressed,
            update.new_uncompressed,
            update.new_compressed
        );
    }
    let encrypted_patched = encrypt_header(
        &patched_plain,
        &header_key,
        iv,
        &constants,
        dummy_position,
        &dummy_data,
    );
    fs::write(output, &encrypted_patched).expect("write patched encrypted header");
    println!(
        "encrypted_offset={encrypted_offset} header_bytes={}",
        encrypted_patched.len()
    );
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 3 || args.len() > 6 {
        eprintln!("usage: pak_extract PACK Blowfish.cs ENCRYPTION_KEY_HEX [NAME_SUBSTRING [OUTPUT LZ4Stream.cs]]");
        std::process::exit(2);
    }
    let pack = fs::read(&args[0]).expect("pack");
    let constants = fs::read_to_string(&args[1]).expect("Blowfish.cs");
    let output_path = args.get(4);
    let lz4_source = args
        .get(5)
        .map(|path| fs::read_to_string(path).expect("LZ4Stream.cs"));
    assert_eq!(
        output_path.is_some(),
        lz4_source.is_some(),
        "OUTPUT and LZ4Stream.cs must be given together"
    );
    let supplied = (0..args[2].len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&args[2][index..index + 2], 16).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(supplied.len(), 16);
    let query = args.get(3).map(|value| value.to_ascii_lowercase());
    let (flags, header_max, encrypted_offset, iv) = parse_unencrypted_prefix(&pack);
    let mut header_key = supplied.clone();
    if flags & 2 != 0 {
        for (byte, default) in header_key.iter_mut().zip(HEADER_KEY) {
            *byte ^= default;
        }
    }
    let mut reader = CryptoCursor::new(
        &pack[encrypted_offset..header_max],
        &header_key,
        iv,
        &constants,
    );
    let header_md5 = reader.read_bytes(16);
    let gbx_headers_start = reader.read_u32();
    let gbx_headers_size = reader.read_i32();
    let gbx_headers_compressed = reader.read_i32();
    let unused1 = reader.read_bytes(16);
    let file_size = reader.read_u32();
    let unused2 = reader.read_bytes(16);
    let inner_flags = reader.read_u32();
    let folder_count = reader.read_i32();
    assert!((0..100_000).contains(&folder_count));
    let mut folders = Vec::with_capacity(folder_count as usize);
    for _ in 0..folder_count {
        folders.push(Folder {
            parent: reader.read_i32(),
            name: reader.read_string(),
        });
    }
    if folders.len() > 2 && folders[2].name.len() > 4 {
        let utf16 = folders[2]
            .name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        reader.initialize_iv_xor(&utf16[4..8]);
    }
    let file_count = reader.read_i32();
    assert!((0..10_000_000).contains(&file_count));
    println!(
        "header_md5={} gbx_headers_start=0x{gbx_headers_start:X} gbx_headers_size={gbx_headers_size} gbx_headers_compressed={gbx_headers_compressed} file_size={file_size} flags=0x{inner_flags:X} folders={folder_count} files={file_count} unused_zero={}",
        hex(&header_md5),
        unused1.iter().chain(&unused2).all(|byte| *byte == 0),
    );
    let mut matches = 0usize;
    for index in 0..file_count {
        let folder_index = reader.read_i32();
        let name = reader.read_string();
        let unknown = reader.read_i32();
        let uncompressed_size = reader.read_i32();
        let compressed_size = reader.read_i32();
        let offset = reader.read_u32();
        let class_id = reader.read_u32();
        let size = reader.read_i32();
        let checksum = reader.read_bytes(16);
        let file_flags = reader.read_u64();
        let folder = folder_path(folder_index, &folders);
        let path = if folder.is_empty() {
            name.clone()
        } else {
            format!("{folder}\\{name}")
        };
        let selected = match &query {
            Some(query) => path.to_ascii_lowercase().contains(query),
            None => class_id == 0x2E002000,
        };
        if selected {
            println!(
                "index={index} path={path:?} class=0x{class_id:08X} offset=0x{offset:X} compressed={compressed_size} uncompressed={uncompressed_size} size={size} flags=0x{file_flags:X} checksum={} unknown={unknown}",
                hex(&checksum),
            );
            if let (Some(output_path), Some(lz4_source)) = (output_path, &lz4_source) {
                assert_eq!(matches, 0, "output query matched more than one file");
                let start = header_max + offset as usize;
                let encrypted =
                    file_flags & 0x4000000000000 == 0 && file_flags & 0x2000000000000 == 0;
                let decrypted;
                let stored = if encrypted {
                    let file_iv = u64::from_le_bytes(pack[start..start + 8].try_into().unwrap());
                    let mut file_reader =
                        CryptoCursor::new(&pack[start + 8..], &supplied, file_iv, &constants);
                    decrypted = file_reader.read_bytes(compressed_size as usize);
                    decrypted.as_slice()
                } else {
                    &pack[start..start + compressed_size as usize]
                };
                let payload = if file_flags & 0x3c != 0 {
                    decompress_lz4_blocks(stored, uncompressed_size as usize, lz4_source)
                } else {
                    stored.to_vec()
                };
                fs::write(output_path, &payload).expect("write extracted file");
                eprintln!("wrote {} bytes to {output_path}", payload.len());
            }
            matches += 1;
        }
    }
    eprintln!("matches={matches}");
}
