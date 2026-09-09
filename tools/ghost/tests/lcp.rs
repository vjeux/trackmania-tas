//! `ghost lcp`: the editor's launched-checkpoints cache, decoded.
//!
//! The fixture is the client's own file for tiny map 20 after vjeux's
//! 2026-09-08 test drive: three checkpoints reached in one run. The numbers
//! asserted here were read off the file by hand (od) before the decoder
//! existed, so the test pins the layout, not the decoder's opinion of it.

use std::path::PathBuf;
use std::process::Command;

fn ghost() -> Option<PathBuf> {
    let mut p = std::env::current_exe().ok()?;
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    let b = p.join("ghost");
    b.exists().then_some(b)
}

fn fixture() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/launchedcp_tiny20.LaunchedCP.gbx");
    p.exists().then_some(p)
}

#[test]
fn the_tiny_20_cache_decodes_to_three_crossings_with_their_approaches() {
    let (Some(ghost), Some(f)) = (ghost(), fixture()) else { return };
    let l = ghost::lcp::load(f.to_str().unwrap()).expect("parse");
    assert_eq!(l.version, 7);
    assert_eq!(l.entries.len(), 3);
    let lm: Vec<u32> = l.entries.iter().map(|e| e.landmark).collect();
    assert_eq!(lm, vec![4, 7, 9]);
    let t: Vec<u32> = l.entries.iter().map(|e| e.time_ms).collect();
    assert_eq!(t, vec![15970, 24590, 37910]);
    let e0 = &l.entries[0];
    assert_eq!(e0.launch_landmark, 4);
    assert!((e0.cp_pos[0] - 918.56165).abs() < 1e-3 && (e0.cp_pos[1] - 45.0).abs() < 1e-6 && (e0.cp_pos[2] - 770.52747).abs() < 1e-3);
    assert!((e0.pos[0] - 916.76953).abs() < 1e-3);
    // the quaternion is unit length and the signed speed is the velocity's norm here
    let q2: f32 = e0.quat.iter().map(|x| x * x).sum();
    assert!((q2 - 1.0).abs() < 1e-3, "|q|^2 = {q2}");
    let vn = (e0.vel[0].powi(2) + e0.vel[1].powi(2) + e0.vel[2].powi(2)).sqrt();
    assert!((vn - e0.speed).abs() < 0.3, "|v| {vn} vs speed {}", e0.speed);
    // front wheels steer, rear wheels do not
    assert!(e0.wheels[0].steer.abs() > 0.1 && (e0.wheels[0].steer - e0.wheels[1].steer).abs() < 1e-6);
    assert_eq!(e0.wheels[2].steer, 0.0);
    assert_eq!(e0.wheels[3].steer, 0.0);
    // approach windows: 28 + 28 + 27 samples, ~1.5 s each, the last one a frame before the crossing
    let counts: Vec<usize> = l.entries.iter().map(|e| e.samples.len()).collect();
    assert_eq!(counts, vec![28, 28, 27]);
    for e in &l.entries {
        let (t_last, last, _) = e.samples.last().unwrap();
        assert!((1400..1600).contains(t_last), "window {t_last} ms");
        let d = ((last.x - e.pos[0]).powi(2) + (last.y - e.pos[1]).powi(2) + (last.z - e.pos[2]).powi(2)).sqrt();
        assert!(d < 3.0, "last approach sample {d:.2} m from the crossing state");
        assert!(e.samples.windows(2).all(|w| w[0].0 < w[1].0), "window times increase");
    }
    // the first approach of entry 0 was read off the file by hand
    let (t0, s0, _) = &l.entries[0].samples[0];
    assert_eq!(*t0, 20);
    assert!((s0.x - 899.2341).abs() < 1e-3 && (s0.z - 802.234).abs() < 1e-3);

    // and the CLI agrees, and refuses a file of another class
    let o = Command::new(&ghost).args(["lcp", f.to_str().unwrap()]).output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success() && out.contains("3 checkpoint(s) reached, 83 approach samples"), "{out}");
    let other = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/human_22730.Ghost.Gbx");
    let o = Command::new(&ghost).args(["lcp", other.to_str().unwrap()]).output().unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("not CGameSaveLaunchedCheckpoints"));
}
