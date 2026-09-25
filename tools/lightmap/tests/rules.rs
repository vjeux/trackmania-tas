//! The baker's rules, pinned by the measurements that produced them (no store needed).

use lightmap::moods::{base_rule, effective_mood, mood_xml, MOOD_XML};
use lightmap::probes::SlotGrid;
use lightmap::synth::{decode_value, encode_value, patch_frame_records, FrameParams};

#[test]
fn sqrt_encoding_round_trips_and_pins_the_game_curve() {
    // p = 255·sqrt(E/m): the chart max lands on 255, half the max on 180, a quarter on 128
    assert_eq!(encode_value(1.0, 1.0), 255);
    assert_eq!(encode_value(0.5, 1.0), 180);
    assert_eq!(encode_value(0.25, 1.0), 128);
    assert_eq!(encode_value(0.0, 1.0), 0);
    assert_eq!(encode_value(2.0, 1.0), 255, "clamped at the max");
    // decode is the inverse in the frame byte's units: fb = 255 ⇒ m = 1
    for p in [0u8, 1, 17, 64, 128, 180, 254, 255] {
        let e = decode_value(p, 255);
        assert_eq!(encode_value(e, 1.0), p, "p {p}");
    }
    // the chart byte is sqrt-encoded too (two editor bakes of one pad agree only this way):
    // fb 128 ⇒ the chart max is (128/255)² of the frame's MaxHDR
    let m = (128.0f32 / 255.0).powi(2);
    assert!((decode_value(255, 128) - m).abs() < 1e-6);
    assert!((decode_value(180, 128) - 0.5 * m).abs() < 2e-3);
    assert_eq!(lightmap::synth::frame_byte(m, 1.0), 128);
}

#[test]
fn the_lighting_mood_is_the_daytime_quarter() {
    // the 25 Summer sources' frame records (MaxHDR/Bounce/Sky triples) — 2026-09-23
    assert_eq!(effective_mood("Sunrise64", Some(0xdaab)), "Sunset", "Tiny 16: Sunrise64 at 0.854");
    assert_eq!(effective_mood("Day", Some(0xdaab)), "Sunset", "RedIsland 02 at 0.854");
    assert_eq!(effective_mood("Day", Some(0x9b59)), "Day", "0.607");
    assert_eq!(effective_mood("48x48Screen155Day", Some(0xceb8)), "Sunset", "Stadium 05 at 0.8075");
    assert_eq!(effective_mood("48x48Screen155Day", Some(0x8111)), "Day", "0.504");
    assert_eq!(effective_mood("48x48Screen155Day", Some(0x5148)), "Sunrise", "0.3175");
    assert_eq!(effective_mood("Sunset", Some(0x4e4b)), "Sunrise", "WhiteShore 13 at 0.306");
    assert_eq!(effective_mood("Day64", Some(0x199a)), "Night", "GreenCoast 09 at 0.10");
    // the default word keeps the decoration's mood
    assert_eq!(effective_mood("Day64", Some(0xffff_ffff)), "Day");
    assert_eq!(effective_mood("Day64", None), "Day");
    assert_eq!(effective_mood("Sunrise", None), "Sunrise");
    assert_eq!(effective_mood("NoStadium48x48Night", None), "Night");
}

#[test]
fn every_collection_has_its_four_moods_with_the_pack_constants() {
    for c in ["BlueBay", "GreenCoast", "RedIsland", "WhiteShore", "Stadium"] {
        for m in ["Day", "Night", "Sunrise", "Sunset"] {
            let x = mood_xml(c, m).unwrap_or_else(|| panic!("{c} {m}"));
            assert!(x.max_hdr >= 1.0 && x.max_hdr <= 3.5, "{c} {m} MaxHDR {}", x.max_hdr);
            assert!(x.bounce_factor >= 1.6 && x.bounce_factor <= 3.0);
            assert!(x.sky_factor >= 0.5 && x.sky_factor <= 5.0);
            if m == "Night" {
                assert_eq!(x.l_dir_sun, [0.0; 3], "{c} night has no sun");
                assert!(x.l_ambient[0] < 0.02);
            } else {
                assert!(x.l_dir_sun[0] > 1.0);
            }
        }
    }
    assert_eq!(MOOD_XML.len(), 20);
    // the frame records of the sources: BlueBay Sunset 3/2/1, GreenCoast Night 1.7/2/5, Stadium Sunset 2.7/1.8/1
    let bb = mood_xml("BlueBay", "Sunset").unwrap();
    assert_eq!((bb.max_hdr, bb.bounce_factor, bb.sky_factor), (3.0, 2.0, 1.0));
    let gc = mood_xml("GreenCoast", "Night").unwrap();
    assert_eq!((gc.max_hdr, gc.bounce_factor, gc.sky_factor), (1.7, 2.0, 5.0));
    let st = mood_xml("Stadium", "Sunset").unwrap();
    assert_eq!((st.max_hdr, st.bounce_factor, st.sky_factor), (2.7, 1.8, 1.0));
}

#[test]
fn the_base_rule_reproduces_the_giant_measurements() {
    // giant child, lmtool itembase on the editor bakes (2026-09-23)
    let none: Vec<(i32, i32, &str)> = vec![];
    // Stadium NoStadium48x48Day, 25 ×2 shipped: 0 blocks, S = 96 → 16384 + 96²
    assert_eq!(base_rule("Stadium", "NoStadium48x48Day", [96, 96, 96], none.iter().copied(), 0, None).base(), 25600);
    // 25 ×2 with one kept Grass block: 16384 + 1 + S² + 7 generated pieces (G from the game's list)
    let one = vec![(10, 10, "Grass")];
    assert_eq!(base_rule("Stadium", "NoStadium48x48Day", [96, 96, 96], one.iter().copied(), 7, None).base(), 25608);
    assert_eq!(base_rule("Stadium", "NoStadium48x48Day", [96, 96, 96], one.iter().copied(), 0, Some(96 * 96 + 7)).base(), 25608);
    // 05 ×2: 604 pool tiles + 2108 generated pieces
    let pool: Vec<(i32, i32, &str)> = (0..604).map(|i| (i % 96, i / 96, "PoolTile")).collect();
    assert_eq!(base_rule("Stadium", "NoStadium48x48Day", [96, 96, 96], pool.iter().copied(), 2108, None).base(), 16384 + 604 + 9216 + 2108);
    // tiny 05: 46 custom water tiles COUNT as authored, 386 generated → 19120
    let water: Vec<(i32, i32, &str)> = (0..46).map(|i| (i, 5, "Water_CustomBlock")).collect();
    let r = base_rule("Stadium", "48x48Screen155Day", [48, 40, 48], water.iter().copied(), 386, None);
    assert_eq!(r.custom_blocks, 46);
    assert_eq!(r.base(), 19120);
    // BlueBay 01 ×2 (S = 128, 1886 Sea records kept — baked records do not count): 128²
    assert_eq!(base_rule("BlueBay", "Day64", [128, 128, 128], none.iter().copied(), 0, None).base(), 16384);
    // RedIsland 17 ×2 / GreenCoast 04 ×2: one kept terrain block replaces its column's tile → S²
    assert_eq!(base_rule("RedIsland", "Sunrise", [96, 96, 96], one.iter().copied(), 0, None).base(), 9216);
    assert_eq!(base_rule("GreenCoast", "Day64", [128, 128, 128], one.iter().copied(), 0, None).base(), 16384);
    // tiny maps: 48² / 64²
    assert_eq!(base_rule("BlueBay", "Sunrise64", [64, 64, 64], none.iter().copied(), 0, None).base(), 4096);
    assert_eq!(base_rule("Stadium", "48x48Screen155Day", [48, 40, 48], none.iter().copied(), 0, None).base(), 16384 + 2304);
}

#[test]
fn frame_records_are_patched_in_place() {
    // a head laid out like the game's: 60 bytes of constants, then three 66-byte records
    let mut head = vec![0u8; 60 + 3 * 66 + 12];
    let fp = FrameParams { daytime: 0xdaab, max_hdr_mood: 3.0, max_hdr: 2.3812, bounce: 2.0, sky: 1.0, sum_area: None, quality: None, decoration: None, filetime: None };
    patch_frame_records(&mut head, &fp);
    for i in 0..3 {
        let r = 60 + 66 * i;
        let u = |o: usize| u32::from_le_bytes([head[r + o], head[r + o + 1], head[r + o + 2], head[r + o + 3]]);
        let f = |o: usize| f32::from_le_bytes([head[r + o], head[r + o + 1], head[r + o + 2], head[r + o + 3]]);
        assert_eq!(u(8), 0xdaab);
        assert_eq!(f(16), 3.0);
        assert_eq!(f(20), 2.3812);
        assert_eq!(f(24), 2.0);
        assert_eq!(f(28), 1.0);
        // the kind word and the −FLT_MAX slot are not the patch's business
        assert_eq!(u(0), 0);
        assert_eq!(u(12), 0);
    }
}

#[test]
fn the_slot_grid_follows_the_map() {
    // tiny BlueBay 64³ with geometry inside 2048 m: the 5×3×5 grid at 16 m cells
    let g = SlotGrid::for_map("BlueBay", "Sunrise64", [2048.0, 512.0, 2048.0], [800.0, -16.0, 33.0], [1584.0, 145.0, 1073.0]);
    assert_eq!(g.n, [5, 3, 5]);
    assert_eq!(g.origin, [0.0, -38.0, 0.0]);
    assert_eq!(g.cell, 16.0);
    // giant 254³ BlueBay: cells double to 32 m while the slot count exceeds 512
    let g = SlotGrid::for_map("BlueBay", "Sunrise64", [8128.0, 2032.0, 8128.0], [976.0, -72.0, -48.0], [7136.0, 1105.0, 8160.0]);
    assert_eq!(g.n, [9, 6, 9]);
    assert_eq!(g.cell, 32.0);
    assert!(g.n[0] * g.n[1] * g.n[2] <= lightmap::probes::MAX_SLOTS);
}
