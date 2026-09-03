mod pak_extract {
    include!("pak_extract.rs");
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        eprintln!("usage: pak_header_patch PACK Blowfish.cs ENCRYPTION_KEY_HEX OUTPUT_HEADER");
        std::process::exit(2);
    }
    pak_extract::patch_snow_header(&args[0], &args[1], &args[2], &args[3]);
}
