//! The engine's class hierarchy, as far as the pak cipher needs it.
//!
//! While the game reads an ENCRYPTED, UNCOMPRESSED pak file it "dummy-writes"
//! into the cipher: at the start of every node body (the main node's, and each
//! inline node's, right after its class id) it folds the four bytes of the
//! node's PARENT class id into the Blowfish stream's IV perturbation (GBX.NET:
//! `GbxReader.TryInitializeDecryption` -> `BlowfishStream.Initialize`). The
//! perturbation lands at the next 0x100-byte boundary of the stream, and the
//! IV chains through every later block, so a reader that skips it decodes the
//! first 256 bytes of such a file and garbage after — which is how every
//! `.Light.Gbx` of the Stadium pack (all 73 of them, 275-485 bytes) read until
//! 2026-09-07. Files flagged `DontUseDummyWrite` (pak entry flag bit 32) and
//! LZ4-compressed ones are exempt.
//!
//! The table is GBX.NET's `inherits:` lines (its chunkl corpus, 251 classes);
//! a class without an entry falls back on the engine's own layout: a Plug
//! class (0x09xxxxxx) derives from `CPlug` (0x0902B000), everything else from
//! `CMwNod` (0x01001000). GBX.NET's own special cases (its hierarchy differs
//! from the engine's in a few places) are folded in below the table.

pub const C_MWNOD: u32 = 0x01001000;
pub const C_PLUG: u32 = 0x0902B000;

/// (class, parent) — from GBX.NET's chunkl `inherits:` lines.
const PARENTS: &[(u32, u32)] = &[
    (0x01031000, 0x01052000), // CMwCmdInst : CMwCmdScript
    (0x01032000, 0x01031000), // CMwCmdAffectIdent : CMwCmdInst
    (0x01038000, 0x01052000), // CMwCmdExp : CMwCmdScript
    (0x01052000, 0x01005000), // CMwCmdScript : CMwCmd
    (0x01056000, 0x01038000), // CMwCmdExpClass : CMwCmdExp
    (0x01057000, 0x01038000), // CMwCmdExpNum : CMwCmdExp
    (0x01058000, 0x01057000), // CMwCmdExpNumConst : CMwCmdExpNum
    (0x0105B000, 0x01052000), // CMwCmdBlock : CMwCmdScript
    (0x0105C000, 0x01052000), // CMwCmdBlockCast : CMwCmdScript
    (0x03028000, 0x03028000), // CGameCtnMediaBlockFx : CGameCtnMediaBlock
    (0x0302B000, 0x03028000), // CGameCtnMediaBlockFxColors : CGameCtnMediaBlockFx
    (0x03030000, 0x03028000), // CGameCtnMediaBlockDOF : CGameCtnMediaBlockFx
    (0x0303F000, 0x03028000), // CGameCtnMediaBlockFxBloom : CGameCtnMediaBlockFx
    (0x03043000, 0x0301A000), // CGameCtnChallenge : CGameCtnCollector
    (0x03053000, 0x03028000), // CGameCtnMediaBlockFxBlur : CGameCtnMediaBlockFx
    (0x03054000, 0x03028000), // CGameCtnMediaBlockFxBlurDepth : CGameCtnMediaBlockFxBlur
    (0x03055000, 0x03028000), // CGameCtnMediaBlockFxBlurMotion : CGameCtnMediaBlockFxBlur
    (0x0305D000, 0x0301C000), // CGameCtnZoneFlat : CGameCtnZone
    (0x0305E000, 0x0301C000), // CGameCtnZoneFrontier : CGameCtnZone
    (0x0305F000, 0x0301C000), // CGameCtnZoneTransition : CGameCtnZone
    (0x03062000, 0x0301C000), // CGameCtnZoneGenealogy : CGameCtnZone
    (0x03066000, 0x0301A000), // CGameCtnMacroBlockInfo : CGameCtnCollector
    (0x0306A000, 0x0301A000), // CGameCtnArticleGroup : CGameCtnCollector
    (0x0306B000, 0x0301A000), // CGameCtnDecorationSize : CGameCtnCollector
    (0x0306D000, 0x03078000), // CGameCtnMediaBlockFxCameraBlend : CGameCtnMediaBlockFx
    (0x03072000, 0x03078000), // CGameCtnMediaBlockFxBloom2 : CGameCtnMediaBlockFx
    (0x0307D000, 0x03078000), // CGameCtnMediaBlockFxLensFlare : CGameCtnMediaBlockFx
    (0x0307E000, 0x03078000), // CGameCtnMediaBlockFxSaturation : CGameCtnMediaBlockFx
    (0x03080000, 0x03078000), // CGameCtnMediaBlockFxTone : CGameCtnMediaBlockFx
    (0x03084000, 0x03028000), // CGameCtnMediaBlockCameraCustom : CGameCtnMediaBlockCamera
    (0x030A2000, 0x03028000), // CGameCtnMediaBlockTriangles2D : CGameCtnMediaBlockTriangles
    (0x030A3000, 0x03028000), // CGameCtnMediaBlockTriangles3D : CGameCtnMediaBlockTriangles
    (0x030A4000, 0x03028000), // CGameCtnMediaBlockCameraPath : CGameCtnMediaBlockCamera
    (0x030A5000, 0x03028000), // CGameCtnMediaBlockCameraEffectShake : CGameCtnMediaBlockCameraEffect
    (0x030A6000, 0x03028000), // CGameCtnMediaBlockCameraEffectScript : CGameCtnMediaBlockCameraEffect
    (0x030A7000, 0x03028000), // CGameCtnMediaBlockTransitionFade : CGameCtnMediaBlockFx
    (0x030A8000, 0x03028000), // CGameCtnMediaBlockCameraGame : CGameCtnMediaBlockCamera
    (0x030A9000, 0x03028000), // CGameCtnMediaBlockCameraOrbital : CGameCtnMediaBlockCamera
    (0x030E0000, 0x03028000), // CGameCtnMediaBlockDirtyLens : CGameCtnMediaBlockFx
    (0x03165000, 0x03028000), // CGameCtnMediaBlockFxGrain : CGameCtnMediaBlockFx
    (0x03166000, 0x03028000), // CGameCtnMediaBlockFxTone2 : CGameCtnMediaBlockFx
    (0x03167000, 0x03028000), // CGameCtnMediaBlockFxTone3 : CGameCtnMediaBlockFx
    (0x0316A000, 0x03028000), // CGameCtnMediaBlockTrails : CGameCtnMediaBlockFx
    (0x0316B000, 0x03028000), // CGameCtnMediaBlockShoot : CGameCtnMediaBlockFx
    (0x0316C000, 0x03028000), // CGameCtnMediaBlockFog : CGameCtnMediaBlockFx
    (0x0316D000, 0x03028000), // CGameCtnMediaBlockFxDirtyLens : CGameCtnMediaBlockFx
    (0x0316E000, 0x03028000), // CGameCtnMediaBlockManialink : CGameCtnMediaBlockFx
    (0x03196000, 0x03028000), // CGameCtnMediaBlockOpponentVisibility : CGameCtnMediaBlockFx
    (0x03197000, 0x03028000), // CGameCtnMediaBlockToneMapping : CGameCtnMediaBlockFx
    (0x03198000, 0x03028000), // CGameCtnMediaBlockFxSSAO : CGameCtnMediaBlockFx
    (0x03199000, 0x03028000), // CGameCtnMediaBlockEntity : CGameCtnMediaBlock
    (0x0319A000, 0x03028000), // CGameCtnMediaBlockBloomHdr : CGameCtnMediaBlockFx
    (0x0319B000, 0x03028000), // CGameCtnMediaBlockColorGrading : CGameCtnMediaBlockFx
    (0x0319C000, 0x03028000), // CGameCtnMediaBlockTimeSpeed : CGameCtnMediaBlockFx
    (0x0319D000, 0x03028000), // CGameCtnMediaBlockSound2 : CGameCtnMediaBlockSound
    (0x0319E000, 0x03028000), // CGameCtnMediaBlockInterface : CGameCtnMediaBlockFx
    (0x0319F000, 0x03028000), // CGameCtnMediaBlockFxDirtyLens2 : CGameCtnMediaBlockFx
    (0x031A0000, 0x03028000), // CGameCtnMediaBlockTrails2 : CGameCtnMediaBlockFx
    (0x031A1000, 0x03028000), // CGameCtnMediaBlockFxVertigo : CGameCtnMediaBlockFx
    (0x031A2000, 0x03028000), // CGameCtnMediaBlockFxFisheye : CGameCtnMediaBlockFx
    (0x031A3000, 0x03028000), // CGameCtnMediaBlockFxWaterDrops : CGameCtnMediaBlockFx
    (0x031A4000, 0x03028000), // CGameCtnMediaBlockFxDistortion : CGameCtnMediaBlockFx
    (0x031A5000, 0x03028000), // CGameCtnMediaBlockFxDazzle : CGameCtnMediaBlockFx
    (0x031A6000, 0x03028000), // CGameCtnMediaBlockFxSharpen : CGameCtnMediaBlockFx
    (0x031A7000, 0x03028000), // CGameCtnMediaBlockFxMotionBlur : CGameCtnMediaBlockFx
    (0x031A8000, 0x03028000), // CGameCtnMediaBlockFxLensFlare2 : CGameCtnMediaBlockFx
    (0x031A9000, 0x03028000), // CGameCtnMediaBlockFxChromaticAberration : CGameCtnMediaBlockFx
    (0x031AA000, 0x03028000), // CGameCtnMediaBlockFxFilmGrain : CGameCtnMediaBlockFx
    (0x031AB000, 0x03028000), // CGameCtnMediaBlockFxVignette : CGameCtnMediaBlockFx
    (0x031AC000, 0x03028000), // CGameCtnMediaBlockFxTiltShift : CGameCtnMediaBlockFx
    (0x031AD000, 0x03028000), // CGameCtnMediaBlockFxColorGrading2 : CGameCtnMediaBlockFx
    (0x031AE000, 0x03028000), // CGameCtnMediaBlockFxDepthOfField : CGameCtnMediaBlockFx
    (0x031AF000, 0x03028000), // CGameCtnMediaBlockFxBloom3 : CGameCtnMediaBlockFx
    (0x031B0000, 0x03028000), // CGameCtnMediaBlockFxLensDirt : CGameCtnMediaBlockFx
    (0x031B1000, 0x03028000), // CGameCtnMediaBlockFxRainbow : CGameCtnMediaBlockFx
    (0x031B2000, 0x03028000), // CGameCtnMediaBlockFxSpeedLines : CGameCtnMediaBlockFx
    (0x04002000, 0x04003000), // GxLightBall : GxLightPoint
    (0x04003000, 0x04006000), // GxLightPoint : GxLightNotAmbient
    (0x04005000, 0x04001000), // GxLightAmbient : GxLight
    (0x04006000, 0x04001000), // GxLightNotAmbient : GxLight
    (0x04007000, 0x04006000), // GxLightDirectional : GxLightNotAmbient
    (0x0400A000, 0x04002000), // GxLightFrustum : GxLightBall
    (0x0400B000, 0x04002000), // GxLightSpot : GxLightBall
    (0x0500B000, 0x05002000), // CFuncPlug : CFuncShader
    (0x05015000, 0x05002000), // CFuncShaderLayerUV : CFuncShader
    (0x05016000, 0x05002000), // CFuncShaderLayerUVCubeMap : CFuncShader
    (0x0501F000, 0x05002000), // CFuncKeysReal : CFuncKeys
    (0x05020000, 0x05002000), // CFuncClouds : CFuncShader
    (0x07031000, 0x07001000), // CControlFrame : CControlContainer
    (0x0900E000, 0x0902B000), // CPlugShaderApply : CPlugShader
    (0x0901D000, 0x0902B000), // CPlugLight : CPlug
    (0x0901E000, 0x0906A000), // CPlugVisualIndexedTriangles : CPlugVisualIndexed
    (0x0902C000, 0x09006000), // CPlugVisual3D : CPlugVisual
    (0x0903A000, 0x0902B000), // CPlugMaterialCustom : CPlug
    (0x09051000, 0x0902B000), // CPlugTreeGenerator : CPlug
    (0x09056000, 0x0902B000), // CPlugVertexStream : CPlug
    (0x09057000, 0x0902B000), // CPlugIndexBuffer : CPlug
    (0x0906A000, 0x0902C000), // CPlugVisualIndexed : CPlugVisual3D
    (0x09079000, 0x0902B000), // CPlugMaterial : CPlug
    (0x090BB000, 0x0902B000), // CPlugSolid2Model : CPlug
    (0x090FD000, 0x0902B000), // CPlugMaterialUserInst : CPlug
    (0x0A02B000, 0x0A02B000), // CSceneVehicleCar : CSceneVehicle
    (0x24005000, 0x24005000), // CGameCtnBlockInfo : CGameCtnCollector
];

/// The class id the game folds into the cipher at the start of this class's
/// node body — `None` for a class the table does not know: the fold happens
/// only for a class with a declared parent (GBX.NET folds nothing when
/// `GetParentClassId` is null). Measured 2026-09-07 on
/// `ObstaclePusher8mPiston.DynaObject.Gbx` (CPlugDynaObjectModel, 278 bytes,
/// raw, dummy-written, no inline node): its bytes past 0x100 are right with NO
/// fold (LocAnim -1, WaterModel -1, zeros) and garbage with the CPlug fallback
/// fold, while `ItemLampSpot.Light.Gbx` (CPlugLight : CPlug in the table)
/// needs its fold. The old fallback (CPlug for 0x09xxxxxx, CMwNod otherwise)
/// corrupted every table-less raw class past 0x100.
/// Every class id the table mentions (children and parents) plus the two
/// roots — the candidate set of a fold hunt.
pub fn known_classes() -> Vec<u32> {
    let mut v: Vec<u32> = PARENTS.iter().flat_map(|(c, p)| [*c, *p]).collect();
    v.extend([C_MWNOD, C_PLUG, 0x2401C000, 0x24005000, 0x07001000]);
    v.sort();
    v.dedup();
    v
}

pub fn dummy_write_class(class_id: u32) -> Option<u32> {
    // GBX.NET's own overrides where its hierarchy differs from the engine's
    match class_id {
        0x07031000 => return Some(0x07001000),          // CControlFrame
        0x0A02E000 => return Some(0x0A02E000),          // CPlugVehiclePhyTuning (its own id)
        0x0501F000 => return Some(C_MWNOD),             // CFuncKeysReal ("weird case of CPlugCurveSimpleNod")
        0x0305D000 | 0x0305E000 | 0x0305F000 | 0x03062000 => return Some(0x2401C000), // CGameCtnZone*
        0x03051000 | 0x0304E000 | 0x0304F000 | 0x03050000 => return Some(0x24005000), // CGameCtnBlockInfo*
        _ => {}
    }
    PARENTS.iter().find(|(c, _)| *c == class_id).map(|(_, p)| *p)
}
