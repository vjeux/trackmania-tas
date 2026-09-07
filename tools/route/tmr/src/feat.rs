//! One door for both feature layouts: `Featurizer::V1` (plumb probes, FEATURES.md)
//! and `Featurizer::V2` (LocalScene rays/path/cells, features2.rs). Rows, models
//! and estimators carry the feature version and ask the featurizer for `dim()`.

use crate::features::{self, Probe, Target};
use crate::features2::{self, Geo2, Target2, TargetKind};
use tmreach::tmr::CarState;

/// What a target is, independent of the layout.
#[derive(Clone, Debug)]
pub struct TargetSpec {
    pub centre: [f32; 3],
    pub normal: [f32; 3],
    pub half_width: f32,
    pub group_size: u32,
    pub kind: TargetKind,
    pub collected_share: f32,
}

pub enum Featurizer<'a> {
    V1(Probe<'a>),
    V2(Geo2<'a>),
}

impl<'a> Featurizer<'a> {
    pub fn version(&self) -> u32 {
        match self {
            Featurizer::V1(_) => features::FEATURE_VERSION,
            Featurizer::V2(_) => features2::FEATURE_VERSION_2,
        }
    }
    pub fn dim(&self) -> usize {
        dim_of(self.version())
    }
    pub fn fill(&self, s: &CarState, t: &TargetSpec, h: u16, out: &mut [f32]) {
        match self {
            Featurizer::V1(p) => features::features(s, &Target { centre: t.centre, normal: t.normal, half_width: t.half_width, group_size: t.group_size }, p, h, out),
            Featurizer::V2(g) => features2::features2(s, &Target2 { centre: t.centre, normal: t.normal, half_width: t.half_width, group_size: t.group_size, kind: t.kind, collected_share: t.collected_share }, g, h, out),
        }
    }
}

pub fn dim_of(fv: u32) -> usize {
    match fv {
        1 => features::DIM,
        2 => features2::DIM2,
        _ => panic!("feature version {fv} unknown"),
    }
}

pub fn mask_blocks(fv: u32, x: &mut [f32], keep: &[&str]) {
    match fv {
        1 => features::mask_blocks(x, keep),
        _ => features2::mask_blocks2(x, keep),
    }
}

pub fn ablation_keep(fv: u32, name: &str) -> Option<Vec<&'static str>> {
    match fv {
        1 => features::ablation_keep(name),
        _ => features2::ablation_keep2(name),
    }
}

/// Mirror augmentation exists for v1 only (v2's ray/path layout has no cheap mirror yet).
pub fn mirror(fv: u32, x: &mut [f32]) -> bool {
    if fv == 1 {
        features::mirror(x);
        true
    } else {
        false
    }
}

/// Column ranges that are GEOMETRY (the block-dropout target).
pub fn geometry_ranges(fv: u32) -> Vec<(usize, usize)> {
    match fv {
        1 => vec![(features::OFF_PROBES, features::DIM)],
        _ => vec![(features2::OFF2_PATH, features2::OFF2_TARGET), (features2::OFF2_CHORD, features2::DIM2)],
    }
}
