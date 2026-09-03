use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tmmaps::map::{Kind, MapFile};

const START: usize = 311;
const FINISH: usize = 244;
const CHECKPOINTS: [usize; 4] = [165, 170, 243, 261];
const TURBO: usize = 265;
const CORRIDOR_X: i32 = 10;
const ROAD_Y: i32 = 30;
const GATE_Y: i32 = 29;

#[derive(Clone, Copy)]
struct Surface {
    target: &'static str,
    cell: (i32, i32, i32),
}

fn route() -> Vec<Surface> {
    let mut out = Vec::new();
    for z in 9..=15 {
        out.push(Surface { target: "RoadIceStraight", cell: (CORRIDOR_X, ROAD_Y, z) });
    }
    for z in 20..=34 {
        out.push(Surface { target: "RoadIceStraight", cell: (CORRIDOR_X, ROAD_Y, z) });
    }
    out
}

fn same_f32(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits()
}

fn assert_unedited_records(before: &MapFile, after: &MapFile, changed: &BTreeSet<usize>) {
    assert_eq!(before.blocks.len(), after.blocks.len(), "unbaked block count");
    assert_eq!(before.baked.len(), after.baked.len(), "baked block count");
    assert_eq!(before.items.len(), after.items.len(), "item count");

    for (a, b) in before.blocks.iter().zip(&after.blocks) {
        if changed.contains(&a.index) {
            continue;
        }
        assert_eq!(a.name, b.name, "unedited block#{} name", a.index);
        assert_eq!(a.coords(), b.coords(), "unedited block#{} cell", a.index);
        assert_eq!(a.dir, b.dir, "unedited block#{} dir", a.index);
        assert_eq!(a.flags, b.flags, "unedited block#{} flags", a.index);
        assert_eq!(a.waypoint_tag, b.waypoint_tag, "unedited block#{} waypoint", a.index);
        assert_eq!(a.free_pos.map(|v| v.map(f32::to_bits)), b.free_pos.map(|v| v.map(f32::to_bits)), "unedited block#{} free position", a.index);
        assert_eq!(a.free_rot.map(|v| v.map(f32::to_bits)), b.free_rot.map(|v| v.map(f32::to_bits)), "unedited block#{} free rotation", a.index);
    }
    for (a, b) in before.baked.iter().zip(&after.baked) {
        assert_eq!(a.name, b.name, "baked block#{} name", a.index);
        assert_eq!(a.coords(), b.coords(), "baked block#{} cell", a.index);
        assert_eq!(a.dir, b.dir, "baked block#{} dir", a.index);
        assert_eq!(a.flags, b.flags, "baked block#{} flags", a.index);
        assert_eq!(a.waypoint_tag, b.waypoint_tag, "baked block#{} waypoint", a.index);
        assert_eq!(a.free_pos.map(|v| v.map(f32::to_bits)), b.free_pos.map(|v| v.map(f32::to_bits)), "baked block#{} free position", a.index);
        assert_eq!(a.free_rot.map(|v| v.map(f32::to_bits)), b.free_rot.map(|v| v.map(f32::to_bits)), "baked block#{} free rotation", a.index);
    }
    for (a, b) in before.items.iter().zip(&after.items) {
        assert_eq!(a.model, b.model, "item#{} model", a.index);
        assert_eq!(a.coords(), b.coords(), "item#{} cell", a.index);
        assert!(same_f32(a.yaw, b.yaw), "item#{} yaw", a.index);
        assert_eq!(a.pos.map(f32::to_bits), b.pos.map(f32::to_bits), "item#{} position", a.index);
        assert!(same_f32(a.pitch, b.pitch), "item#{} pitch", a.index);
        assert!(same_f32(a.roll, b.roll), "item#{} roll", a.index);
        assert!(same_f32(a.scale, b.scale), "item#{} scale", a.index);
        assert_eq!(a.waypoint_tag, b.waypoint_tag, "item#{} waypoint", a.index);
    }
}

fn main() {
    let mut args = std::env::args_os();
    let _exe = args.next();
    let src = PathBuf::from(args.next().expect("usage: sep30-native-control SOURCE OUT"));
    let out = PathBuf::from(args.next().expect("usage: sep30-native-control SOURCE OUT"));
    assert!(args.next().is_none(), "usage: sep30-native-control SOURCE OUT");
    assert_ne!(src, out, "source and output must differ");
    assert!(!out.exists(), "refusing to overwrite {}", out.display());

    let before = MapFile::load(Path::new(&src));
    assert_eq!(before.decoration_id, "48x48Day", "source decoration");
    assert_eq!(before.size, [48, 40, 48], "source size");
    assert_eq!(before.blocks.len(), 317, "source unbaked blocks");
    assert_eq!(before.baked.len(), 2471, "source baked blocks");
    assert_eq!(before.items.len(), 199, "source items");
    for (index, expected, tag) in [
        (START, "RoadBumpStart", "Spawn"),
        (FINISH, "GateFinish", "Goal"),
        (TURBO, "GateSpecialTurbo", ""),
        (165, "GateCheckpoint", "Checkpoint"),
        (170, "GateCheckpoint", "Checkpoint"),
        (243, "GateCheckpoint", "Checkpoint"),
        (261, "GateCheckpoint", "Checkpoint"),
    ] {
        let b = &before.blocks[index];
        assert_eq!(b.name, expected, "source block#{index} model");
        if tag.is_empty() {
            assert!(b.waypoint_tag.is_none(), "source block#{index} must remain a non-waypoint native effect");
        } else {
            assert_eq!(b.waypoint_tag.as_deref(), Some(tag), "source block#{index} waypoint tag");
        }
        assert!(b.free_off.is_none(), "source block#{index} must be a grid block");
    }

    assert!(ROAD_Y >= 0 && ROAD_Y < before.size[1], "road y must be inside map height");
    assert!(GATE_Y >= 0 && GATE_Y < before.size[1], "gate y must be inside map height");
    let corridor_empty = before.blocks.iter().all(|b| {
        let (x, y, z) = b.coords();
        !(x == CORRIDOR_X && (GATE_Y..=ROAD_Y).contains(&y) && (8..=34).contains(&z))
    }) && before.baked.iter().all(|b| {
        let (x, y, z) = b.coords();
        !(x == CORRIDOR_X && (GATE_Y..=ROAD_Y).contains(&y) && (8..=34).contains(&z))
    }) && before.items.iter().all(|item| {
        let p = item.pos;
        !(p[0] >= 320.0 && p[0] <= 352.0 && p[1] >= 160.0 && p[1] <= 194.0 && p[2] >= 256.0 && p[2] <= 1120.0)
    });
    assert!(corridor_empty, "target corridor was not empty in the source");

    const VARIABLE_FLAGS: u32 = 0x0000_8000 | 0x0010_0000 | 0x2000_0000;
    let excluded = [START, FINISH, TURBO, 165, 170, 243, 261];
    let surfaces = route();

    // Pick complete lookback-slot groups rather than arbitrary records. Every
    // use of each selected slot must be one of these neutral unbaked blocks;
    // renaming the whole group means no surviving field needs the slot's old
    // string, so the slot-preserving encoder keeps cardinality exactly constant.
    let mut all_uses = BTreeMap::<usize, usize>::new();
    for field in &before.body_ids {
        if let Some(slot) = field.slot {
            *all_uses.entry(slot).or_default() += 1;
        }
    }
    let mut unbaked_by_slot = BTreeMap::<usize, Vec<usize>>::new();
    for block in &before.blocks {
        let slot = before.body_ids[block.name_field].slot.expect("block model has a lookback slot");
        unbaked_by_slot.entry(slot).or_default().push(block.index);
    }
    let baked_slots: BTreeSet<usize> = before.baked.iter()
        .map(|block| before.body_ids[block.name_field].slot.expect("baked model has a lookback slot"))
        .collect();
    let mut groups: Vec<(usize, Vec<usize>)> = unbaked_by_slot.into_iter()
        .filter(|(slot, indices)| !baked_slots.contains(slot) && all_uses.get(slot) == Some(&indices.len()))
        .filter(|(_, indices)| indices.iter().all(|&index| {
            let block = &before.blocks[index];
            block.free_off.is_none()
                && block.flags & VARIABLE_FLAGS == 0
                && block.waypoint_tag.is_none()
                && !excluded.contains(&index)
        }))
        .collect();
    groups.sort_by_key(|(_, indices)| indices[0]);

    let target_count = surfaces.len();
    let mut choices: Vec<Option<Vec<usize>>> = vec![None; target_count + 1];
    choices[0] = Some(Vec::new());
    for (group_index, (_, indices)) in groups.iter().enumerate() {
        let count = indices.len();
        for total in (count..=target_count).rev() {
            if choices[total].is_none() {
                if let Some(mut prior) = choices[total - count].clone() {
                    prior.push(group_index);
                    choices[total] = Some(prior);
                }
            }
        }
    }
    let selected_groups = choices[target_count].clone().expect("no slot-stable neutral record set for route");
    let selected_slots: BTreeSet<usize> = selected_groups.iter().map(|&i| groups[i].0).collect();
    let mut candidates: Vec<usize> = selected_groups.into_iter()
        .flat_map(|i| groups[i].1.iter().copied())
        .collect();
    candidates.sort_unstable();
    assert_eq!(candidates.len(), surfaces.len(), "slot-stable neutral route source count");
    for (slot, uses) in &all_uses {
        if selected_slots.contains(slot) {
            let selected_uses = candidates.iter()
                .filter(|&&index| before.body_ids[before.blocks[index].name_field].slot == Some(*slot))
                .count();
            assert_eq!(selected_uses, *uses, "every use of selected lookback slot {slot} is renamed");
        }
    }

    let source_names: Vec<String> = candidates.iter().map(|&i| before.blocks[i].name.clone()).collect();
    let mut map = MapFile::load(Path::new(&src));

    // Keep every native waypoint model and its serialized waypoint class/tag.
    // Only its fixed-size cell/direction bytes move.
    map.move_block_cell(START, (CORRIDOR_X, ROAD_Y, 8));
    map.set_block_dir(START, 0);
    // Keep the source's exact Sep30-era turbo model and flags. It is an effect
    // gate over an ordinary RoadIceStraight surface, not a synthesized road model.
    map.move_block_cell(TURBO, (CORRIDOR_X, GATE_Y, 14));
    map.set_block_dir(TURBO, 0);
    let checkpoint_cells = [
        (CORRIDOR_X, GATE_Y, 10),
        (CORRIDOR_X, GATE_Y, 13),
        (CORRIDOR_X, GATE_Y, 24),
        (CORRIDOR_X, GATE_Y, 30),
    ];
    for (&index, &cell) in CHECKPOINTS.iter().zip(&checkpoint_cells) {
        map.move_block_cell(index, cell);
        map.set_block_dir(index, 0);
    }
    map.move_block_cell(FINISH, (CORRIDOR_X, GATE_Y, 34));
    map.set_block_dir(FINISH, 0);

    // Rename only neutral, non-waypoint grid records to Sep30-known pieces.
    for (&index, surface) in candidates.iter().zip(&surfaces) {
        map.set_block_name(index, surface.target);
        map.move_block_cell(index, surface.cell);
        map.set_block_dir(index, 0);
        map.set_block_flags_fixed(index, 0);
    }

    let splice = map.write_to_reporting(&out).expect("write candidate");
    let after = MapFile::load(Path::new(&out));

    let mut changed: BTreeSet<usize> = candidates.iter().copied().collect();
    changed.extend([START, FINISH, TURBO, 165, 170, 243, 261]);
    assert_unedited_records(&before, &after, &changed);

    // Native waypoint models, classes/tags and flags must survive byte-for-byte
    // semantically; only cell and direction are allowed to differ.
    let waypoint_expect = [
        (START, "RoadBumpStart", "Spawn", (CORRIDOR_X, ROAD_Y, 8)),
        (165, "GateCheckpoint", "Checkpoint", checkpoint_cells[0]),
        (170, "GateCheckpoint", "Checkpoint", checkpoint_cells[1]),
        (243, "GateCheckpoint", "Checkpoint", checkpoint_cells[2]),
        (261, "GateCheckpoint", "Checkpoint", checkpoint_cells[3]),
        (FINISH, "GateFinish", "Goal", (CORRIDOR_X, GATE_Y, 34)),
    ];
    for (index, model, tag, cell) in waypoint_expect {
        let a = &before.blocks[index];
        let b = &after.blocks[index];
        assert_eq!(b.name, model, "candidate block#{index} model");
        assert_eq!(b.waypoint_tag.as_deref(), Some(tag), "candidate block#{index} tag");
        assert_eq!(b.flags, a.flags, "candidate block#{index} flags");
        assert_eq!(b.coords(), cell, "candidate block#{index} cell");
        assert_eq!(b.dir, 0, "candidate block#{index} direction");
    }
    let waypoints = after.waypoints();
    assert_eq!(waypoints.len(), 6, "candidate waypoint count");
    assert!(waypoints.iter().all(|w| w.kind == Kind::Block), "all waypoints remain native blocks");
    let turbo_before = &before.blocks[TURBO];
    let turbo_after = &after.blocks[TURBO];
    assert_eq!(turbo_after.name, "GateSpecialTurbo", "native turbo model");
    assert_eq!(turbo_after.flags, turbo_before.flags, "native turbo flags");
    assert_eq!(turbo_after.waypoint_tag, turbo_before.waypoint_tag, "native turbo waypoint state");
    assert_eq!(turbo_after.coords(), (CORRIDOR_X, GATE_Y, 14), "native turbo cell");
    assert_eq!(turbo_after.dir, 0, "native turbo direction");

    let expected_surface_cells: BTreeSet<_> = std::iter::once((CORRIDOR_X, ROAD_Y, 8))
        .chain(surfaces.iter().map(|s| s.cell))
        .collect();
    let placed: Vec<_> = after.blocks.iter()
        .filter(|b| expected_surface_cells.contains(&b.coords()))
        .collect();
    assert_eq!(placed.len(), expected_surface_cells.len(), "one surface block per route cell");
    assert_eq!(after.blocks[START].name, "RoadBumpStart");
    for (&index, surface) in candidates.iter().zip(&surfaces) {
        let b = &after.blocks[index];
        assert_eq!(b.name, surface.target, "route block#{index} model");
        assert_eq!(b.coords(), surface.cell, "route block#{index} cell");
        assert_eq!(b.dir, 0, "route block#{index} direction");
        assert_eq!(b.flags, 0, "route block#{index} flags");
        assert!(b.waypoint_tag.is_none(), "route block#{index} became a waypoint");
    }

    println!("wrote {}", out.display());
    println!("{}", splice.summary());
    println!("source_counts blocks={} baked={} items={} waypoints={}", before.blocks.len(), before.baked.len(), before.items.len(), before.waypoints().len());
    println!("candidate_counts blocks={} baked={} items={} waypoints={}", after.blocks.len(), after.baked.len(), after.items.len(), waypoints.len());
    println!("surface start=block#{} RoadBumpStart@{:?}/dir0", START, after.blocks[START].coords());
    println!("effect block#{} GateSpecialTurbo@{:?}/dir0 flags={:08X}", TURBO, after.blocks[TURBO].coords(), after.blocks[TURBO].flags);
    for ((&index, old), surface) in candidates.iter().zip(&source_names).zip(&surfaces) {
        println!("surface block#{} {} -> {} @ {:?}/dir0 flags={:08X}", index, old, surface.target, surface.cell, after.blocks[index].flags);
    }
    for &index in &CHECKPOINTS {
        println!("waypoint block#{} GateCheckpoint {:?}/dir0 flags={:08X} tag=Checkpoint", index, after.blocks[index].coords(), after.blocks[index].flags);
    }
    println!("waypoint block#{} GateFinish {:?}/dir0 flags={:08X} tag=Goal", FINISH, after.blocks[FINISH].coords(), after.blocks[FINISH].flags);
    println!("validation=PASS bounds_checked corridor_empty native_waypoint_models_preserved native_turbo_preserved unedited_records_semantically_identical route_re_read");
}
