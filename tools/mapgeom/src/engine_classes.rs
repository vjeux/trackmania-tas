//! The engine's class tables, read off `Trackmania.exe` (45 467 720 B, md5
//! 4a28c00429c6f75c894cf7bc4378a8a2, the Aug 2025 client) by `asmdig`:
//!
//! ```text
//! objdump -d -b pei-x86-64 -M intel --wide -j .text Trackmania.exe > tm.asm
//! asmdig classtree tm.asm Trackmania.exe 1402d52e0 1402ea9e0 --rust   → ENGINE_CLASSES
//! asmdig cmptree   tm.asm Trackmania.exe 1402f3570 edx --rust          → REMAP
//! asmdig cmptree   tm.asm Trackmania.exe 1402f2610 edx --rust          → NORMALISE
//! asmdig vtables   tm.asm Trackmania.exe 1402d0720 --reg2 1402ea9e0 --rust → NO_FOLD_CLASSES
//! ```
//!
//! `ENGINE_CLASSES` is every `CMwClassInfo::Register` call (two spellings:
//! `Register(&info, id, &parentInfo, "Name", …)` at 0x1402d52e0 and
//! `Register2(id, "Name", size, isNod, parentId, …)` at 0x1402ea9e0) as
//! (class id, parent class id); parent 0 = no parent (CMwNod and the
//! primitive types). `REMAP` (0x1402f3570) is the id the engine WRITES for
//! a class — the 161 CGame `0x03xxxxxx` ids that files carry as
//! `0x24xxxxxx`; `NORMALISE` (0x1402f2610) is the read-side inverse plus
//! the legacy aliases (`0x0301A000 → 0x2E001000` CGameCtnCollector, the
//! `0x0805xxxx → 0x090Bxxxx` particle classes, …): what a class id read
//! from a file goes through before the class registry lookup
//! (`0x1402f20a0`). The pak cipher's dummy write (`parents.rs`) needs all
//! three. Generated 2026-09-23; regenerate with the commands above when the
//! exe changes.

/// (class id, parent class id) — the engine's hierarchy, sorted by class.
pub const ENGINE_CLASSES: &[(u32, u32)] = &[
    (0x01001000, 0x00000000), // CMwNod : -
    (0x01003000, 0x01001000), // CMwEngine : CMwNod
    (0x01004000, 0x01003000), // CMwEngineMain : CMwEngine
    (0x01005000, 0x01001000), // CMwCmd : CMwNod
    (0x01012000, 0x01005000), // CMwCmdFastCall : CMwCmd
    (0x0101C000, 0x01001000), // CMwCmdBuffer : CMwNod
    (0x0101E000, 0x01005000), // CMwCmdFiber : CMwCmd
    (0x01020000, 0x01001000), // CMwCmdBufferCore : CMwNod
    (0x01022000, 0x01001000), // CMwClassInfoViewer : CMwNod
    (0x01026000, 0x01001000), // CMwRefBuffer : CMwNod
    (0x01029000, 0x01001000), // CMwStatsValue : CMwNod
    (0x01037000, 0x00000000), // Bool : -
    (0x01038000, 0x00000000), // Real : -
    (0x01039000, 0x00000000), // GmVec2 : -
    (0x0103A000, 0x00000000), // GmVec3 : -
    (0x0103B000, 0x00000000), // GmVec4 : -
    (0x0103C000, 0x00000000), // GmInt2 : -
    (0x0103D000, 0x00000000), // GmInt3 : -
    (0x0103E000, 0x00000000), // GmNat2 : -
    (0x0103F000, 0x00000000), // GmNat3 : -
    (0x01040000, 0x00000000), // GmQuat : -
    (0x01041000, 0x00000000), // GmIso4 : -
    (0x01042000, 0x00000000), // Nat : -
    (0x01043000, 0x00000000), // Nat8 : -
    (0x01044000, 0x00000000), // Nat16 : -
    (0x01045000, 0x00000000), // Int : -
    (0x01046000, 0x00000000), // Int8 : -
    (0x01047000, 0x00000000), // Int16 : -
    (0x01048000, 0x00000000), // CMwId : -
    (0x01049000, 0x00000000), // CFastString : -
    (0x0104A000, 0x00000000), // CFastStringInt : -
    (0x0104B000, 0x00000000), // Enum8 : -
    (0x0104C000, 0x00000000), // Enum16 : -
    (0x0104D000, 0x00000000), // Enum32 : -
    (0x0104E000, 0x00000000), // Vec3Color : -
    (0x0104F000, 0x00000000), // GmIso3 : -
    (0x01050000, 0x00000000), // NatRange : -
    (0x01051000, 0x00000000), // IntRange : -
    (0x01052000, 0x00000000), // RealRange : -
    (0x01053000, 0x00000000), // GxRGBAColor : -
    (0x01093000, 0x01001000), // CMwCmdContainer : CMwNod
    (0x010C0000, 0x01005000), // CMwCmdFastCallUser : CMwCmd
    (0x010C4000, 0x01005000), // CMwCmdFastCallStatic : CMwCmd
    (0x010C5000, 0x01005000), // CMwCmdFastCallStaticParam : CMwCmd
    (0x03000000, 0x01003000), // CGameEngine : CMwEngine
    (0x03001000, 0x03008000), // CGameManiaTitle : CGameNod
    (0x03002000, 0x01001000), // CGamePlayer : CMwNod
    (0x03003000, 0x01001000), // CGameTerminal : CMwNod
    (0x03005000, 0x01001000), // CGameApp : CMwNod
    (0x03006000, 0x12014000), // CGameMasterServer : CNetMasterServer
    (0x03007000, 0x031A4000), // CGameModuleEditorBase : CGameManiaApp
    (0x03008000, 0x01001000), // CGameNod : CMwNod
    (0x03009000, 0x01001000), // CGameMenu : CMwNod
    (0x0300A000, 0x12001000), // CGameNetFormPlaygroundSync : CNetNod
    (0x0300B000, 0x07016000), // CGameMenuFrame : CControlFrame
    (0x0300C000, 0x01001000), // CGameSystemOverlay : CMwNod
    (0x0300D000, 0x03102000), // CGamePlayground : CGameSwitcherModule
    (0x0300E000, 0x01001000), // CGameNetPlayerInfo : CMwNod
    (0x0300F000, 0x01001000), // CGameNetwork : CMwNod
    (0x03010000, 0x0302F000), // CGameNetFormTunnel : CGameNetForm
    (0x03011000, 0x030D2000), // CGameManiaPlanetNetwork : CGameCtnNetwork
    (0x03012000, 0x03008000), // CGameManiaTitleCore : CGameNod
    (0x03013000, 0x030D3000), // CGameManiaPlanet : CGameCtnApp
    (0x03014000, 0x03008000), // CGameStation : CGameNod
    (0x03017000, 0x03106000), // CGameManialinkEntry : CGameManialinkControl
    (0x03019000, 0x07005000), // CGameMenuColorEffect : CControlEffect
    (0x0301B000, 0x01001000), // CGameCtnCollectorList : CMwNod
    (0x0301D000, 0x01001000), // CGameCtnChapter : CMwNod
    (0x0301E000, 0x01001000), // CGameCtnCatalog : CMwNod
    (0x0301F000, 0x01001000), // CGameCtnArticle : CMwNod
    (0x03020000, 0x01001000), // CGameLaunchedCheckpoint : CMwNod
    (0x03021000, 0x0300D000), // CGameCtnPlayground : CGamePlayground
    (0x03023000, 0x1203C000), // CWebServicesTask_PostConnect_PlugInList : CWebServicesTaskSequence
    (0x03024000, 0x03077000), // CGameCtnMediaBlock3dStereo : CGameCtnMediaBlock
    (0x03026000, 0x12027000), // CGameMasterServerRequest : CNetMasterServerRequest
    (0x03027000, 0x01001000), // CGameAvatar : CMwNod
    (0x03028000, 0x01001000), // CGameNetOnlineMessage : CMwNod
    (0x03029000, 0x03077000), // CGameCtnMediaBlockTriangles : CGameCtnMediaBlock
    (0x0302A000, 0x01001000), // CGameRemoteBuffer : CMwNod
    (0x0302B000, 0x01001000), // CGameRemoteBufferPool : CMwNod
    (0x0302C000, 0x01001000), // CGameRemoteBufferDataInfo : CMwNod
    (0x0302D000, 0x01001000), // CGameResources : CMwNod
    (0x0302E000, 0x12015000), // CGameNetServerInfo : CNetMasterHost
    (0x0302F000, 0x12001000), // CGameNetForm : CNetNod
    (0x03030000, 0x01001000), // CGameDialogs : CMwNod
    (0x03032000, 0x07005000), // CGameMenuScaleEffect : CControlEffect
    (0x03033000, 0x01001000), // CGameCtnCollection : CMwNod
    (0x03034000, 0x01001000), // CGameCtnMediaBlockEditor : CMwNod
    (0x03036000, 0x01001000), // CGameCtnBlockUnitInfo : CMwNod
    (0x03037000, 0x01001000), // CGameFid : CMwNod
    (0x03038000, 0x2E001000), // CGameCtnDecoration : CGameCtnCollector
    (0x03039000, 0x01001000), // CGameCtnDecorationAudio : CMwNod
    (0x0303A000, 0x01001000), // CGameCtnDecorationMood : CMwNod
    (0x0303B000, 0x01001000), // CGameCtnDecorationSize : CMwNod
    (0x0303F000, 0x01001000), // CGameGhost : CMwNod
    (0x03041000, 0x0306B000), // CGameControlCameraFirstPerson : -
    (0x03042000, 0x0306B000), // CGameControlCameraThirdPerson : -
    (0x03043000, 0x01001000), // CGameCtnChallenge : CMwNod
    (0x03044000, 0x03037000), // CGameCtnChallengeInfo : CGameFid
    (0x03045000, 0x01001000), // CGameOutlineBox : CMwNod
    (0x03046000, 0x01001000), // CGameCtnParticleParam : CMwNod
    (0x03047000, 0x01001000), // CGameHighScore : CMwNod
    (0x03048000, 0x01001000), // CGameCtnPainterSetting : CMwNod
    (0x03049000, 0x01001000), // CGameLeagueManager : CMwNod
    (0x0304A000, 0x01001000), // CGameCtnMediaBlockEditorTriangles : CMwNod
    (0x0304B000, 0x03029000), // CGameCtnMediaBlockTriangles2D : CGameCtnMediaBlockTriangles
    (0x0304C000, 0x03029000), // CGameCtnMediaBlockTriangles3D : CGameCtnMediaBlockTriangles
    (0x0304D000, 0x01001000), // CGameDisplaySettingsWrapper : CMwNod
    (0x0304E000, 0x2E001000), // CGameCtnBlockInfo : CGameCtnCollector
    (0x0304F000, 0x0304E000), // CGameCtnBlockInfoFlat : CGameCtnBlockInfo
    (0x03050000, 0x0304E000), // CGameCtnBlockInfoFrontier : CGameCtnBlockInfo
    (0x03051000, 0x0304E000), // CGameCtnBlockInfoClassic : CGameCtnBlockInfo
    (0x03052000, 0x0304E000), // CGameCtnBlockInfoRoad : CGameCtnBlockInfo
    (0x03053000, 0x0304E000), // CGameCtnBlockInfoClip : CGameCtnBlockInfo
    (0x03054000, 0x0304E000), // CGameCtnBlockInfoSlope : CGameCtnBlockInfo
    (0x03055000, 0x0304E000), // CGameCtnBlockInfoPylon : CGameCtnBlockInfo
    (0x03056000, 0x0304E000), // CGameCtnBlockInfoRectAsym : CGameCtnBlockInfo
    (0x03057000, 0x01001000), // CGameCtnBlock : CMwNod
    (0x03058000, 0x01001000), // CGameCtnBlockUnit : CMwNod
    (0x03059000, 0x01001000), // CGameCtnBlockSkin : CMwNod
    (0x0305A000, 0x01001000), // CGameCtnPylonColumn : CMwNod
    (0x0305B000, 0x01001000), // CGameCtnChallengeParameters : CMwNod
    (0x0305C000, 0x01001000), // CGameCtnZone : CMwNod
    (0x0305D000, 0x0305C000), // CGameCtnZoneFlat : CGameCtnZone
    (0x0305E000, 0x0305C000), // CGameCtnZoneFrontier : CGameCtnZone
    (0x0305F000, 0x03008000), // CGameSkinnedNod : CGameNod
    (0x03060000, 0x01001000), // CGameCtnMediaShootParams : CMwNod
    (0x03061000, 0x01001000), // CGameScoreLoaderAndSynchronizer : CMwNod
    (0x03063000, 0x03336000), // CWebServicesTaskResult_SeasonScript : CWebServicesTaskResult_Season
    (0x03064000, 0x1203D000), // CWebServicesTaskResult_SeasonList : CWebServicesTaskResult
    (0x03065000, 0x03064000), // CWebServicesTaskResult_SeasonListScript : CWebServicesTaskResult_SeasonList
    (0x03066000, 0x01001000), // CGameManialinkBrowser : CMwNod
    (0x03067000, 0x12001000), // CGameNetFormAdmin : CNetNod
    (0x03068000, 0x12018000), // CGameNetFileTransfer : CNetFileTransfer
    (0x03069000, 0x12004000), // CGameNetFormTimeSync : CNetFormTimed
    (0x0306A000, 0x0302F000), // CGameNetFormCallVote : CGameNetForm
    (0x0306B000, 0x01001000), // CGameControlCamera : CMwNod
    (0x0306D000, 0x0306B000), // CGameControlCameraFree : -
    (0x0306E000, 0x03072000), // CGameControlCameraOrbital3d : -
    (0x0306F000, 0x01001000), // CGameControlCameraEffect : CMwNod
    (0x03071000, 0x0306F000), // CGameControlCameraEffectShake : CGameControlCameraEffect
    (0x03072000, 0x0306B000), // CGameControlCameraTarget : -
    (0x03076000, 0x01001000), // CGameLadderRanking : CMwNod
    (0x03077000, 0x01001000), // CGameCtnMediaBlock : CMwNod
    (0x03078000, 0x01001000), // CGameCtnMediaTrack : CMwNod
    (0x03079000, 0x01001000), // CGameCtnMediaClip : CMwNod
    (0x0307A000, 0x01001000), // CGameCtnMediaClipGroup : CMwNod
    (0x0307C000, 0x03077000), // CGameCtnMediaBlockCamera : CGameCtnMediaBlock
    (0x0307D000, 0x03077000), // CGameCtnMediaBlockUi : CGameCtnMediaBlock
    (0x0307E000, 0x03077000), // CGameCtnMediaBlockFx : CGameCtnMediaBlock
    (0x0307F000, 0x0307E000), // CGameCtnMediaBlockFxBlur : CGameCtnMediaBlockFx
    (0x03080000, 0x0307E000), // CGameCtnMediaBlockFxColors : CGameCtnMediaBlockFx
    (0x03081000, 0x0307F000), // CGameCtnMediaBlockFxBlurDepth : CGameCtnMediaBlockFxBlur
    (0x03082000, 0x0307F000), // CGameCtnMediaBlockFxBlurMotion : CGameCtnMediaBlockFxBlur
    (0x03083000, 0x0307E000), // CGameCtnMediaBlockFxBloom : CGameCtnMediaBlockFx
    (0x03084000, 0x0307C000), // CGameCtnMediaBlockCameraGame : CGameCtnMediaBlockCamera
    (0x03085000, 0x03077000), // CGameCtnMediaBlockTime : CGameCtnMediaBlock
    (0x03086000, 0x01001000), // CGameCtnMediaClipPlayer : CMwNod
    (0x03087000, 0x03077000), // CGameCtnMediaBlockEvent_deprecated : CGameCtnMediaBlock
    (0x03088000, 0x03321000), // CWebServicesTaskResult_AccountTrophyGainHistoryScript : CWebServicesTaskResult_AccountTrophyGainHistory
    (0x03089000, 0x01001000), // CGameManiaNetResource : CMwNod
    (0x0308A000, 0x0300E000), // CGamePlayerInfo : CGameNetPlayerInfo
    (0x0308B000, 0x01001000), // CGameClientTrackingScript : CMwNod
    (0x0308C000, 0x01001000), // CGamePlayerProfile : CMwNod
    (0x0308D000, 0x01001000), // CGameScriptDebugger : CMwNod
    (0x0308E000, 0x01001000), // CGameLeague : CMwNod
    (0x0308F000, 0x01001000), // CGameCtnChallengeGroup : CMwNod
    (0x03090000, 0x01001000), // CGameCtnCampaign : CMwNod
    (0x03092000, 0x0303F000), // CGameCtnGhost : CGameGhost
    (0x03093000, 0x01001000), // CGameCtnReplayRecord : CMwNod
    (0x03094000, 0x03037000), // CGameCtnReplayRecordInfo : CGameFid
    (0x03096000, 0x03076000), // CGameLadderRankingLeague : CGameLadderRanking
    (0x03097000, 0x03076000), // CGameLadderRankingPlayer : CGameLadderRanking
    (0x03099000, 0x03076000), // CGameLadderRankingSkill : CGameLadderRanking
    (0x0309A000, 0x07016000), // CGameControlCard : CControlFrame
    (0x0309B000, 0x01001000), // CGameControlCardManager : CMwNod
    (0x0309C000, 0x01001000), // CGameControlDataType : CMwNod
    (0x0309F000, 0x0307C000), // CGameCtnMediaBlockCameraSimple : CGameCtnMediaBlockCamera
    (0x030A0000, 0x0307C000), // CGameCtnMediaBlockCameraOrbital : CGameCtnMediaBlockCamera
    (0x030A1000, 0x0307C000), // CGameCtnMediaBlockCameraPath : CGameCtnMediaBlockCamera
    (0x030A2000, 0x0307C000), // CGameCtnMediaBlockCameraCustom : CGameCtnMediaBlockCamera
    (0x030A3000, 0x03077000), // CGameCtnMediaBlockCameraEffect : CGameCtnMediaBlock
    (0x030A4000, 0x030A3000), // CGameCtnMediaBlockCameraEffectShake : CGameCtnMediaBlockCameraEffect
    (0x030A5000, 0x03077000), // CGameCtnMediaBlockImage : CGameCtnMediaBlock
    (0x030A6000, 0x03077000), // CGameCtnMediaBlockMusicEffect : CGameCtnMediaBlock
    (0x030A7000, 0x03077000), // CGameCtnMediaBlockSound : CGameCtnMediaBlock
    (0x030A8000, 0x03077000), // CGameCtnMediaBlockText : CGameCtnMediaBlock
    (0x030A9000, 0x03077000), // CGameCtnMediaBlockTrails : CGameCtnMediaBlock
    (0x030AA000, 0x03077000), // CGameCtnMediaBlockTransition : CGameCtnMediaBlock
    (0x030AB000, 0x030AA000), // CGameCtnMediaBlockTransitionFade : CGameCtnMediaBlockTransition
    (0x030AD000, 0x03102000), // CGameCtnMediaClipViewer : CGameSwitcherModule
    (0x030AE000, 0x01001000), // CGameCursorBlock : CMwNod
    (0x030AF000, 0x03230000), // CGameCtnEditor : CGameEditorParent
    (0x030B3000, 0x01001000), // CGameCtnEdControlCam : CMwNod
    (0x030B4000, 0x030B3000), // CGameCtnEdControlCamCustom : CGameCtnEdControlCam
    (0x030B5000, 0x030B3000), // CGameCtnEdControlCamPath : CGameCtnEdControlCam
    (0x030B9000, 0x07015000), // CGameControlGrid : CControlGrid
    (0x030BA000, 0x030B9000), // CGameControlGridCard : CGameControlGrid
    (0x030BB000, 0x0302E000), // CGameCtnNetServerInfo : CGameNetServerInfo
    (0x030BC000, 0x0309A000), // CGameControlCardCtnChallengeInfo : CGameControlCard
    (0x030BD000, 0x0309A000), // CGameControlCardGeneric : CGameControlCard
    (0x030BE000, 0x0309A000), // CGameControlCardLeague : CGameControlCard
    (0x030BF000, 0x0309A000), // CGameControlCardCtnNetServerInfo : CGameControlCard
    (0x030C0000, 0x03077000), // CGameCtnMediaBlockLightmap : CGameCtnMediaBlock
    (0x030C1000, 0x0309A000), // CGameControlCardLadderRanking : CGameControlCard
    (0x030C2000, 0x0309A000), // CGameControlCardMessage : CGameControlCard
    (0x030C7000, 0x0309A000), // CGameControlCardProfile : CGameControlCard
    (0x030C8000, 0x0309A000), // CGameControlCardCtnReplayRecordInfo : CGameControlCard
    (0x030C9000, 0x03102000), // CGameCtnMenus : CGameSwitcherModule
    (0x030CA000, 0x03076000), // CGameLadderRankingCtnChallengeAchievement : CGameLadderRanking
    (0x030CB000, 0x0302F000), // CGameCtnNetForm : CGameNetForm
    (0x030CC000, 0x0302C000), // CGameRemoteBufferDataInfoFinds : CGameRemoteBufferDataInfo
    (0x030CD000, 0x01001000), // CGameUserProfileWrapper_VehicleSettings : CMwNod
    (0x030CE000, 0x0302C000), // CGameRemoteBufferDataInfoSearchs : CGameRemoteBufferDataInfo
    (0x030CF000, 0x01001000), // CGameMgrActionFxPhy : CMwNod
    (0x030D0000, 0x030F0000), // CGameScriptHandlerStation : CGameManialinkScriptHandler
    (0x030D1000, 0x03006000), // CGameCtnMasterServer : CGameMasterServer
    (0x030D2000, 0x0300F000), // CGameCtnNetwork : CGameNetwork
    (0x030D3000, 0x03005000), // CGameCtnApp : CGameApp
    (0x030D5000, 0x0309A000), // CGameControlCardCtnArticle : CGameControlCard
    (0x030D7000, 0x01001000), // CGameManiaPlanetScriptAPI : CMwNod
    (0x030D8000, 0x0309A000), // CGameControlCardCtnChapter : CGameControlCard
    (0x030DA000, 0x0309A000), // CGameControlCardCtnGhostInfo : CGameControlCard
    (0x030DD000, 0x0309A000), // CGameControlCardCtnVehicle : CGameControlCard
    (0x030E0000, 0x01001000), // CGameScriptDebuggerWorkspace : CMwNod
    (0x030E1000, 0x01001000), // CGameAnalyzer : CMwNod
    (0x030E2000, 0x01001000), // CGamePlaygroundInterface : CMwNod
    (0x030E3000, 0x01001000), // CGamePlaygroundSpectating : CMwNod
    (0x030E4000, 0x1203D000), // CWebServicesTaskResult_AccountTrophyGainList : CWebServicesTaskResult
    (0x030E5000, 0x03077000), // CGameCtnMediaBlockGhostTM : CGameCtnMediaBlock
    (0x030E6000, 0x01001000), // CGameEnvironmentManager : CMwNod
    (0x030E7000, 0x01001000), // CGameDialogShootParams : CMwNod
    (0x030E8000, 0x03017000), // CGameManialinkFileEntry : CGameManialinkEntry
    (0x030E9000, 0x01001000), // CGameNetDataDownload : CMwNod
    (0x030EB000, 0x03077000), // CGameCtnMediaBlockSpectators : CGameCtnMediaBlock
    (0x030EC000, 0x12001000), // CGameNetFormBuddy : CNetNod
    (0x030ED000, 0x030F0000), // CGameScriptHandlerPlaygroundInterface : CGameManialinkScriptHandler
    (0x030EE000, 0x031BC000), // CGameManiaAppStation : CGameManiaAppMinimal
    (0x030F0000, 0x01001000), // CGameManialinkScriptHandler : CMwNod
    (0x030F3000, 0x07016000), // CGamePlaygroundControlScores : CControlFrame
    (0x030F4000, 0x03106000), // CGameManialinkMediaPlayer : CGameManialinkControl
    (0x030F5000, 0x0302F000), // CGameNetFormPlayground : CGameNetForm
    (0x030F6000, 0x01001000), // CGameCtnArticleNode : CMwNod
    (0x030F7000, 0x01001000), // CGameSwitcher : CMwNod
    (0x030F9000, 0x03106000), // CGameManialinkOldTable : CGameManialinkControl
    (0x030FA000, 0x030C9000), // CGameCtnMenusManiaPlanet : CGameCtnMenus
    (0x030FB000, 0x03106000), // CGameManialinkLabel : CGameManialinkControl
    (0x030FF000, 0x01001000), // CGameUILayer : CMwNod
    (0x03100000, 0x03021000), // CGamePlaygroundCommon : CGameCtnPlayground
    (0x03101000, 0x01001000), // CGameCtnAnchoredObject : CMwNod
    (0x03102000, 0x01001000), // CGameSwitcherModule : CMwNod
    (0x03103000, 0x01001000), // CGamePlaygroundUIConfig : CMwNod
    (0x03104000, 0x03106000), // CGameManialinkFrame : CGameManialinkControl
    (0x03105000, 0x01001000), // CGameManialinkPage : CMwNod
    (0x03106000, 0x01001000), // CGameManialinkControl : CMwNod
    (0x03109000, 0x03106000), // CGameManialinkQuad : CGameManialinkControl
    (0x0310B000, 0x01001000), // CGameManiaPlanetMenuStations : CMwNod
    (0x0310C000, 0x01001000), // CGameCtnAnchorPoint : CMwNod
    (0x0310D000, 0x2E001000), // CGameCtnMacroBlockInfo : CGameCtnCollector
    (0x0310E000, 0x030AF000), // CGameCtnEditorCommon : CGameCtnEditor
    (0x0310F000, 0x0310E000), // CGameCtnEditorFree : CGameCtnEditorCommon
    (0x03110000, 0x0310F000), // CGameCtnEditorPuzzle : CGameCtnEditorFree
    (0x03111000, 0x01001000), // CGamePlaygroundScore : CMwNod
    (0x03112000, 0x01001000), // CGameCtnEditorCommonInterface : CMwNod
    (0x03114000, 0x2E001000), // CGameCtnMacroDecals : CGameCtnCollector
    (0x03115000, 0x03110000), // CGameCtnEditorSimple : CGameCtnEditorPuzzle
    (0x03116000, 0x030F6000), // CGameCtnArticleNodeDirectory : CGameCtnArticleNode
    (0x03117000, 0x030F6000), // CGameCtnArticleNodeArticle : CGameCtnArticleNode
    (0x0311C000, 0x03106000), // CGameManialinkArrow : CGameManialinkControl
    (0x0311D000, 0x01001000), // CGameCtnZoneGenealogy : CMwNod
    (0x0311E000, 0x01001000), // CGameServerPlugin : CMwNod
    (0x0311F000, 0x11005000), // CGameServerPluginEvent : CScriptBaseConstEvent
    (0x03120000, 0x01001000), // CGameCtnAutoTerrain : CMwNod
    (0x03121000, 0x01001000), // CGameCtnSolidDecals : CMwNod
    (0x03122000, 0x01001000), // CGameCtnBlockInfoMobil : CMwNod
    (0x03123000, 0x01001000), // CGameConnectedClient : CMwNod
    (0x03124000, 0x1203C000), // CGameScoreTask_GetSeasonListFromUser : CWebServicesTaskSequence
    (0x03125000, 0x0306B000), // CGameControlCameraEditorOrbital : -
    (0x03126000, 0x03077000), // CGameCtnMediaBlockDOF : CGameCtnMediaBlock
    (0x03127000, 0x03077000), // CGameCtnMediaBlockToneMapping : CGameCtnMediaBlock
    (0x03128000, 0x03077000), // CGameCtnMediaBlockBloomHdr : CGameCtnMediaBlock
    (0x03129000, 0x03077000), // CGameCtnMediaBlockTimeSpeed : CGameCtnMediaBlock
    (0x0312A000, 0x03077000), // CGameCtnMediaBlockManialink : CGameCtnMediaBlock
    (0x0312B000, 0x01001000), // CGamePlayerProfileChunk : CMwNod
    (0x0312C000, 0x0312B000), // CGamePlayerProfileChunk_AccountSettings : CGamePlayerProfileChunk
    (0x0312D000, 0x0312B000), // CGamePlayerProfileChunk_GameSettings : CGamePlayerProfileChunk
    (0x0312E000, 0x0312B000), // CGamePlayerProfileChunk_InterfaceSettings : CGamePlayerProfileChunk
    (0x0312F000, 0x0312B000), // CGamePlayerProfileChunk_InputBindingsConfig : CGamePlayerProfileChunk
    (0x03130000, 0x0312B000), // CGamePlayerProfileChunk_VehiclesSettings : CGamePlayerProfileChunk
    (0x03131000, 0x0312B000), // CGamePlayerProfileChunk_OldChallenge : CGamePlayerProfileChunk
    (0x03132000, 0x0312B000), // CGamePlayerProfileChunk_OldCampaign : CGamePlayerProfileChunk
    (0x03133000, 0x03077000), // CGameCtnMediaBlockVehicleLight : CGameCtnMediaBlock
    (0x03134000, 0x1203C000), // CGameScoreTask_GetSeasonList : CWebServicesTaskSequence
    (0x03135000, 0x01001000), // CGamePlaygroundUIConfigMgrScript : CMwNod
    (0x03136000, 0x07016000), // CGamePlaygroundControlSmPlayers : CControlFrame
    (0x03137000, 0x07016000), // CGamePlaygroundControlMessages : CControlFrame
    (0x03138000, 0x01001000), // CGamePlaygroundScript : CMwNod
    (0x03139000, 0x03077000), // CGameCtnMediaBlockFxCameraMap : CGameCtnMediaBlock
    (0x0313A000, 0x1203C000), // CGameScoreTask_GetAccountTrophyGainHistory : CWebServicesTaskSequence
    (0x0313D000, 0x0306B000), // CGameControlCameraTrackManiaRace : -
    (0x0313E000, 0x0300D000), // CGamePlaygroundBasic : CGamePlayground
    (0x0313F000, 0x030F0000), // CGameScriptHandlerManiaPlanetPlugin : CGameManialinkScriptHandler
    (0x03140000, 0x0312B000), // CGamePlayerProfileChunk_PackagesInfos : CGamePlayerProfileChunk
    (0x03145000, 0x03077000), // CGameCtnMediaBlockShoot : CGameCtnMediaBlock
    (0x03147000, 0x0312B000), // CGamePlayerProfileChunk_GameStats : CGamePlayerProfileChunk
    (0x03148000, 0x0312B000), // CGamePlayerProfileChunk_ChallengesStats : CGamePlayerProfileChunk
    (0x0314A000, 0x03077000), // CGameCtnMediaBlockSkel : CGameCtnMediaBlock
    (0x0314B000, 0x01001000), // CGameEditorAnimClip : CMwNod
    (0x0314C000, 0x0304E000), // CGameCtnBlockInfoTransition : CGameCtnBlockInfo
    (0x0314D000, 0x0305C000), // CGameCtnZoneTransition : CGameCtnZone
    (0x03150000, 0x01001000), // CGameEditorTrigger : CMwNod
    (0x03151000, 0x01001000), // CGameCtnZoneFusionInfo : CMwNod
    (0x03152000, 0x0309A000), // CGameControlCardBuddy : CGameControlCard
    (0x03153000, 0x01001000), // CGameBuddy : CMwNod
    (0x03154000, 0x03160000), // CGameEditorPluginMapMapType : CGameEditorPluginMap
    (0x03155000, 0x01001000), // CGameCursorItem : CMwNod
    (0x03156000, 0x01001000), // CGameCtnMacroBlockJunction : CMwNod
    (0x03157000, 0x0318F000), // CGameActionMaker : CGameEditorAsset
    (0x03159000, 0x01001000), // CGameCtnEditorScriptAnchoredObject : CMwNod
    (0x0315A000, 0x01001000), // CGameCtnEditorScriptSpecialProperty : CMwNod
    (0x0315B000, 0x01001000), // CGameCtnBlockInfoVariant : CMwNod
    (0x0315C000, 0x0315B000), // CGameCtnBlockInfoVariantGround : CGameCtnBlockInfoVariant
    (0x0315D000, 0x0315B000), // CGameCtnBlockInfoVariantAir : CGameCtnBlockInfoVariant
    (0x0315E000, 0x01001000), // CGameTeamProfile : CMwNod
    (0x0315F000, 0x11005000), // CGameManialinkScriptEvent : CScriptBaseConstEvent
    (0x03160000, 0x031A4000), // CGameEditorPluginMap : CGameManiaApp
    (0x03161000, 0x030A3000), // CGameCtnMediaBlockCameraEffectScript : CGameCtnMediaBlockCameraEffect
    (0x03162000, 0x01001000), // CGameScriptServerAdmin : CMwNod
    (0x03163000, 0x0312B000), // CGamePlayerProfileChunk_EditorSettings : CGamePlayerProfileChunk
    (0x03164000, 0x031A5000), // CGameEditorPluginMapScriptEvent : CGameManiaAppScriptEvent
    (0x03165000, 0x03077000), // CGameCtnMediaBlockDirtyLens : CGameCtnMediaBlock
    (0x03166000, 0x030A3000), // CGameCtnMediaBlockCameraEffectInertialTracking : CGameCtnMediaBlockCameraEffect
    (0x03167000, 0x01001000), // CGameEditorPacks : CMwNod
    (0x03169000, 0x03077000), // CGameCtnMediaBlockBulletFx_Deprecated : CGameCtnMediaBlock
    (0x0316A000, 0x03077000), // CGameCtnMediaBlockCharVis_Deprecated : CGameCtnMediaBlock
    (0x0316B000, 0x01001000), // CGameServerScriptXmlRpc : CMwNod
    (0x0316C000, 0x03077000), // CGameCtnMediaBlockColoringCapturable : CGameCtnMediaBlock
    (0x0316D000, 0x03077000), // CGameCtnMediaBlockFxCameraBlend : CGameCtnMediaBlock
    (0x0316F000, 0x01001000), // CGameCoverFlowDesc : CMwNod
    (0x03170000, 0x0312B000), // CGamePlayerProfileChunk_ScriptPersistentTraits : CGamePlayerProfileChunk
    (0x03171000, 0x01001000), // CGamePlayerProfileCompatibilityChunk : CMwNod
    (0x03172000, 0x03077000), // CGameCtnMediaBlockColoringBase : CGameCtnMediaBlock
    (0x03173000, 0x01001000), // CGameCtnMediaClipConfigScriptContext : CMwNod
    (0x03174000, 0x11005000), // CGameServerScriptXmlRpcEvent : CScriptBaseConstEvent
    (0x03176000, 0x030F0000), // CGameScriptHandlerBrowser : CGameManialinkScriptHandler
    (0x03177000, 0x01001000), // CGameCtnEditorBody : CMwNod
    (0x03178000, 0x03106000), // CGameManialinkPlayerList : CGameManialinkControl
    (0x03179000, 0x0312B000), // CGamePlayerProfileChunk_GlobalInterfaceSettings : CGamePlayerProfileChunk
    (0x0317A000, 0x01001000), // CGameAnimSet : CMwNod
    (0x0317B000, 0x01001000), // CGameScriptChatManager : CMwNod
    (0x0317C000, 0x01001000), // CGameScriptChatContact : CMwNod
    (0x0317D000, 0x11005000), // CGameScriptChatEvent : CScriptBaseConstEvent
    (0x0317E000, 0x01001000), // CGameManialink3dMood : CMwNod
    (0x0317F000, 0x01001000), // CGameManialink3dWorld : CMwNod
    (0x03180000, 0x0312B000), // CGamePlayerProfileChunk_ManiaPlanetStations : CGamePlayerProfileChunk
    (0x03181000, 0x01001000), // CGameManialink3dStyle : CMwNod
    (0x03182000, 0x031A4000), // CGameManiaplanetPlugin : CGameManiaApp
    (0x03184000, 0x01001000), // CGameCtnInterfaceViewer : CMwNod
    (0x03185000, 0x03106000), // CGameManialinkGauge : CGameManialinkControl
    (0x03186000, 0x03077000), // CGameCtnMediaBlockColorGrading : CGameCtnMediaBlock
    (0x03187000, 0x01001000), // CGamePlaygroundClientScriptAPI : CMwNod
    (0x03188000, 0x03077000), // CGameCtnMediaBlockScenery : CGameCtnMediaBlock
    (0x03189000, 0x0306B000), // CGameControlCameraVehicleInternal : -
    (0x0318A000, 0x1203C000), // CWebServicesTask_SetServerInfo : CWebServicesTaskSequence
    (0x0318B000, 0x01001000), // CGameHud3dMarkerConfig : CMwNod
    (0x0318C000, 0x01001000), // CGamePlaygroundResources : CMwNod
    (0x0318D000, 0x1203C000), // CWebServicesTask_GetServerInfo : CWebServicesTaskSequence
    (0x0318E000, 0x1203D000), // CWebServicesTaskResult_ServerInfo : CWebServicesTaskResult
    (0x0318F000, 0x030AF000), // CGameEditorAsset : CGameCtnEditor
    (0x03190000, 0x03106000), // CGameManialinkGraph : CGameManialinkControl
    (0x03191000, 0x01001000), // CGameManialinkGraphCurve : CMwNod
    (0x03192000, 0x01001000), // CGameCtnBlockInfoMobilLink : CMwNod
    (0x03193000, 0x01001000), // CGameManiaplanetPluginInterface : CMwNod
    (0x03194000, 0x11005000), // CGameManiaplanetPluginInterfaceEvent : CScriptBaseConstEvent
    (0x03195000, 0x03077000), // CGameCtnMediaBlockInterface : CGameCtnMediaBlock
    (0x03196000, 0x03077000), // CGameCtnMediaBlockObject : CGameCtnMediaBlock
    (0x03197000, 0x01001000), // CGameScriptCloudManager : CMwNod
    (0x03198000, 0x01001000), // CGameEditorBullet : CMwNod
    (0x03199000, 0x03077000), // CGameCtnMediaBlockFog : CGameCtnMediaBlock
    (0x0319A000, 0x01001000), // CGameEditorActionScript : CMwNod
    (0x0319B000, 0x0318F000), // CGameEditorItem : CGameEditorAsset
    (0x0319C000, 0x03106000), // CGameManialinkMiniMap : CGameManialinkControl
    (0x0319D000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesSkin : CWebServicesTaskResult
    (0x0319E000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesSkinList : CWebServicesTaskResult
    (0x0319F000, 0x031A1000), // CGameEditorManialink : CGameEditorBase
    (0x031A0000, 0x01001000), // CGameManiaTitleEditionScriptAPI : CMwNod
    (0x031A1000, 0x01001000), // CGameEditorBase : CMwNod
    (0x031A2000, 0x01001000), // CGameEditorPropertyList : CMwNod
    (0x031A3000, 0x01001000), // CGameEditorFileToolBar : CMwNod
    (0x031A4000, 0x01001000), // CGameManiaApp : CMwNod
    (0x031A5000, 0x11005000), // CGameManiaAppScriptEvent : CScriptBaseConstEvent
    (0x031A6000, 0x031A4000), // CGameManiaAppPlaygroundCommon : CGameManiaApp
    (0x031A7000, 0x03106000), // CGameManialinkCamera : CGameManialinkControl
    (0x031A8000, 0x01001000), // CGameReplayObjectVisData : CMwNod
    (0x031A9000, 0x031A4000), // CGameManiaAppTitle : CGameManiaApp
    (0x031AA000, 0x03077000), // CGameCtnMediaBlockDecal2d : CGameCtnMediaBlock
    (0x031AB000, 0x03034000), // CGameCtnMediaBlockEditorDecal2d : CGameCtnMediaBlockEditor
    (0x031AC000, 0x031A4000), // CGameManiaAppBrowser : CGameManiaApp
    (0x031AD000, 0x01001000), // CGameManiaTitleControlScriptAPI : CMwNod
    (0x031AE000, 0x030F0000), // CGameManiaAppTitleLayerScriptHandler : CGameManialinkScriptHandler
    (0x031AF000, 0x030F0000), // CGameEditorPluginMapLayerScriptHandler : CGameManialinkScriptHandler
    (0x031B0000, 0x01001000), // CGameScriptNotificationsConsumerNotification : CMwNod
    (0x031B1000, 0x01001000), // CGameScriptNotificationsProducer : CMwNod
    (0x031B2000, 0x01001000), // CGameScriptNotificationsConsumer : CMwNod
    (0x031B3000, 0x01001000), // CGameScriptNotificationsProducerEvent : CMwNod
    (0x031B4000, 0x01001000), // CGameScriptNotificationsConsumerEvent : CMwNod
    (0x031B6000, 0x03106000), // CGameManialinkTextEdit : CGameManialinkControl
    (0x031B7000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_GetListForUser : CWebServicesTaskSequence
    (0x031B8000, 0x01001000), // CGameManialinkStylesheet : CMwNod
    (0x031B9000, 0x1203C000), // CGameWebServicesNotificationTask_BuildVisualNotification : CWebServicesTaskSequence
    (0x031BC000, 0x031A4000), // CGameManiaAppMinimal : CGameManiaApp
    (0x031BD000, 0x01001000), // CGameVideoScriptManager : CMwNod
    (0x031BE000, 0x01001000), // CGameGhostScript : CMwNod
    (0x031BF000, 0x01001000), // CTmRaceResultNod : CMwNod
    (0x031C0000, 0x030AF000), // CGameEditorModule : CGameCtnEditor
    (0x031C1000, 0x01001000), // CGameMenuSceneScriptManager : CMwNod
    (0x031C7000, 0x031D7000), // CGamePlaygroundModuleClientInventory : CGamePlaygroundModuleClient
    (0x031C8000, 0x031D7000), // CGamePlaygroundModuleClientScoresTable : CGamePlaygroundModuleClient
    (0x031C9000, 0x01001000), // CGamePlaygroundModuleManagerClient : CMwNod
    (0x031CA000, 0x031A5000), // CGameManiaAppPlaygroundScriptEvent : CGameManiaAppScriptEvent
    (0x031CC000, 0x01001000), // CGameUserProfile : CMwNod
    (0x031CD000, 0x01001000), // CGameUIAnimManager : CMwNod
    (0x031CE000, 0x03072000), // CGameControlCameraTrackManiaRace3 : -
    (0x031CF000, 0x03072000), // CGameControlCameraTrackManiaRace2 : -
    (0x031D3000, 0x031D7000), // CGamePlaygroundModuleClientStore : CGamePlaygroundModuleClient
    (0x031D4000, 0x01001000), // CGamePlaygroundModuleConfig : CMwNod
    (0x031D5000, 0x12039000), // CGameMasterServerUserInfo : CNetMasterServerUserInfo
    (0x031D6000, 0x01001000), // CGameBadgeStickerSlots : CMwNod
    (0x031D7000, 0x031A6000), // CGamePlaygroundModuleClient : CGameManiaAppPlaygroundCommon
    (0x031D8000, 0x031A6000), // CGameManiaAppPlayground : CGameManiaAppPlaygroundCommon
    (0x031D9000, 0x01001000), // CGameVideoScriptVideo : CMwNod
    (0x031DA000, 0x01001000), // CGameUserScript : CMwNod
    (0x031DB000, 0x01001000), // CGameUserManagerScript : CMwNod
    (0x031DC000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_AddFavorite : CWebServicesTaskSequence
    (0x031DD000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_RemoveFavorite : CWebServicesTaskSequence
    (0x031DE000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_GetFavoriteList : CWebServicesTaskSequence
    (0x031DF000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_GetFavoriteListByUid : CWebServicesTaskSequence
    (0x031E2000, 0x031A9000), // CGameModuleMenuBase : CGameManiaAppTitle
    (0x031E3000, 0x031E7000), // CGameModuleMenuBrowser : CGameModuleMenuComponent
    (0x031E4000, 0x01001000), // CGameHapticDevice : CMwNod
    (0x031E5000, 0x01001000), // CGamePluginInterfacesScript : CMwNod
    (0x031E7000, 0x01001000), // CGameModuleMenuComponent : CMwNod
    (0x031ED000, 0x01001000), // CGameScoreComputer_MultiAsyncLevel : CMwNod
    (0x031F8000, 0x030E4000), // CWebServicesTaskResult_AccountTrophyGainListScript : CWebServicesTaskResult_AccountTrophyGainList
    (0x031FB000, 0x1203C000), // CGameScoreTask_LoadPlayerScore : CWebServicesTaskSequence
    (0x031FC000, 0x1203C000), // CGameScoreTask_SynchronizePlayerScore : CWebServicesTaskSequence
    (0x031FD000, 0x0319D000), // CWebServicesTaskResult_NadeoServicesSkinScript : CWebServicesTaskResult_NadeoServicesSkin
    (0x031FE000, 0x0319E000), // CWebServicesTaskResult_NadeoServicesSkinListScript : CWebServicesTaskResult_NadeoServicesSkinList
    (0x031FF000, 0x031A5000), // CGameEditorPluginModuleScriptEvent : CGameManiaAppScriptEvent
    (0x03203000, 0x1203D000), // CWebServicesTaskResult_MapRecordList : CWebServicesTaskResult
    (0x03204000, 0x1203C000), // CGameScoreTask_SetSeason : CWebServicesTaskSequence
    (0x03205000, 0x031E7000), // CGameModuleMenuLadderRankings : CGameModuleMenuComponent
    (0x03206000, 0x030F0000), // CGameScriptHandlerPlaygroundModuleStore : CGameManialinkScriptHandler
    (0x03207000, 0x031E7000), // CGameModuleMenuServerBrowser : CGameModuleMenuComponent
    (0x03208000, 0x031A6000), // CGamePlaygroundModuleClientHud : CGameManiaAppPlaygroundCommon
    (0x03209000, 0x01001000), // CGamePlaygroundModuleServer : CMwNod
    (0x0320D000, 0x031AE000), // CGameScriptHandlerTitleModuleMenu : CGameManiaAppTitleLayerScriptHandler
    (0x0320E000, 0x01001000), // CGameEditorAnimChar : CMwNod
    (0x0320F000, 0x030F0000), // CGameScriptHandlerPlaygroundModuleInventory : CGameManialinkScriptHandler
    (0x03210000, 0x01001000), // CGameModuleInventoryCategory : CMwNod
    (0x03211000, 0x01001000), // CGameModuleScriptItem : CMwNod
    (0x03212000, 0x1204F000), // CGameMasterServerTask_SetBuddies : CNetMasterServerRequestTask
    (0x03214000, 0x1203C000), // CGameDataFileTask_Skin_NadeoServices_Get : CWebServicesTaskSequence
    (0x03215000, 0x1203C000), // CGameScoreTask_SetNewMapRecord : CWebServicesTaskSequence
    (0x03217000, 0x03209000), // CGamePlaygroundModuleServerStore : CGamePlaygroundModuleServer
    (0x03218000, 0x01001000), // CGamePlaygroundModuleServerHud : CMwNod
    (0x03219000, 0x01001000), // CGamePlaygroundModuleManagerServer : CMwNod
    (0x0321A000, 0x03209000), // CGamePlaygroundModuleServerInventory : CGamePlaygroundModuleServer
    (0x0321B000, 0x03209000), // CGamePlaygroundModuleServerScoresTable : CGamePlaygroundModuleServer
    (0x0321F000, 0x1203D000), // CWebServicesTaskResult_Ghost : CWebServicesTaskResult
    (0x03220000, 0x1203C000), // CGameScoreTask_GetPlayerMapRecordGhost : CWebServicesTaskSequence
    (0x03221000, 0x01001000), // CGameScoreAndLeaderBoardManagerScript : CMwNod
    (0x03223000, 0x1203D000), // CWebServicesTaskResult_GetDisplayNameScriptResult : CWebServicesTaskResult
    (0x03224000, 0x01001000), // CGameEditorTimeLine : CMwNod
    (0x03225000, 0x01001000), // CGameEditorAnimChar_Interface : CMwNod
    (0x0322A000, 0x031A1000), // CGameEditorAnimSet : CGameEditorBase
    (0x0322B000, 0x01001000), // CGameAnimClipNod : CMwNod
    (0x0322F000, 0x03230000), // CGameEditorVehicle : CGameEditorParent
    (0x03230000, 0x03102000), // CGameEditorParent : CGameSwitcherModule
    (0x03233000, 0x1204F000), // CGameCtnMasterServerTask_GetLeagues : CNetMasterServerRequestTask
    (0x03235000, 0x1206B000), // CGameMasterServerTask_Connect : CNetMasterServerTask_Connect
    (0x03237000, 0x01001000), // CGameMasterServerPlayerOnlinePresence : CMwNod
    (0x03238000, 0x1203D000), // CWebServicesTaskResult_OnlinePresenceList : CWebServicesTaskResult
    (0x03239000, 0x1204F000), // CGameCtnMasterServerTask_GetOnlinePresenceForPlayers : CNetMasterServerRequestTask
    (0x0323A000, 0x03245000), // CGameMasterServerRichPresenceTaskResult_GetOnlinePresenceForPlayersScript : CGameMasterServerRichPresenceTaskResult_PlayerOnlinePresenceList
    (0x0323C000, 0x01001000), // CGameMasterServerRichPresenceManager : CMwNod
    (0x0323D000, 0x1203C000), // CGameMasterServerRichPresenceTask_UpdatePresence : CWebServicesTaskSequence
    (0x0323E000, 0x01001000), // CGameUserPrivilegesManagerScript : CMwNod
    (0x03242000, 0x121A7000), // CWebServicesTaskResult_CheckTargetedPrivilegeResultScript : CWebServicesTaskResult_CheckTargetedPrivilegeResult
    (0x03244000, 0x1203C000), // CGameMasterServerRichPresenceTask_GetOnlinePresence : CWebServicesTaskSequence
    (0x03245000, 0x1203D000), // CGameMasterServerRichPresenceTaskResult_PlayerOnlinePresenceList : CWebServicesTaskResult
    (0x03246000, 0x01001000), // CGameMasterServerRichPresenceManagerScript : CMwNod
    (0x0324A000, 0x1203C000), // CGameCtnMasterServerTask_BuyFullGame : CWebServicesTaskSequence
    (0x0324C000, 0x0302F000), // CGameNetFormVoiceChat : CGameNetForm
    (0x0324F000, 0x1203C000), // CGameScoreTask_SetTrophyLiveTimeAttackAchievementResults : CWebServicesTaskSequence
    (0x03251000, 0x1203C000), // CGameScoreTask_UploadNewMapRecord : CWebServicesTaskSequence
    (0x03255000, 0x01001000), // CGameZoneManagerScript : CMwNod
    (0x0325F000, 0x03211000), // CGameModuleScriptStoreItem : CGameModuleScriptItem
    (0x03260000, 0x01001000), // CGameModuleScriptStoreCategory : CMwNod
    (0x03262000, 0x01001000), // CGameSaveLaunchedCheckpoints : CMwNod
    (0x03267000, 0x03274000), // CGameEditorMainPlugin : CGameEditorPlugin
    (0x03268000, 0x1203C000), // CGameDataFileTask_Skin_NadeoServices_GetList : CWebServicesTaskSequence
    (0x0326A000, 0x11004000), // CGamePlaygroundUIConfigEvent : CScriptBaseEvent
    (0x0326B000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesMap : CWebServicesTaskResult
    (0x0326C000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesMapList : CWebServicesTaskResult
    (0x0326D000, 0x01001000), // CGamePackCreatorScript : CMwNod
    (0x0326E000, 0x01001000), // CGamePackCreator_PackScript : CMwNod
    (0x0326F000, 0x01001000), // CGamePackCreator_TitleInfoScript : CMwNod
    (0x03270000, 0x01001000), // CGamePackCreator_RecipientScript : CMwNod
    (0x03271000, 0x01001000), // CGameCtnDecorationMaterialModifiers : CMwNod
    (0x03272000, 0x01001000), // CGameModuleMenuPage : CMwNod
    (0x03274000, 0x031A4000), // CGameEditorPlugin : CGameManiaApp
    (0x03275000, 0x01001000), // CGameManialinkAnimManager : CMwNod
    (0x03276000, 0x030AF000), // CGameEditorEditor : CGameCtnEditor
    (0x03277000, 0x03072000), // CGameControlCameraHmdExternal : -
    (0x03278000, 0x0326B000), // CWebServicesTaskResult_NadeoServicesMapScript : CWebServicesTaskResult_NadeoServicesMap
    (0x03279000, 0x0326C000), // CWebServicesTaskResult_NadeoServicesMapListScript : CWebServicesTaskResult_NadeoServicesMapList
    (0x0327B000, 0x01001000), // CGameScriptChatRoom : CMwNod
    (0x03282000, 0x01001000), // CGameShield : CMwNod
    (0x03284000, 0x01001000), // CGameDialogsScript : CMwNod
    (0x03285000, 0x01001000), // CGameScriptChatHistory : CMwNod
    (0x03286000, 0x01001000), // CGameScriptChatHistoryEntry : CMwNod
    (0x03287000, 0x01001000), // CGameEditorCanvas : CMwNod
    (0x0328A000, 0x01001000), // CGameEditorPluginMapManager : CMwNod
    (0x0328B000, 0x0318F000), // CGameEditorMesh : CGameEditorAsset
    (0x0328C000, 0x031A5000), // CGameEditorEvent : CGameManiaAppScriptEvent
    (0x0328D000, 0x1203C000), // CGameDataFileTask_Skin_NadeoServices_GetAccountList : CWebServicesTaskSequence
    (0x0328E000, 0x01001000), // CGameTurretPhy : CMwNod
    (0x0328F000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_GetAccountList : CWebServicesTaskSequence
    (0x03290000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_GetList : CWebServicesTaskSequence
    (0x03293000, 0x01001000), // CGameEditorPluginHandle : CMwNod
    (0x03294000, 0x03077000), // CGameCtnMediaBlockTurret : CGameCtnMediaBlock
    (0x03295000, 0x030F0000), // CGameEditorPluginLayerScriptHandler : CGameManialinkScriptHandler
    (0x03297000, 0x01001000), // CGameDataFileManager : CMwNod
    (0x03299000, 0x1203C000), // CGameDataFileTask_GhostStoreUserRecord_Maniaplanet : CWebServicesTaskSequence
    (0x0329B000, 0x1203C000), // CGameDataFileTask_GhostLoadUserRecord_Maniaplanet : CWebServicesTaskSequence
    (0x0329C000, 0x01001000), // CGameDialogsScriptEvent : CMwNod
    (0x0329D000, 0x0321F000), // CWebServicesTaskResult_GhostScript : CWebServicesTaskResult_Ghost
    (0x0329E000, 0x1203C000), // CGameDataFileTask_GhostLoadMedal : CWebServicesTaskSequence
    (0x0329F000, 0x03077000), // CGameCtnMediaBlockEntity : CGameCtnMediaBlock
    (0x032A1000, 0x032A7000), // CWebServicesTaskResult_MapListScript : CWebServicesTaskResult_GameFidList
    (0x032A3000, 0x1203C000), // CGameDataFileTask_GameFidGetGameList : CWebServicesTaskSequence
    (0x032A5000, 0x01001000), // CGameDataFileManagerScript : CMwNod
    (0x032A7000, 0x1203D000), // CWebServicesTaskResult_GameFidList : CWebServicesTaskResult
    (0x032A8000, 0x032A7000), // CWebServicesTaskResult_ReplayListScript : CWebServicesTaskResult_GameFidList
    (0x032A9000, 0x1203D000), // CWebServicesTaskResult_FileList : CWebServicesTaskResult
    (0x032AA000, 0x032A9000), // CWebServicesTaskResult_FileListScript : CWebServicesTaskResult_FileList
    (0x032AB000, 0x1203C000), // CGameDataFileTask_FileGetGameList : CWebServicesTaskSequence
    (0x032AD000, 0x01001000), // CGameMapScoreManager : CMwNod
    (0x032AE000, 0x032AD000), // CGameMapScoreManager_MapRecord : CGameMapScoreManager
    (0x032AF000, 0x032AD000), // CGameMapScoreManager_MultiAsyncLevel : CGameMapScoreManager
    (0x032B1000, 0x01001000), // CGameScriptMgrTurret : CMwNod
    (0x032B2000, 0x032E5000), // CGameScriptTurret : CGameScriptEntity
    (0x032B3000, 0x1203D000), // CWebServicesTaskResult_GhostList : CWebServicesTaskResult
    (0x032B4000, 0x032B3000), // CWebServicesTaskResult_GhostListScript : CWebServicesTaskResult_GhostList
    (0x032B5000, 0x1204F000), // CGameMasterServerTask_SetTitlePaid : CNetMasterServerRequestTask
    (0x032BB000, 0x01001000), // CGameMatchSettingsManagerScript : CMwNod
    (0x032BC000, 0x01001000), // CGameMatchSettingsScript : CMwNod
    (0x032BD000, 0x1203C000), // CGameDataFileTask_PackDownloadOrUpdate : CWebServicesTaskSequence
    (0x032BE000, 0x1203D000), // CWebServicesTaskResult_Title : CWebServicesTaskResult
    (0x032BF000, 0x01001000), // CGameMatchSettingsPlaylistItemScript : CMwNod
    (0x032C0000, 0x1203C000), // CGameScoreTask_GetPlayerPersonalBestMapRecordList : CWebServicesTaskSequence
    (0x032C1000, 0x1222A000), // CWebServicesTaskResult_MapRecordListScript : CWebServicesTaskResult_WSMapRecordList
    (0x032C2000, 0x1203C000), // CGameScoreTask_SetTrophyCompetitionMatchAchievementResults : CWebServicesTaskSequence
    (0x032C3000, 0x1203C000), // CGameDataFileTask_GameModeGetGameList : CWebServicesTaskSequence
    (0x032C4000, 0x1203D000), // CWebServicesTaskResult_GameModeListScript : CWebServicesTaskResult
    (0x032C5000, 0x01001000), // CGameGameModeInfoScript : CMwNod
    (0x032C6000, 0x01001000), // CGameUserProfileWrapper : CMwNod
    (0x032C7000, 0x01001000), // CGameEditorPluginAPI : CMwNod
    (0x032CA000, 0x01001000), // CGameBlockItemVariantChooser : CMwNod
    (0x032CC000, 0x03230000), // CGameEditorMaterial : CGameEditorParent
    (0x032CD000, 0x121A8000), // CWebServicesTaskResult_UserNewsListScript : CWebServicesTaskResult_WSNewsList
    (0x032CE000, 0x01001000), // CGameMgrAction : CMwNod
    (0x032D0000, 0x032E5000), // CGameScriptVehicle : CGameScriptEntity
    (0x032D1000, 0x01001000), // CGameScriptMgrVehicle : CMwNod
    (0x032D2000, 0x01001000), // CGameAction : CMwNod
    (0x032D3000, 0x01001000), // CGameScriptAction : CMwNod
    (0x032D4000, 0x1203C000), // CGameDataFileTask_Skin_NadeoServices_Set : CWebServicesTaskSequence
    (0x032D5000, 0x01001000), // CGameScriptMapLandmark : CMwNod
    (0x032D6000, 0x01001000), // CGameScriptMapSector : CMwNod
    (0x032D7000, 0x01001000), // CGameScriptMapSpawn : CMwNod
    (0x032D8000, 0x01001000), // CGameScriptMapBotPath : CMwNod
    (0x032D9000, 0x01001000), // CGameScriptMapObjectAnchor : CMwNod
    (0x032DA000, 0x01001000), // CGameScriptMapBotSpawn : CMwNod
    (0x032DE000, 0x1203C000), // CWebServicesTask_Title_GetConfig : CWebServicesTaskSequence
    (0x032DF000, 0x1203C000), // CWebServicesTask_Title_GetPolicyRuleValues : CWebServicesTaskSequence
    (0x032E0000, 0x1203C000), // CWebServicesTask_Title_GetLadderInfo : CWebServicesTaskSequence
    (0x032E2000, 0x0A020000), // CGameVehiclePhy : -
    (0x032E5000, 0x01001000), // CGameScriptEntity : CMwNod
    (0x032E6000, 0x032E5000), // CGameScriptPlayer : CGameScriptEntity
    (0x032E7000, 0x01001000), // CGameScriptChatSquadInvitation : CMwNod
    (0x032E8000, 0x01001000), // CGameScriptChatHistoryEntryMessage : CMwNod
    (0x032E9000, 0x1203C000), // CGameZoneTask_UpdateZoneList : CWebServicesTaskSequence
    (0x032EA000, 0x1204F000), // CGameMasterServerTask_GetOnlineProfile : CNetMasterServerRequestTask
    (0x032EB000, 0x1203D000), // CGameMasterServerRichPresenceTaskResult_NextPresence : CWebServicesTaskResult
    (0x032EC000, 0x1203D000), // CWebServicesTaskResult_PlanetsTransaction_Bill : CWebServicesTaskResult
    (0x032ED000, 0x01001000), // CGameScriptMapVehicleAnchor : CMwNod
    (0x032EE000, 0x01001000), // CGameShootIconSetting : CMwNod
    (0x032EF000, 0x01001000), // CGameScriptMapWaypoint : CMwNod
    (0x032F0000, 0x0306B000), // CGameControlCameraHelico : -
    (0x032F1000, 0x1203C000), // CWebServicesTask_PostConnect : CWebServicesTaskSequence
    (0x032F2000, 0x01001000), // CGameShootIconConfig : CMwNod
    (0x032F3000, 0x1203C000), // CWebServicesTask_PostConnect_BannedCryptedChecksumsList : CWebServicesTaskSequence
    (0x032F4000, 0x1203C000), // CWebServicesTask_PostConnect_UrlConfig : CWebServicesTaskSequence
    (0x032F5000, 0x01001000), // CGameEditorMapScriptClipList : CMwNod
    (0x032F7000, 0x01001000), // CGameEditorMapScriptClip : CMwNod
    (0x032F8000, 0x1203C000), // CWebServicesTask_SynchronizeProfileChunks : CWebServicesTaskSequence
    (0x032F9000, 0x031D7000), // CGamePlaygroundModuleClientChrono : CGamePlaygroundModuleClient
    (0x032FA000, 0x03209000), // CGamePlaygroundModuleServerChrono : CGamePlaygroundModuleServer
    (0x032FB000, 0x031D7000), // CGamePlaygroundModuleClientSpeedMeter : CGamePlaygroundModuleClient
    (0x032FC000, 0x03209000), // CGamePlaygroundModuleServerSpeedMeter : CGamePlaygroundModuleServer
    (0x032FD000, 0x031D7000), // CGamePlaygroundModuleClientPlayerState : CGamePlaygroundModuleClient
    (0x032FE000, 0x03209000), // CGamePlaygroundModuleServerPlayerState : CGamePlaygroundModuleServer
    (0x032FF000, 0x031D7000), // CGamePlaygroundModuleClientTeamState : CGamePlaygroundModuleClient
    (0x03300000, 0x03209000), // CGamePlaygroundModuleServerTeamState : CGamePlaygroundModuleServer
    (0x03301000, 0x031D7000), // CGamePlaygroundModuleClientAltimeter : CGamePlaygroundModuleClient
    (0x03302000, 0x03209000), // CGamePlaygroundModuleServerAltimeter : CGamePlaygroundModuleServer
    (0x03303000, 0x031D7000), // CGamePlaygroundModuleClientThrottle : CGamePlaygroundModuleClient
    (0x03304000, 0x03209000), // CGamePlaygroundModuleServerThrottle : CGamePlaygroundModuleServer
    (0x03305000, 0x01001000), // CGameEditorUndoSystem_State : CMwNod
    (0x03306000, 0x1203C000), // CWebServicesTask_UploadProfileChunks : CWebServicesTaskSequence
    (0x03309000, 0x1204F000), // CGameMasterServerTask_GetTitlePackagesInfos : CNetMasterServerRequestTask
    (0x0330A000, 0x1203C000), // CWebServicesTask_GetTitlePackagesInfos : CWebServicesTaskSequence
    (0x0330B000, 0x1204F000), // CGameMasterServerTask_UpdateManiaPlanetStationInfos : CNetMasterServerRequestTask
    (0x0330C000, 0x1203C000), // CWebServicesTask_LoadStation : CWebServicesTaskSequence
    (0x0330D000, 0x1203C000), // CWebServicesTask_Title_GetPlayerInfos : CWebServicesTaskSequence
    (0x0330E000, 0x1204F000), // CGameMasterServerTask_GetPackageUpdateUrl : CNetMasterServerRequestTask
    (0x0330F000, 0x1203C000), // CWebServicesTask_GetPackageUpdateUrl : CWebServicesTaskSequence
    (0x03310000, 0x1203D000), // CWebServicesTaskResult_CreditedPackageUpdateUrlList : CWebServicesTaskResult
    (0x03313000, 0x1204F000), // CGameMasterServerTask_GetAuthenticationToken : CNetMasterServerRequestTask
    (0x03314000, 0x1204F000), // CGameMasterServerTask_GetAccountFromUplayUser : CNetMasterServerRequestTask
    (0x03315000, 0x1204F000), // CGameMasterServerTask_GetSubscribedGroups : CNetMasterServerRequestTask
    (0x03317000, 0x1204F000), // CGameMasterServerTask_UpdateOnlineProfile : CNetMasterServerRequestTask
    (0x03318000, 0x1203C000), // CWebServicesTask_UnloadStation : CWebServicesTaskSequence
    (0x03319000, 0x01001000), // CGameEditorMapMacroBlockInstance : CMwNod
    (0x0331A000, 0x01001000), // CGameScoreAndLeaderBoardManager : CMwNod
    (0x0331B000, 0x1203C000), // CWebServicesTask_PostConnect_Zone : CWebServicesTaskSequence
    (0x0331C000, 0x1203D000), // CWebServicesTaskResult_UserZoneListScript : CWebServicesTaskResult
    (0x03321000, 0x1203D000), // CWebServicesTaskResult_AccountTrophyGainHistory : CWebServicesTaskResult
    (0x03322000, 0x1204F000), // CGameMasterServerTask_GetPlayerCreditedPackagesGroups : CNetMasterServerRequestTask
    (0x03323000, 0x1203C000), // CWebServicesTask_GetPlayerCreditedPackagesGroups : CWebServicesTaskSequence
    (0x03324000, 0x01001000), // CGameEditorPluginCameraManager : CMwNod
    (0x03325000, 0x01001000), // CGameEditorPluginCursorManager : CMwNod
    (0x03326000, 0x01001000), // CGameEditorPluginCameraAPI : CMwNod
    (0x03327000, 0x01001000), // CGameEditorPluginCursorAPI : CMwNod
    (0x03328000, 0x01001000), // CGameEditorPluginMapConnectResults : CMwNod
    (0x03329000, 0x01001000), // CGameEditorGenericInventory : CMwNod
    (0x0332A000, 0x03230000), // CGameEditorMediaTracker : CGameEditorParent
    (0x0332B000, 0x03230000), // CGameEditorSkin : CGameEditorParent
    (0x0332C000, 0x032C7000), // CGameEditorSkinPluginAPI : -
    (0x0332D000, 0x032C7000), // CGameEditorMediaTrackerPluginAPI : -
    (0x0332E000, 0x03106000), // CGameManialinkColorChooser : CGameManialinkControl
    (0x0332F000, 0x03106000), // CGameManialinkSlider : CGameManialinkControl
    (0x03330000, 0x03106000), // CGameManialinkTimeLine : CGameManialinkControl
    (0x03331000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_Get : CWebServicesTaskSequence
    (0x03332000, 0x1203C000), // CGameDataFileTask_Map_NadeoServices_Register : CWebServicesTaskSequence
    (0x03333000, 0x03230000), // CGameEditorAction : CGameEditorParent
    (0x03334000, 0x1203C000), // CGameScoreTask_GetSeason : CWebServicesTaskSequence
    (0x03335000, 0x03230000), // CGameEditorCustomBullet : CGameEditorParent
    (0x03336000, 0x1203D000), // CWebServicesTaskResult_Season : CWebServicesTaskResult
    (0x03337000, 0x03230000), // CGameEditorScript : CGameEditorParent
    (0x03339000, 0x01001000), // CGameSeasonScoreManager : CMwNod
    (0x0333A000, 0x1203C000), // CWebServicesTask_PostConnect_AdditionalFileList : CWebServicesTaskSequence
    (0x0333B000, 0x03339000), // CGameSeasonScoreManager_MapRecord : CGameSeasonScoreManager
    (0x0333C000, 0x03339000), // CGameSeasonScoreManager_MultiAsyncLevel : CGameSeasonScoreManager
    (0x0333D000, 0x1203C000), // CGameScoreTask_AddMapListToSeason : CWebServicesTaskSequence
    (0x0333E000, 0x1203C000), // CGameScoreTask_RemoveMapListToSeason : CWebServicesTaskSequence
    (0x0333F000, 0x1203C000), // CGameScoreTask_GetPlayerSeasonMapRecordList : CWebServicesTaskSequence
    (0x03340000, 0x03053000), // CGameCtnBlockInfoClipVertical : CGameCtnBlockInfoClip
    (0x03341000, 0x01001000), // CGameGhostMgrScript : CMwNod
    (0x03346000, 0x01001000), // CGameBlockInfoGroups : CMwNod
    (0x03347000, 0x1203C000), // CWebServicesTask_GetAccountXp : CWebServicesTaskSequence
    (0x03348000, 0x01001000), // CGameBlockInfoTreeRoot : CMwNod
    (0x03349000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_Unset : CWebServicesTaskSequence
    (0x0334A000, 0x1203D000), // CWebServicesTaskResult_AccountTrophyLastYearSummary : CWebServicesTaskResult
    (0x0334B000, 0x1203C000), // CGameScoreTask_GetAccountTrophyLastYearSummary : CWebServicesTaskSequence
    (0x0334C000, 0x1203D000), // CWebServicesTaskResult_TrophySoloMedalAchievementSettings : CWebServicesTaskResult
    (0x0334D000, 0x1203C000), // CGameScoreTask_GetTrophySoloMedalAchievementSettings : CWebServicesTaskSequence
    (0x0334E000, 0x0334A000), // CWebServicesTaskResult_AccountTrophyLastYearSummaryScript : CWebServicesTaskResult_AccountTrophyLastYearSummary
    (0x0334F000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_AddFavorite : CWebServicesTaskSequence
    (0x03351000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_GetFavoriteList : CWebServicesTaskSequence
    (0x03352000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_GetList : CWebServicesTaskSequence
    (0x03353000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_RemoveFavorite : CWebServicesTaskSequence
    (0x03354000, 0x1203C000), // CGameDataFileTask_AccountSkin_NadeoServices_Set : CWebServicesTaskSequence
    (0x03355000, 0x01001000), // CGameAudioSettingsWrapper : CMwNod
    (0x03356000, 0x01001000), // CGameItemModelTreeRoot : CMwNod
    (0x03358000, 0x0334C000), // CWebServicesTaskResult_TrophySoloMedalAchievementSettingsScript : CWebServicesTaskResult_TrophySoloMedalAchievementSettings
    (0x0335B000, 0x03053000), // CGameCtnBlockInfoClipHorizontal : CGameCtnBlockInfoClip
    (0x0335C000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesItemCollection : CWebServicesTaskResult
    (0x0335D000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesItemCollectionList : CWebServicesTaskResult
    (0x0335E000, 0x0335C000), // CWebServicesTaskResult_NadeoServicesItemCollectionScript : CWebServicesTaskResult_NadeoServicesItemCollection
    (0x0335F000, 0x0335D000), // CWebServicesTaskResult_NadeoServicesItemCollectionListScript : CWebServicesTaskResult_NadeoServicesItemCollectionList
    (0x03360000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_AddFavorite : CWebServicesTaskSequence
    (0x03361000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_Create : CWebServicesTaskSequence
    (0x03362000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_Get : CWebServicesTaskSequence
    (0x03363000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_GetAccountList : CWebServicesTaskSequence
    (0x03364000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_GetFavoriteList : CWebServicesTaskSequence
    (0x03365000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_GetList : CWebServicesTaskSequence
    (0x03366000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_RemoveFavorite : CWebServicesTaskSequence
    (0x03367000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_Update : CWebServicesTaskSequence
    (0x03368000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_CreateVersion : CWebServicesTaskSequence
    (0x03369000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_SetActivityId : CWebServicesTaskSequence
    (0x0336A000, 0x1203C000), // CGameScoreTask_LoadAndSynchronizeSeasonScoreList : CWebServicesTaskSequence
    (0x0336B000, 0x1203C000), // CGameScoreTask_LoadAndSynchronizePersonalBestScoreList : CWebServicesTaskSequence
    (0x0336C000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_RemoveFavoriteFromName : CWebServicesTaskSequence
    (0x0336D000, 0x1203C000), // CGameDataFileTask_ItemCollection_NadeoServices_GetListFromIdentifierList : CWebServicesTaskSequence
    (0x0336E000, 0x1203D000), // CWebServicesTaskResult_NadeoServicesItemCollectionWithClubInfoList : CWebServicesTaskResult
    (0x0336F000, 0x01001000), // CGameSessionArchive : CMwNod
    (0x03370000, 0x121B8000), // CWebServicesTaskResult_PrestigeListScript : CWebServicesTaskResult_WSPrestigeList
    (0x03371000, 0x12063000), // CWebServicesTaskResult_FriendListScript : CWebServicesTaskResult_WSFriendList
    (0x03372000, 0x01001000), // CGameUserService : CMwNod
    (0x03373000, 0x121B9000), // CWebServicesTaskResult_UserPrestigeListScript : CWebServicesTaskResult_WSUserPrestigeList
    (0x03374000, 0x1203D000), // CWebServicesTaskResult_Squad : CWebServicesTaskResult
    (0x03375000, 0x03374000), // CWebServicesTaskResult_SquadScript : CWebServicesTaskResult_Squad
    (0x03376000, 0x03385000), // CGameUserTask_Squad_AcceptInvitation : CGameUserTask_Squad_AbstractTask
    (0x03377000, 0x03385000), // CGameUserTask_Squad_CancelInvitation : CGameUserTask_Squad_AbstractTask
    (0x03378000, 0x03385000), // CGameUserTask_Squad_Create : CGameUserTask_Squad_AbstractTask
    (0x03379000, 0x03385000), // CGameUserTask_Squad_DeclineInvitation : CGameUserTask_Squad_AbstractTask
    (0x0337A000, 0x03385000), // CGameUserTask_Squad_Get : CGameUserTask_Squad_AbstractTask
    (0x0337B000, 0x03385000), // CGameUserTask_Squad_GetCurrent : CGameUserTask_Squad_AbstractTask
    (0x0337C000, 0x03385000), // CGameUserTask_Squad_InviteInto : CGameUserTask_Squad_AbstractTask
    (0x0337E000, 0x03385000), // CGameUserTask_Squad_RemoveMember : CGameUserTask_Squad_AbstractTask
    (0x0337F000, 0x03385000), // CGameUserTask_Squad_SetLeader : CGameUserTask_Squad_AbstractTask
    (0x03380000, 0x01001000), // CGameWebServicesNotificationService : CMwNod
    (0x03381000, 0x01001000), // CGameWebServicesNotificationManagerScript : CMwNod
    (0x03382000, 0x1203D000), // CWebServicesTaskResult_WSNotification : CWebServicesTaskResult
    (0x03383000, 0x03382000), // CWebServicesTaskResult_WSNotificationScript : CWebServicesTaskResult_WSNotification
    (0x03384000, 0x1203C000), // CGameWebServicesNotificationTask_PopNextNotification : CWebServicesTaskSequence
    (0x03385000, 0x1203C000), // CGameUserTask_Squad_AbstractTask : CWebServicesTaskSequence
    (0x03386000, 0x01001000), // CGameManialinkScriptHandler_ReadOnly : CMwNod
    (0x03387000, 0x03386000), // CGameScriptHandlerPlaygroundInterface_ReadOnly : -
    (0x03388000, 0x1203C000), // CWebServicesTask_PostConnect_Tag : CWebServicesTaskSequence
    (0x03389000, 0x121BA000), // CWebServicesTaskResult_UserPrestigeScript : CWebServicesTaskResult_WSUserPrestige
    (0x0338A000, 0x1203D000), // CWebServicesTaskResult_ClubTagListScript : CWebServicesTaskResult
    (0x0338B000, 0x03077000), // CGameCtnMediaBlockOpponentVisibility : CGameCtnMediaBlock
    (0x0338C000, 0x12199000), // CWebServicesTaskResult_ZoneListScript : CWebServicesTaskResult_WSZonePtrList
    (0x0338D000, 0x01001000), // CGameUserVoiceChat : CMwNod
    (0x0338E000, 0x030F0000), // CGameScriptHandlerMediaTrack : CGameManialinkScriptHandler
    (0x04001000, 0x01001000), // GxLight : CMwNod
    (0x04002000, 0x04003000), // CGxLightBall : GxLightPoint
    (0x04003000, 0x04006000), // GxLightPoint : GxLightNotAmbient
    (0x04004000, 0x01001000), // CGxFog : CMwNod
    (0x04005000, 0x04001000), // GxLightAmbient : GxLight
    (0x04006000, 0x04001000), // GxLightNotAmbient : GxLight
    (0x04007000, 0x04006000), // GxLightDirectional : GxLightNotAmbient
    (0x04008000, 0x01001000), // GxFogBlender : CMwNod
    (0x0400A000, 0x04002000), // CGxLightFrustum : CGxLightBall
    (0x0400B000, 0x04002000), // CGxLightSpot : CGxLightBall
    (0x05002000, 0x05010000), // CFuncKeys : CFunc
    (0x05003000, 0x05002000), // CFuncKeysTrans : CFuncKeys
    (0x0500B000, 0x05010000), // CFuncPlug : CFunc
    (0x0500C000, 0x05018000), // CFuncLightIntensity : CFuncLight
    (0x0500D000, 0x0501C000), // CFuncTreeTranslate : CFuncTree
    (0x0500E000, 0x05010000), // CFuncEnum : CFunc
    (0x05010000, 0x01001000), // CFunc : CMwNod
    (0x05011000, 0x0500B000), // CFuncShader : CFuncPlug
    (0x05014000, 0x05011000), // CFuncShaders : CFuncShader
    (0x05015000, 0x05011000), // CFuncShaderLayerUV : CFuncShader
    (0x05018000, 0x0500B000), // CFuncLight : CFuncPlug
    (0x05019000, 0x05018000), // CFuncLightColor : CFuncLight
    (0x0501C000, 0x0500B000), // CFuncTree : CFuncPlug
    (0x0501E000, 0x0501C000), // CFuncTreeRotate : CFuncTree
    (0x0501F000, 0x0501C000), // CFuncTreeBend : CFuncTree
    (0x05030000, 0x05002000), // CFuncKeysNatural : CFuncKeys
    (0x05031000, 0x0501C000), // CFuncTreeSubVisualSequence : CFuncTree
    (0x05037000, 0x05010000), // CFuncSegment : CFunc
    (0x05038000, 0x05010000), // CFuncColorGradient : CFunc
    (0x05039000, 0x05010000), // CFuncFullColorGradient : CFunc
    (0x06001000, 0x06007000), // CHmsCamera : CHmsPoc
    (0x06002000, 0x06008000), // CHmsCorpus : CHmsZoneElem
    (0x06003000, 0x01001000), // CHmsItem : CMwNod
    (0x06004000, 0x01001000), // CHmsZone : CMwNod
    (0x06006000, 0x01001000), // CHmsPortal : CMwNod
    (0x06007000, 0x06008000), // CHmsPoc : CHmsZoneElem
    (0x06008000, 0x01001000), // CHmsZoneElem : CMwNod
    (0x06009000, 0x06004000), // CHmsZoneOverlay : CHmsZone
    (0x0600C000, 0x06007000), // CHmsLight : CHmsPoc
    (0x0600E000, 0x01001000), // CHmsPortalProperty : CMwNod
    (0x06010000, 0x01001000), // CHmsViewport : CMwNod
    (0x06011000, 0x01001000), // CHmsPrecalcRender : CMwNod
    (0x06012000, 0x01001000), // CHmsShadowGroup : CMwNod
    (0x06014000, 0x01001000), // CHmsViewportPerfDbg : CMwNod
    (0x06016000, 0x01001000), // CHmsMgrVisDyna : CMwNod
    (0x06017000, 0x06008000), // CHmsFogPlane : CHmsZoneElem
    (0x06018000, 0x01001000), // CHmsPicker : CMwNod
    (0x0601D000, 0x01001000), // CHmsConfig : CMwNod
    (0x06020000, 0x01001000), // CHmsItemShadow : CMwNod
    (0x06021000, 0x01001000), // CHmsLightMap : CMwNod
    (0x06022000, 0x01001000), // CHmsLightMapCache : CMwNod
    (0x06023000, 0x01001000), // CHmsLightMapMood : CMwNod
    (0x06025000, 0x06002000), // CHmsCorpus2d : CHmsCorpus
    (0x06026000, 0x01001000), // CHmsAmbientOcc : CMwNod
    (0x06027000, 0x01001000), // CHmsLightProbeGrid : CMwNod
    (0x06028000, 0x01001000), // CHmsLightMapCacheSH : CMwNod
    (0x0602B000, 0x01001000), // CHmsLightMapParam : CMwNod
    (0x0602C000, 0x01001000), // CHmsLightArray : CMwNod
    (0x0602D000, 0x01001000), // CHmsDecalArray : CMwNod
    (0x0602E000, 0x01001000), // CHmsLightProbePartition : CMwNod
    (0x0602F000, 0x01001000), // CHmsMgrVisEnvMap : CMwNod
    (0x06032000, 0x01001000), // CHmsMgrVisDynaDecal2d : CMwNod
    (0x06035000, 0x01001000), // CHmsVisMiniMap : CMwNod
    (0x0603C000, 0x01001000), // CHmsVolumeShadow : CMwNod
    (0x06046000, 0x01001000), // CHmsMoodBlender : CMwNod
    (0x06047000, 0x01001000), // CHmsMgrVisVolume : CMwNod
    (0x07001000, 0x0A011000), // CControlBase : CSceneMobil
    (0x07002000, 0x07001000), // CControlContainer : CControlBase
    (0x07005000, 0x01001000), // CControlEffect : CMwNod
    (0x07006000, 0x07031000), // CControlLabel : CControlText
    (0x07007000, 0x07031000), // CControlButton : CControlText
    (0x07009000, 0x07031000), // CControlEntry : CControlText
    (0x0700A000, 0x07031000), // CControlEnum : CControlText
    (0x0700B000, 0x07001000), // CControlSlider : CControlBase
    (0x0700C000, 0x01001000), // CControlLayout : CMwNod
    (0x0700D000, 0x01001000), // CControlListItem : CMwNod
    (0x07010000, 0x07005000), // CControlEffectSimi : CControlEffect
    (0x07011000, 0x07005000), // CControlEffectMotion : CControlEffect
    (0x07014000, 0x07001000), // CControlUiRange : CControlBase
    (0x07015000, 0x07002000), // CControlGrid : CControlContainer
    (0x07016000, 0x07002000), // CControlFrame : CControlContainer
    (0x07017000, 0x0902B000), // CControlStyle : CPlug
    (0x07019000, 0x07001000), // CControlUrlLinks : CControlBase
    (0x0701B000, 0x07001000), // CControlQuad : CControlBase
    (0x0701C000, 0x01001000), // CControlEffectMaster : CMwNod
    (0x0701E000, 0x07016000), // CControlColorChooser : CControlFrame
    (0x0701F000, 0x07016000), // CControlColorChooser2 : CControlFrame
    (0x07021000, 0x01001000), // CControlSimi2 : CMwNod
    (0x07022000, 0x07001000), // CControlTimeLine2 : CControlBase
    (0x07023000, 0x07005000), // CControlEffectCombined : CControlEffect
    (0x07025000, 0x07005000), // CControlEffectMoveFrame : CControlEffect
    (0x07026000, 0x07016000), // CControlFrameStyled : CControlFrame
    (0x07027000, 0x0902B000), // CControlStyleSheet : CPlug
    (0x0702C000, 0x07016000), // CControlMediaPlayer : CControlFrame
    (0x0702F000, 0x07001000), // CControlGraph : CControlBase
    (0x07030000, 0x07016000), // CControlPager : CControlFrame
    (0x07031000, 0x07001000), // CControlText : CControlBase
    (0x07032000, 0x07016000), // CControlFrameAnimated : CControlFrame
    (0x07034000, 0x07001000), // CControlScriptEditor : CControlBase
    (0x07035000, 0x07001000), // CControlScriptConsole : CControlBase
    (0x07036000, 0x07016000), // CControlListCard : CControlFrame
    (0x07037000, 0x07001000), // CControlMiniMap : CControlBase
    (0x07038000, 0x07001000), // CControlCamera : CControlBase
    (0x09001000, 0x0902B000), // CPlugAudio : CPlug
    (0x09002000, 0x0902B000), // CPlugShader : CPlug
    (0x09003000, 0x0902B000), // CPlugCrystal : CPlug
    (0x09004000, 0x09002000), // CPlugShaderGeneric : CPlugShader
    (0x09005000, 0x0902B000), // CPlugSolid : CPlug
    (0x09006000, 0x0902B000), // CPlugVisual : CPlug
    (0x09007000, 0x01001000), // CPlugMapAINode : CMwNod
    (0x09008000, 0x0B017000), // CPlugBitmapHighLevel : CSystemNodWrapper
    (0x09009000, 0x0906A000), // CPlugVisualIndexedLines : CPlugVisualIndexed
    (0x0900A000, 0x01001000), // CPlugVisualOctree : CMwNod
    (0x0900B000, 0x09086000), // CPlugBitmapRenderShadow : CPlugBitmapRender
    (0x0900C000, 0x0902B000), // CPlugSurface : CPlug
    (0x0900E000, 0x01001000), // CPlugModelShading : CMwNod
    (0x0900F000, 0x0902B000), // CPlugSurfaceGeomDeprecated : CPlug
    (0x09010000, 0x0902C000), // CPlugVisualSprite : CPlugVisual3D
    (0x09011000, 0x0902B000), // CPlugBitmap : CPlug
    (0x09013000, 0x0902C000), // CPlugVisualLines : CPlugVisual3D
    (0x09014000, 0x0904A000), // CPlugVisualLines2D : CPlugVisual2D
    (0x09015000, 0x0904F000), // CPlugTreeVisualMip : CPlugTree
    (0x09016000, 0x0902C000), // CPlugVisualStrip : CPlugVisual3D
    (0x09017000, 0x0902C000), // CPlugVisualVertexs : CPlugVisual3D
    (0x09018000, 0x01001000), // CPlugVoxelResource : CMwNod
    (0x09019000, 0x09035000), // CPlugFilePack : CPlugFileFidContainer
    (0x0901A000, 0x09001000), // CPlugSound : CPlugAudio
    (0x0901B000, 0x0901A000), // CPlugSoundMood : CPlugSound
    (0x0901C000, 0x09037000), // CPlugMusic : CPlugMusicType
    (0x0901D000, 0x0902B000), // CPlugLight : CPlug
    (0x0901E000, 0x0906A000), // CPlugVisualIndexedTriangles : CPlugVisualIndexed
    (0x09020000, 0x0902B000), // CPlugFile : CPlug
    (0x09021000, 0x09086000), // CPlugBitmapRenderLightFromMap : CPlugBitmapRender
    (0x09022000, 0x09025000), // CPlugFileJpg : CPlugFileImg
    (0x09023000, 0x09025000), // CPlugFileTga : CPlugFileImg
    (0x09024000, 0x09025000), // CPlugFileDds : CPlugFileImg
    (0x09025000, 0x09020000), // CPlugFileImg : CPlugFile
    (0x09026000, 0x09004000), // CPlugShaderApply : CPlugShaderGeneric
    (0x09027000, 0x0902C000), // CPlugVisualQuads : CPlugVisual3D
    (0x09028000, 0x0902C000), // CPlugVisualTriangles : CPlugVisual3D
    (0x09029000, 0x0901A000), // CPlugSoundEngine2 : CPlugSound
    (0x0902A000, 0x0906A000), // CPlugVisualIndexedStrip : CPlugVisualIndexed
    (0x0902B000, 0x01001000), // CPlug : CMwNod
    (0x0902C000, 0x09006000), // CPlugVisual3D : CPlugVisual
    (0x0902D000, 0x09020000), // CPlugFileFont : CPlugFile
    (0x0902F000, 0x09025000), // CPlugFileGen : CPlugFileImg
    (0x09030000, 0x09020000), // CPlugFileSnd : CPlugFile
    (0x09031000, 0x09030000), // CPlugFileWav : CPlugFileSnd
    (0x09034000, 0x09001000), // CPlugAudioBalance : CPlugAudio
    (0x09035000, 0x09020000), // CPlugFileFidContainer : CPlugFile
    (0x09036000, 0x0902B000), // CPlugBitmapPacker : CPlug
    (0x09037000, 0x0901A000), // CPlugMusicType : CPlugSound
    (0x09039000, 0x09001000), // CPlugAudioEnvironment : CPlugAudio
    (0x0903A000, 0x0902B000), // CPlugMaterialCustom : CPlug
    (0x0903B000, 0x0902C000), // CPlugVisualGrid : CPlugVisual3D
    (0x0903D000, 0x09025000), // CPlugFilePng : CPlugFileImg
    (0x0903E000, 0x0902B000), // CPlugBlendShapes : CPlug
    (0x0903F000, 0x09051000), // CPlugTreeGenText : CPlugTreeGenerator
    (0x09040000, 0x09020000), // CPlugFileGPU : CPlugFile
    (0x09041000, 0x09020000), // CPlugFileText : CPlugFile
    (0x09044000, 0x0902B000), // CPlugBitmapPack : CPlug
    (0x09046000, 0x0902B000), // CPlugBitmapPackElem : CPlug
    (0x09047000, 0x0907E000), // CPlugBitmapAddress : CPlugBitmapSampler
    (0x09048000, 0x0902B000), // CPlugBitmapPackInput : CPlug
    (0x09049000, 0x09035000), // CPlugFileFidCache : CPlugFileFidContainer
    (0x0904A000, 0x09006000), // CPlugVisual2D : CPlugVisual
    (0x0904B000, 0x0904A000), // CPlugVisualQuads2D : CPlugVisual2D
    (0x0904D000, 0x0902B000), // CPlugFont : CPlug
    (0x0904E000, 0x0904D000), // CPlugFontBitmap : CPlugFont
    (0x0904F000, 0x0902B000), // CPlugTree : CPlug
    (0x09051000, 0x0902B000), // CPlugTreeGenerator : CPlug
    (0x09052000, 0x0901A000), // CPlugSoundGauge : CPlugSound
    (0x09053000, 0x0902B000), // CPlugGpuCompileCache : CPlug
    (0x09054000, 0x09041000), // CPlugFileTextScript : CPlugFileText
    (0x09055000, 0x09020000), // CPlugFileI18n : CPlugFile
    (0x09056000, 0x0902B000), // CPlugVertexStream : CPlug
    (0x09057000, 0x0902B000), // CPlugIndexBuffer : CPlug
    (0x09058000, 0x09086000), // CPlugBitmapRenderHemisphere : CPlugBitmapRender
    (0x0905A000, 0x09030000), // CPlugFileOggVorbis : CPlugFileSnd
    (0x0905B000, 0x09086000), // CPlugBitmapRenderPortal : CPlugBitmapRender
    (0x0905C000, 0x09086000), // CPlugBitmapRenderPlaneR : CPlugBitmapRender
    (0x0905D000, 0x01001000), // CPlugSimuDump : CMwNod
    (0x0905E000, 0x0901A000), // CPlugSoundSurface : CPlugSound
    (0x0905F000, 0x09020000), // CPlugFileBink : CPlugFile
    (0x09060000, 0x09025000), // CPlugFileVideo : CPlugFileImg
    (0x09061000, 0x01001000), // CPlugLocatedSound : CMwNod
    (0x09062000, 0x0904F000), // CPlugTreeLight : CPlugTree
    (0x09064000, 0x0901A000), // CPlugSoundMulti : CPlugSound
    (0x09065000, 0x0901A000), // CPlugSoundVideo : CPlugSound
    (0x09066000, 0x0902B000), // CPlugPointsInSphereOpt : CPlug
    (0x09067000, 0x0902B000), // CPlugShaderPass : CPlug
    (0x0906A000, 0x0902C000), // CPlugVisualIndexed : CPlugVisual3D
    (0x0906B000, 0x01001000), // NPlugSkel::SLodSetup : CMwNod
    (0x0906C000, 0x09020000), // CPlugFileSvg : CPlugFile
    (0x09072000, 0x01001000), // CPlugModelTree : CMwNod
    (0x09073000, 0x01001000), // CPlugModelMesh : CMwNod
    (0x09074000, 0x09075000), // CPlugFileVHlsl : CPlugFileGPUV
    (0x09075000, 0x09040000), // CPlugFileGPUV : CPlugFileGPU
    (0x09076000, 0x09040000), // CPlugFileGPUP : CPlugFileGPU
    (0x09077000, 0x09076000), // CPlugFilePHlsl : CPlugFileGPUP
    (0x09078000, 0x0902B000), // CPlugBitmapDecals : CPlug
    (0x09079000, 0x0902B000), // CPlugMaterial : CPlug
    (0x0907A000, 0x0902B000), // CPlugMaterialFx : CPlug
    (0x0907B000, 0x0907A000), // CPlugMaterialFxFlags : CPlugMaterialFx
    (0x0907C000, 0x0907A000), // CPlugMaterialFxFur : CPlugMaterialFx
    (0x0907D000, 0x0907A000), // CPlugMaterialFxs : CPlugMaterialFx
    (0x0907E000, 0x0902B000), // CPlugBitmapSampler : CPlug
    (0x09080000, 0x0902B000), // CPlugBitmapShader : CPlug
    (0x09081000, 0x0907A000), // CPlugMaterialFxDynaBump : CPlugMaterialFx
    (0x09082000, 0x0907A000), // CPlugMaterialFxDynaMobil : CPlugMaterialFx
    (0x09083000, 0x01001000), // CPlugScriptWithSettings : CMwNod
    (0x09084000, 0x09035000), // CPlugFileZip : CPlugFileFidContainer
    (0x09085000, 0x09020000), // CPlugFileAudioMotors : CPlugFile
    (0x09086000, 0x0902B000), // CPlugBitmapRender : CPlug
    (0x09087000, 0x09086000), // CPlugBitmapRenderWater : CPlugBitmapRender
    (0x09088000, 0x09086000), // CPlugBitmapRenderCubeMap : CPlugBitmapRender
    (0x09089000, 0x09086000), // CPlugBitmapRenderCamera : CPlugBitmapRender
    (0x0908A000, 0x09086000), // CPlugBitmapRenderVDepPlaneY : CPlugBitmapRender
    (0x0908B000, 0x09030000), // CPlugFileSndGen : CPlugFileSnd
    (0x0908C000, 0x0907A000), // CPlugMaterialFxGenCV : CPlugMaterialFx
    (0x0908E000, 0x0901A000), // CPlugSoundEngine : CPlugSound
    (0x0908F000, 0x01001000), // CPlugSoundComponent : CMwNod
    (0x09090000, 0x09086000), // CPlugBitmapRenderSolid : CPlugBitmapRender
    (0x09091000, 0x09086000), // CPlugBitmapRenderSub : CPlugBitmapRender
    (0x09092000, 0x09051000), // CPlugModel : CPlugTreeGenerator
    (0x09093000, 0x01001000), // CPlugIconIndex : CMwNod
    (0x09094000, 0x01001000), // CPlugVehicleGearBox : CMwNod
    (0x09095000, 0x01001000), // CPlugAdnAnimClip : CMwNod
    (0x09096000, 0x01001000), // NPlugAdn::STagDatabase : CMwNod
    (0x09098000, 0x09020000), // CPlugFileModel : CPlugFile
    (0x09099000, 0x09098000), // CPlugFileModelObj : CPlugFileModel
    (0x0909A000, 0x09051000), // CPlugTreeGenSolid : CPlugTreeGenerator
    (0x0909B000, 0x09098000), // CPlugFileModel3ds : CPlugFileModel
    (0x0909C000, 0x01001000), // CPlugModelLodMesh : CMwNod
    (0x0909D000, 0x01001000), // CPlugModelFur : CMwNod
    (0x0909E000, 0x09086000), // CPlugBitmapRenderOverlay : CPlugBitmapRender
    (0x0909F000, 0x09086000), // CPlugBitmapRenderLightOcc : CPlugBitmapRender
    (0x090A0000, 0x01001000), // CPlugViewDepLocator : CMwNod
    (0x090A2000, 0x01001000), // CPlugDecoratorTree : CMwNod
    (0x090A3000, 0x01001000), // CPlugDecoratorSolid : CMwNod
    (0x090A4000, 0x01001000), // CPlugModelFences : CMwNod
    (0x090A5000, 0x09098000), // CPlugFileModelFbx : CPlugFileModel
    (0x090A6000, 0x01001000), // CPlugFurWind : CMwNod
    (0x090A7000, 0x0902B000), // CPlugDecalModel : CPlug
    (0x090A8000, 0x0902B000), // CPlugBitmapAtlas : CPlug
    (0x090AA000, 0x0902B000), // CPlugSphericalHarmonics : CPlug
    (0x090AB000, 0x0902B000), // CPlugBitmapArray : CPlug
    (0x090AC000, 0x0902B000), // CPlugSpriteParam : CPlug
    (0x090AD000, 0x09025000), // CPlugFileExr : CPlugFileImg
    (0x090AE000, 0x0902B000), // CPlugPoissonDiscDistribution : CPlug
    (0x090B0000, 0x01001000), // CPlugAnimFile : CMwNod
    (0x090B1000, 0x09006000), // CPlugVisualCelEdge : CPlugVisual
    (0x090B2000, 0x01001000), // CPlugParticleEmitterSubModel : CMwNod
    (0x090B3000, 0x01001000), // CPlugParticleEmitterModel : CMwNod
    (0x090B4000, 0x01001000), // CPlugBeamEmitterModel : CMwNod
    (0x090B5000, 0x01001000), // CPlugParticleSplashModel : CMwNod
    (0x090B6000, 0x01001000), // CPlugParticleImpactModel : CMwNod
    (0x090B7000, 0x01001000), // CPlugParticleMaterialImpactModel : CMwNod
    (0x090B8000, 0x0902B000), // CPlugBitmapApplyArray : CPlug
    (0x090B9000, 0x01001000), // CPlugOpModel : CMwNod
    (0x090BA000, 0x01001000), // CPlugSkel : CMwNod
    (0x090BB000, 0x01001000), // CPlugSolid2Model : CMwNod
    (0x090BC000, 0x09098000), // CPlugFileModelCollada : CPlugFileModel
    (0x090BD000, 0x0902B000), // CPlugTimedPixelArray : CPlug
    (0x090BE000, 0x01001000), // CPlugResource : CMwNod
    (0x090BF000, 0x01001000), // CPlugWeatherModel : CMwNod
    (0x090C0000, 0x09035000), // CPlugFileFidContainer_SystemUserSaveProxy : CPlugFileFidContainer
    (0x090C1000, 0x0902B000), // CPlugFxLensFlareArray : CPlug
    (0x090C4000, 0x01001000), // CPlugParticleEmitterSubModelGpu : CMwNod
    (0x090C5000, 0x01001000), // CPlugParticleGpuSpawn : CMwNod
    (0x090C6000, 0x01001000), // CPlugParticleGpuModel : CMwNod
    (0x090C7000, 0x01001000), // CPlugCharVisModel : CMwNod
    (0x090C8000, 0x01001000), // CPlugSkelSetup : CMwNod
    (0x090C9000, 0x01001000), // CPlugCharPhyModel : CMwNod
    (0x090CA000, 0x01001000), // CPlugCharPhyMaterial : CMwNod
    (0x090CB000, 0x01001000), // CPlugFxLensDirtGen : CMwNod
    (0x090CC000, 0x01001000), // CPlugShieldEmitterModel : CMwNod
    (0x090CD000, 0x01001000), // CPlugBulletModel : CMwNod
    (0x090CE000, 0x01001000), // CPlugDataTape : CMwNod
    (0x090CF000, 0x01001000), // CPlugSpline3D : CMwNod
    (0x090D3000, 0x0902B000), // CPlugFogMatter : CPlug
    (0x090D4000, 0x0902B000), // CPlugFogVolume : CPlug
    (0x090D5000, 0x090D4000), // CPlugFogVolumeBox : CPlugFogVolume
    (0x090D6000, 0x01001000), // CPlugDestructibleFx : CMwNod
    (0x090D7000, 0x01001000), // CPlugDynaPointModel : CMwNod
    (0x090D8000, 0x0902B000), // CPlugFxLightning : CPlug
    (0x090D9000, 0x0902B000), // CPlugFxWindOnDecal : CPlug
    (0x090DB000, 0x09025000), // CPlugFileWebP : CPlugFileImg
    (0x090DC000, 0x0902B000), // CPlugMaterialPack : CPlug
    (0x090DD000, 0x01001000), // NPlugItemPlacement::SDatabase : CMwNod
    (0x090E0000, 0x01001000), // CPlugCharPhyRecoilModel : CMwNod
    (0x090E1000, 0x0902B000), // CPlugFxWindOnTreeSprite : CPlug
    (0x090E5000, 0x01001000), // CPlugFlockModel : CMwNod
    (0x090E6000, 0x01001000), // CPlugVehicleVisEmitterModel : CMwNod
    (0x090E7000, 0x01001000), // CPlugVehicleVisModel : CMwNod
    (0x090E8000, 0x01001000), // CPlugVehicleVisModelShared : CMwNod
    (0x090E9000, 0x01001000), // CPlugVehicleMaterialGroup : CMwNod
    (0x090EF000, 0x0910C000), // CPlugVehicleCameraRace3Model : CPlugCamControlModel
    (0x090F0000, 0x01001000), // CPlugBodyPath : CMwNod
    (0x090F2000, 0x01001000), // CPlugCharPhySpecialProperty : CMwNod
    (0x090F3000, 0x01001000), // CPlugParticleGpuVortex : CMwNod
    (0x090F4000, 0x01001000), // CPlugGameSkin : CMwNod
    (0x090F5000, 0x0902B000), // CPlugFxHdrScales_Tech3 : CPlug
    (0x090F6000, 0x0910C000), // CPlugVehicleCameraRace2Model : CPlugCamControlModel
    (0x090F7000, 0x0910C000), // CPlugVehicleCameraInternalModel : CPlugCamControlModel
    (0x090F8000, 0x01001000), // CPlugAnimLocSimple : CMwNod
    (0x090F9000, 0x01001000), // CPlugLightUserModel : CMwNod
    (0x090FB000, 0x01001000), // CPlugCharPhyMaterials : CMwNod
    (0x090FC000, 0x01001000), // CPlugCharPhyModelCustom : CMwNod
    (0x090FD000, 0x01001000), // CPlugMaterialUserInst : CMwNod
    (0x090FE000, 0x0902B000), // CPlugMoodSetting : CPlug
    (0x090FF000, 0x0902B000), // CPlugMoodAtmo : CPlug
    (0x09100000, 0x01001000), // CPlugBodyGraph : CMwNod
    (0x09103000, 0x01001000), // CPlugCustomBulletModel : CMwNod
    (0x09104000, 0x01001000), // CPlugTriggerAction : CMwNod
    (0x09105000, 0x01001000), // CPlugBeamEmitterSubModel : CMwNod
    (0x09106000, 0x0902B000), // CPlugProbe : CPlug
    (0x09107000, 0x01001000), // CPlugCustomBeamModel : CMwNod
    (0x09108000, 0x09060000), // CPlugFileWebM : CPlugFileVideo
    (0x0910A000, 0x01001000), // CPlugCharVisModelCustom : CMwNod
    (0x0910B000, 0x01001000), // CPlugCamShakeModel : CMwNod
    (0x0910C000, 0x01001000), // CPlugCamControlModel : CMwNod
    (0x0910D000, 0x01001000), // CPlugImportMeshParam : CMwNod
    (0x0910E000, 0x01001000), // CPlugVehicleCarPhyShape : CMwNod
    (0x0910F000, 0x01001000), // CPlugTurret : CMwNod
    (0x09110000, 0x01001000), // CPlugVehicleCamInternalVisOffset : CMwNod
    (0x09111000, 0x01001000), // CPlugShieldModel : CMwNod
    (0x09112000, 0x0910C000), // CPlugVehicleCameraRaceModel : CPlugCamControlModel
    (0x09113000, 0x0910C000), // CPlugVehicleCameraHmdExternalModel : CPlugCamControlModel
    (0x09114000, 0x01001000), // CPlugVehicleVisGeomModel : CMwNod
    (0x09115000, 0x01001000), // CPlugVisEntFxModel : CMwNod
    (0x09116000, 0x01001000), // CPlugGpuBuffer : CMwNod
    (0x09118000, 0x01001000), // CPlugPolyLine3 : CMwNod
    (0x09119000, 0x01001000), // CPlugPath : CMwNod
    (0x0911A000, 0x0902B000), // CPlugMoodBlender : CPlug
    (0x0911B000, 0x01001000), // CPlugTrainModel : CMwNod
    (0x0911C000, 0x01001000), // CPlugTrainWagonModel : CMwNod
    (0x0911D000, 0x0902B000), // CPlugMoodCurve : CPlug
    (0x0911E000, 0x01001000), // CPlugVehiclePhyModelCustom : CMwNod
    (0x0911F000, 0x01001000), // CPlugEntRecordData : CMwNod
    (0x09121000, 0x01001000), // CPlugTrainWagonModelCustom : CMwNod
    (0x09123000, 0x01001000), // CPlugEntitySpawner : CMwNod
    (0x09125000, 0x01001000), // CPlugWeather_DayTimeElem_Compat : CMwNod
    (0x09126000, 0x01001000), // CPlugWeather_WindBlockerElem : CMwNod
    (0x09127000, 0x01001000), // CPlugDynaConstraintModel : CMwNod
    (0x09128000, 0x01001000), // CPlugRoadChunk : CMwNod
    (0x0912B000, 0x01001000), // CPlugAnimGraph : CMwNod
    (0x0912F000, 0x01001000), // CPlugDynaModel : CMwNod
    (0x09131000, 0x01001000), // CPlugAdnPart : CMwNod
    (0x09132000, 0x01001000), // CPlugAnimClipBaked : CMwNod
    (0x09133000, 0x01001000), // CPlugAnimChannelGroup : CMwNod
    (0x09134000, 0x01001000), // CPlugAnimClipEdition : CMwNod
    (0x09135000, 0x01001000), // CPlugAnimClip : CMwNod
    (0x09136000, 0x01001000), // CPlugAnimPoseGrid : CMwNod
    (0x09137000, 0x01001000), // CPlugAnimPoseGroup : CMwNod
    (0x09138000, 0x01001000), // CPlugAnimGraphStack : CMwNod
    (0x0913A000, 0x01001000), // CPlugAnimClipEditionPose : CMwNod
    (0x0913B000, 0x01001000), // CPlugAnimVariantGroup : CMwNod
    (0x0913E000, 0x01001000), // CPlugAdnModel : CMwNod
    (0x0913F000, 0x01001000), // CPlugAdnProject : CMwNod
    (0x09140000, 0x01001000), // CPlugAdnRandomGen : CMwNod
    (0x09141000, 0x01001000), // CPlugAnimImport : CMwNod
    (0x09142000, 0x01001000), // CPlugMaterial_VertexIndex : CMwNod
    (0x09143000, 0x01001000), // CPlugAdnRandomGroup : CMwNod
    (0x09144000, 0x01001000), // CPlugDynaObjectModel : CMwNod
    (0x09145000, 0x01001000), // CPlugPrefab : CMwNod
    (0x09146000, 0x01001000), // CPlugVehicleVisStyles : CMwNod
    (0x09147000, 0x01001000), // CPlugVehicleVisStyleRandomGroup : CMwNod
    (0x09148000, 0x01001000), // CPlugCitizenModel : CMwNod
    (0x09149000, 0x01001000), // CPlugFxAnimFromTexture1dArray : CMwNod
    (0x0914A000, 0x01001000), // CPlugAdnTagFidCache : CMwNod
    (0x0914B000, 0x01001000), // CPlugMetaData : CMwNod
    (0x0914C000, 0x01001000), // CPlugImageArray : CMwNod
    (0x0914D000, 0x0910C000), // CPlugVehicleCameraHelicoModel : CPlugCamControlModel
    (0x09150000, 0x01001000), // CPlugRecastPolyMeshData : CMwNod
    (0x09153000, 0x01001000), // CPlugVFXFile : CMwNod
    (0x09154000, 0x01001000), // CPlugSymlink : CMwNod
    (0x09155000, 0x01001000), // CPlugAnimRigUIConfig : CMwNod
    (0x09157000, 0x01001000), // CPlugAdnRandomGenList : CMwNod
    (0x09159000, 0x01001000), // CPlugStaticObjectModel : CMwNod
    (0x0915A000, 0x01001000), // CPlugLightMapCustom : CMwNod
    (0x0915C000, 0x01001000), // CPlugFxSystem : CMwNod
    (0x0915D000, 0x01001000), // CPlugGameSkinAndFolder : CMwNod
    (0x0915E000, 0x01001000), // CPlugMaterialColorTargetTable : CMwNod
    (0x0915F000, 0x01001000), // CPlugDynaWaterModel : CMwNod
    (0x09160000, 0x09128000), // CPlugPlacementPatch : -
    (0x09161000, 0x09128000), // CPlugRoadChunkTraffic : -
    (0x09162000, 0x09128000), // CPlugRoadChunkCitizen : -
    (0x09164000, 0x01001000), // CPlugAnimJointExprGroup : CMwNod
    (0x09165000, 0x01001000), // NPlugModelKit::SDataBaseDesc : CMwNod
    (0x09166000, 0x01001000), // NPlugModelKit::SDataBase : CMwNod
    (0x09178000, 0x01001000), // NPlugTrigger::SWaypoint : CMwNod
    (0x09179000, 0x01001000), // NPlugTrigger::SSpecial : CMwNod
    (0x0917A000, 0x01001000), // CPlugSpawnModel : CMwNod
    (0x0917B000, 0x01001000), // CPlugEditorHelper : CMwNod
    (0x0917D000, 0x01001000), // CPlugMaterialWaterArray : CMwNod
    (0x0917E000, 0x01001000), // CPlugWeather : CMwNod
    (0x0917F000, 0x01001000), // CPlugPuffLull : CMwNod
    (0x09180000, 0x01001000), // CPlugClouds : CMwNod
    (0x09181000, 0x01001000), // CPlugDayTime : CMwNod
    (0x09182000, 0x01001000), // CPlugCloudsParam : CMwNod
    (0x09183000, 0x01001000), // CPlugCloudsSolids : CMwNod
    (0x09184000, 0x01001000), // CPlugCurveEnvelopeDeprec : CMwNod
    (0x09185000, 0x01001000), // CPlugCurveSimpleNod : CMwNod
    (0x09186000, 0x01001000), // CPlugBitmapArrayBuilder : CMwNod
    (0x09187000, 0x01001000), // NPlugItemPlacement::SClass : CMwNod
    (0x09188000, 0x01001000), // CPlugPodium : CMwNod
    (0x09189000, 0x01001000), // CPlugMediaClipList : CMwNod
    (0x0918B000, 0x01001000), // NPlugAnim::SRig : CMwNod
    (0x0918C000, 0x01001000), // NPlugAnim::SRigToSkel : CMwNod
    (0x0918D000, 0x01001000), // NPlugAnim::SRigToSkelNode : CMwNod
    (0x0918E000, 0x0918D000), // NPlugAnim::SRigToSkelNode_Chain2 : -
    (0x0918F000, 0x0918D000), // NPlugAnim::SRigToSkelNode_Fixed : -
    (0x09190000, 0x0918D000), // NPlugAnim::SRigToSkelNode_SetDOV : -
    (0x09191000, 0x0918D000), // NPlugAnim::SRigToSkelNode_SetPos : -
    (0x0A000000, 0x01003000), // CSceneEngine : CMwEngine
    (0x0A001000, 0x01001000), // CScene : CMwNod
    (0x0A002000, 0x0A001000), // CScene2d : CScene
    (0x0A003000, 0x01001000), // CSceneLayout : CMwNod
    (0x0A004000, 0x01001000), // CSceneSector : CMwNod
    (0x0A005000, 0x01001000), // CSceneObject : CMwNod
    (0x0A006000, 0x01001000), // CSceneMgrGUI : CMwNod
    (0x0A007000, 0x0A005000), // CSceneLocation : CSceneObject
    (0x0A009000, 0x0A005000), // CScenePoc : CSceneObject
    (0x0A00B000, 0x0A009000), // CSceneLight : CScenePoc
    (0x0A011000, 0x0A005000), // CSceneMobil : CSceneObject
    (0x0A013000, 0x01001000), // CSceneCloudSystem : CMwNod
    (0x0A017000, 0x01001000), // CScenePickerManager : CMwNod
    (0x0A034000, 0x0A076000), // CSceneFxColors : CSceneFxCompo
    (0x0A035000, 0x0A076000), // CSceneFxSuperSample : CSceneFxCompo
    (0x0A036000, 0x0A007000), // CSceneLocationCamera : CSceneLocation
    (0x0A038000, 0x0A076000), // CSceneFxFlares : CSceneFxCompo
    (0x0A03A000, 0x01001000), // CSceneFxNod : CMwNod
    (0x0A03B000, 0x0A076000), // CSceneFxBloom : CSceneFxCompo
    (0x0A03F000, 0x01001000), // CSceneFxBloomData : CMwNod
    (0x0A040000, 0x01001000), // CSceneConfig : CMwNod
    (0x0A041000, 0x01001000), // CSceneConfigVision : CMwNod
    (0x0A043000, 0x0A076000), // CSceneFxStereoscopy : CSceneFxCompo
    (0x0A044000, 0x0A076000), // CSceneFxHeadTrack : CSceneFxCompo
    (0x0A072000, 0x01001000), // CSceneFx : CMwNod
    (0x0A074000, 0x0A072000), // CSceneFxOverlay : CSceneFx
    (0x0A076000, 0x0A072000), // CSceneFxCompo : CSceneFx
    (0x0A077000, 0x0A076000), // CSceneFxDepthOfField : CSceneFxCompo
    (0x0A079000, 0x0A076000), // CSceneFxCameraBlend : CSceneFxCompo
    (0x0A07A000, 0x0A076000), // CSceneFxBlur : CSceneFxCompo
    (0x0A07B000, 0x0A076000), // CSceneFxDistor2d : CSceneFxCompo
    (0x0A07D000, 0x0A076000), // CSceneFxEdgeBlender : CSceneFxCompo
    (0x0A081000, 0x01001000), // CSceneVehicleCarMarksModel : CMwNod
    (0x0A082000, 0x01001000), // CSceneVehicleCarMarksModelSub : CMwNod
    (0x0A083000, 0x01001000), // CSceneVehicleCarMarksSamples : CMwNod
    (0x0A084000, 0x0A076000), // CSceneFxCellEdge : CSceneFxCompo
    (0x0A086000, 0x01001000), // CSceneFxMgr : CMwNod
    (0x0B000000, 0x01003000), // CSystemEngine : CMwEngine
    (0x0B001000, 0x0B007000), // CSystemMouse : CNodSystem
    (0x0B002000, 0x0B007000), // CSystemKeyboard : CNodSystem
    (0x0B003000, 0x0B007000), // CSystemWindow : CNodSystem
    (0x0B005000, 0x01001000), // CSystemConfig : CMwNod
    (0x0B006000, 0x01001000), // CSystemMemoryMonitor : CMwNod
    (0x0B007000, 0x01001000), // CNodSystem : CMwNod
    (0x0B009000, 0x01001000), // CSystemFidsFolder : CMwNod
    (0x0B00A000, 0x01001000), // CSystemFidFile : CMwNod
    (0x0B00C000, 0x0B009000), // CSystemFidsDrive : CSystemFidsFolder
    (0x0B00F000, 0x01001000), // CSystemPlatformScript : CMwNod
    (0x0B010000, 0x01001000), // CSystemDependenciesList : CMwNod
    (0x0B013000, 0x01001000), // CSystemConfigDisplay : CMwNod
    (0x0B014000, 0x01001000), // CSystemPackManager : CMwNod
    (0x0B015000, 0x01001000), // CSystemPackDesc : CMwNod
    (0x0B017000, 0x01001000), // CSystemNodWrapper : CMwNod
    (0x0B018000, 0x01001000), // CSystemData : CMwNod
    (0x0B01A000, 0x01001000), // CSystemFidContainer : CMwNod
    (0x0C001000, 0x06010000), // CVisionViewport : CHmsViewport
    (0x0C003000, 0x0C001000), // CVisionViewportNull : CVisionViewport
    (0x0C012000, 0x01001000), // CVisionResourceFile : CMwNod
    (0x0C014000, 0x0C001000), // CDx11Viewport : CVisionViewport
    (0x0C019000, 0x01001000), // CVisionResourceShaders : CMwNod
    (0x0C030000, 0x0A076000), // CVisPostFx_ToneMapping : CSceneFxCompo
    (0x0C031000, 0x0A076000), // CVisPostFx_MotionBlur : CSceneFxCompo
    (0x0C032000, 0x0A076000), // CVisPostFx_BloomHdr : CSceneFxCompo
    (0x10001000, 0x01001000), // CAudioPort : CMwNod
    (0x10002000, 0x10001000), // CAudioPortNull : CAudioPort
    (0x10003000, 0x01001000), // CAudioSoundImplem : CMwNod
    (0x10004000, 0x01001000), // CAudioBufferKeeper : CMwNod
    (0x10005000, 0x01001000), // CAudioListener : CMwNod
    (0x10006000, 0x01001000), // CAudioZone : CMwNod
    (0x10007000, 0x01001000), // CAudioScriptManager : CMwNod
    (0x10008000, 0x01001000), // CAudioScriptSound : CMwNod
    (0x10009000, 0x10008000), // CAudioScriptMusic : CAudioScriptSound
    (0x1000F000, 0x01001000), // CAudioZoneSource : CMwNod
    (0x10010000, 0x01001000), // CAudioSource : CMwNod
    (0x10011000, 0x10010000), // CAudioSourceMusic : CAudioSource
    (0x10012000, 0x10010000), // CAudioSourceEngine : CAudioSource
    (0x10013000, 0x10010000), // CAudioSourceSurface : CAudioSource
    (0x10014000, 0x10010000), // CAudioSourceMulti : CAudioSource
    (0x10015000, 0x10010000), // CAudioSourceMood : CAudioSource
    (0x10016000, 0x10010000), // CAudioSourceGauge : CAudioSource
    (0x10030000, 0x10001000), // COalAudioPort : CAudioPort
    (0x10031000, 0x10004000), // COalAudioBufferKeeper : CAudioBufferKeeper
    (0x11000000, 0x01001000), // CScriptSetting : CMwNod
    (0x11001000, 0x01001000), // CScriptTraitsPersistent : CMwNod
    (0x11002000, 0x01001000), // CScriptTraitsMetadata : CMwNod
    (0x11003000, 0x01001000), // CScriptInterfacableValue : CMwNod
    (0x11004000, 0x11005000), // CScriptBaseEvent : CScriptBaseConstEvent
    (0x11005000, 0x01001000), // CScriptBaseConstEvent : CMwNod
    (0x11006000, 0x01001000), // CScriptPoison : CMwNod
    (0x12001000, 0x01001000), // CNetNod : CMwNod
    (0x12002000, 0x01001000), // CNetServerInfo : CMwNod
    (0x12003000, 0x01001000), // CNetClientInfo : CMwNod
    (0x12004000, 0x12001000), // CNetFormTimed : CNetNod
    (0x12007000, 0x12001000), // CNetFormQuerrySessions : CNetNod
    (0x12008000, 0x12001000), // CNetFormEnumSessions : CNetNod
    (0x12009000, 0x12004000), // CNetFormPing : CNetFormTimed
    (0x1200C000, 0x01001000), // CNetServer : CMwNod
    (0x1200D000, 0x01001000), // CNetClient : CMwNod
    (0x1200F000, 0x01001000), // CNetConnection : CMwNod
    (0x12010000, 0x12001000), // CNetFormConnectionAdmin : CNetNod
    (0x12012000, 0x01001000), // CNetHttpClient : CMwNod
    (0x12013000, 0x01001000), // CNetHttpResult : CMwNod
    (0x12014000, 0x01001000), // CNetMasterServer : CMwNod
    (0x12015000, 0x01001000), // CNetMasterHost : CMwNod
    (0x12018000, 0x01001000), // CNetFileTransfer : CMwNod
    (0x12019000, 0x01001000), // CNetMasterServerInfo : CMwNod
    (0x1201A000, 0x01001000), // CNetFileTransferNod : CMwNod
    (0x1201B000, 0x12001000), // CNetFileTransferForm : CNetNod
    (0x1201C000, 0x1201A000), // CNetFileTransferDownload : CNetFileTransferNod
    (0x1201D000, 0x1201A000), // CNetFileTransferUpload : CNetFileTransferNod
    (0x1201E000, 0x01001000), // CNetSource : CMwNod
    (0x12020000, 0x01001000), // CNetIPC : CMwNod
    (0x12021000, 0x12001000), // CNetFormRpcCall : CNetNod
    (0x12022000, 0x01001000), // CNetUPnP : CMwNod
    (0x12026000, 0x1203C000), // CWebServicesTaskWait : CWebServicesTaskSequence
    (0x12027000, 0x01001000), // CNetMasterServerRequest : CMwNod
    (0x12028000, 0x01001000), // CNetIPSource : CMwNod
    (0x12029000, 0x01001000), // CNetMasterServerUptoDateCheck : CMwNod
    (0x12030000, 0x01001000), // CNetURLSource : CMwNod
    (0x12031000, 0x01001000), // CNetScriptHttpManager : CMwNod
    (0x12032000, 0x01001000), // CNetScriptHttpRequest : CMwNod
    (0x12033000, 0x01001000), // CNetXmpp_Timer : CMwNod
    (0x12034000, 0x01001000), // CNetMasterServerDownload : CMwNod
    (0x12037000, 0x1203C000), // CWebServicesTaskWaitMultiple : CWebServicesTaskSequence
    (0x12038000, 0x12001000), // CNetFormNewPing : CNetNod
    (0x12039000, 0x01001000), // CNetMasterServerUserInfo : CMwNod
    (0x1203B000, 0x01001000), // CWebServicesTask : CMwNod
    (0x1203C000, 0x1203B000), // CWebServicesTaskSequence : CWebServicesTask
    (0x1203D000, 0x01001000), // CWebServicesTaskResult : CMwNod
    (0x1203E000, 0x01001000), // CWebServicesTaskScheduler : CMwNod
    (0x1203F000, 0x01001000), // CNetUbiServices : CMwNod
    (0x12040000, 0x1203C000), // CNetUbiServicesTask : CWebServicesTaskSequence
    (0x12041000, 0x12040000), // CNetUbiServicesTask_CreateSession : CNetUbiServicesTask
    (0x12042000, 0x12040000), // CNetUbiServicesTask_DeleteSession : CNetUbiServicesTask
    (0x12045000, 0x12040000), // CNetUbiServicesTask_Party_GetMaxMemberLimit : CNetUbiServicesTask
    (0x12046000, 0x12040000), // CNetUbiServicesTask_Party_SetMaxMemberLimit : CNetUbiServicesTask
    (0x12047000, 0x1203C000), // CWebServicesTask_Party_SetMaxMemberLimit : CWebServicesTaskSequence
    (0x12048000, 0x120A5000), // CWebServicesTask_Empty : CWebServicesTaskVoid
    (0x12049000, 0x01001000), // CNetUplayPC : CMwNod
    (0x1204A000, 0x120B0000), // CNetNadeoServicesTask_GetAccountIdFromWebServicesIdentity : CNetNadeoServicesRequestTask
    (0x1204C000, 0x12040000), // CNetUbiServicesTask_Profile_RetrieveProfileInfoList : CNetUbiServicesTask
    (0x1204D000, 0x1203C000), // CWebServicesTask_GetDisplayNameFromWebServicesUserId : CWebServicesTaskSequence
    (0x1204E000, 0x1203C000), // CWebServicesTask_GetDisplayNameFromWebServicesIdentity : CWebServicesTaskSequence
    (0x1204F000, 0x1203C000), // CNetMasterServerRequestTask : CWebServicesTaskSequence
    (0x12050000, 0x1203D000), // CWebServicesTaskResult_Bool : CWebServicesTaskResult
    (0x12051000, 0x1203D000), // CWebServicesTaskResult_String : CWebServicesTaskResult
    (0x12056000, 0x1204F000), // CNetMasterServerTask_GetClientConfigUrls : CNetMasterServerRequestTask
    (0x12057000, 0x1203C000), // CNetUplayPCTask_Overlay_ShowMicroApp : CWebServicesTaskSequence
    (0x12058000, 0x12040000), // CNetUbiServicesTask_SendNotification : CNetUbiServicesTask
    (0x12059000, 0x1203C000), // CNetUbiServicesTask_CheckNewNotification : CWebServicesTaskSequence
    (0x1205E000, 0x1203C000), // CWebServicesTask_CheckNetworkAvailability : CWebServicesTaskSequence
    (0x12061000, 0x01001000), // CNetUplayPCUserInfo : CMwNod
    (0x12062000, 0x1203C000), // CWebServicesTask_GetFriendList : CWebServicesTaskSequence
    (0x12063000, 0x1203D000), // CWebServicesTaskResult_WSFriendList : CWebServicesTaskResult
    (0x12065000, 0x1204F000), // CNetMasterServerTask_GetApplicationConfig : CNetMasterServerRequestTask
    (0x12066000, 0x1203D000), // CWebServicesTaskResult_ClientConfig : CWebServicesTaskResult
    (0x12067000, 0x1204F000), // CNetMasterServerTask_GetWaitingParams : CNetMasterServerRequestTask
    (0x12068000, 0x1204F000), // CNetMasterServerTask_CheckLoginForSubscribe : CNetMasterServerRequestTask
    (0x12069000, 0x1204F000), // CNetMasterServerTask_Subscribe : CNetMasterServerRequestTask
    (0x1206A000, 0x1204F000), // CNetMasterServerTask_OpenSession : CNetMasterServerRequestTask
    (0x1206B000, 0x1204F000), // CNetMasterServerTask_Connect : CNetMasterServerRequestTask
    (0x1206F000, 0x1203D000), // CWebServicesTaskResult_Session_Get : CWebServicesTaskResult
    (0x1207F000, 0x1203C000), // CWebServicesTask_OpenNewsLink : CWebServicesTaskSequence
    (0x12080000, 0x12040000), // CNetUbiServicesTask_GetNews : CNetUbiServicesTask
    (0x12082000, 0x1203C000), // CNetMasterServerTask_Session_Get : CWebServicesTaskSequence
    (0x12083000, 0x1203C000), // CNetMasterServerTask_Session_JoinOrCreate : CWebServicesTaskSequence
    (0x12084000, 0x1203C000), // CNetMasterServerTask_Session_Leave : CWebServicesTaskSequence
    (0x12087000, 0x1203C000), // CNetMasterServerTask_Session_InviteBuddy : CWebServicesTaskSequence
    (0x12089000, 0x12040000), // CNetUbiServicesTask_Party_UpdateInvitation : CNetUbiServicesTask
    (0x1208A000, 0x1203C000), // CWebServicesTask_Party_CancelInvitation : CWebServicesTaskSequence
    (0x1208C000, 0x1203C000), // CWebServicesTask_GetUserPrestigeSelected : CWebServicesTaskSequence
    (0x1208D000, 0x120B0000), // CNetNadeoServicesTask_AddSubscriptionFromPSN : CNetNadeoServicesRequestTask
    (0x12094000, 0x1204F000), // CNetMasterServerTask_GetFeatureTimeLimit : CNetMasterServerRequestTask
    (0x12095000, 0x1203D000), // CWebServicesTaskResult_Natural : CWebServicesTaskResult
    (0x12096000, 0x1204F000), // CNetMasterServerTask_CheckFeatureTimeLimit : CNetMasterServerRequestTask
    (0x12097000, 0x1204F000), // CNetMasterServerTask_SetFeatureTimeUse : CNetMasterServerRequestTask
    (0x12098000, 0x1203D000), // CWebServicesTaskResult_PlayerFeatureLimitList : CWebServicesTaskResult
    (0x12099000, 0x1203C000), // CWebServicesTask_GetStatList : CWebServicesTaskSequence
    (0x1209B000, 0x1203C000), // CWebServicesTask_GetBlockList : CWebServicesTaskSequence
    (0x1209D000, 0x1203D000), // CWebServicesTaskResult_StringInt : CWebServicesTaskResult
    (0x1209E000, 0x12040000), // CNetUbiServicesTask_GetUnsentEvents : CNetUbiServicesTask
    (0x1209F000, 0x12040000), // CNetUbiServicesTask_RefreshSession : CNetUbiServicesTask
    (0x120A0000, 0x120B0000), // CNetNadeoServicesTask_GetAllPrestigeList : CNetNadeoServicesRequestTask
    (0x120A1000, 0x1203D000), // CWebServicesTaskResult_NSAccountSkinFavorite : CWebServicesTaskResult
    (0x120A2000, 0x12040000), // CNetUbiServicesTask_RetrieveBetaUserInfo : CNetUbiServicesTask
    (0x120A3000, 0x12040000), // CNetUbiServicesTask_AcceptNDA : CNetUbiServicesTask
    (0x120A4000, 0x1203C000), // CNetUplayPCTask_Achievement_Unlock : CWebServicesTaskSequence
    (0x120A5000, 0x1203B000), // CWebServicesTaskVoid : CWebServicesTask
    (0x120A8000, 0x1204F000), // CNetMasterServerTask_ImportAccount : CNetMasterServerRequestTask
    (0x120A9000, 0x1204F000), // CNetMasterServerTask_ImportAccount_IsFinished : CNetMasterServerRequestTask
    (0x120AA000, 0x1203D000), // CWebServicesTaskResult_OpenSession : CWebServicesTaskResult
    (0x120AB000, 0x1203D000), // CWebServicesTaskResult_StringList : CWebServicesTaskResult
    (0x120AC000, 0x1203D000), // CWebServicesTaskResult_StringIntList : CWebServicesTaskResult
    (0x120AD000, 0x01001000), // CNetScriptHttpEvent : CMwNod
    (0x120AE000, 0x01001000), // CNetNadeoServices : CMwNod
    (0x120AF000, 0x01001000), // CNetNadeoServicesUserInfo : CMwNod
    (0x120B0000, 0x1203C000), // CNetNadeoServicesRequestTask : CWebServicesTaskSequence
    (0x120B1000, 0x01001000), // CNetNadeoServicesRequest : CMwNod
    (0x120B2000, 0x1203D000), // CWebServicesTaskResult_UrlConfig : CWebServicesTaskResult
    (0x120B3000, 0x120B0000), // CNetNadeoServicesTask_AuthenticateCommon : CNetNadeoServicesRequestTask
    (0x120B4000, 0x120B0000), // CNetNadeoServicesTask_GetClientConfig : CNetNadeoServicesRequestTask
    (0x120B5000, 0x120B3000), // CNetNadeoServicesTask_AuthenticateWithBasicCredentials : CNetNadeoServicesTask_AuthenticateCommon
    (0x120B6000, 0x120B3000), // CNetNadeoServicesTask_AuthenticateWithUbiServices : CNetNadeoServicesTask_AuthenticateCommon
    (0x120B7000, 0x120B3000), // CNetNadeoServicesTask_AuthenticateWithUnsecureAccountId : CNetNadeoServicesTask_AuthenticateCommon
    (0x120B8000, 0x1203C000), // CWebServicesTask_GetWebIdentityFromWebServicesUserId : CWebServicesTaskSequence
    (0x120B9000, 0x1203D000), // CWebServicesTaskResult_Connect : CWebServicesTaskResult
    (0x120BA000, 0x120B0000), // CNetNadeoServicesTask_GetAccountClientSignature : CNetNadeoServicesRequestTask
    (0x120BB000, 0x120B0000), // CNetNadeoServicesTask_SetClientCaps : CNetNadeoServicesRequestTask
    (0x120BC000, 0x1203D000), // CWebServicesTaskResult_Signature : CWebServicesTaskResult
    (0x120BD000, 0x120B0000), // CNetNadeoServicesTask_GetEncryptedPackageAccountKey : CNetNadeoServicesRequestTask
    (0x120BE000, 0x120B0000), // CNetNadeoServicesTask_GetZones : CNetNadeoServicesRequestTask
    (0x120BF000, 0x1203D000), // CWebServicesTaskResult_NSZoneList : CWebServicesTaskResult
    (0x120C0000, 0x01001000), // CNetNadeoServicesRequestManager : CMwNod
    (0x120C1000, 0x120B0000), // CNetNadeoServicesTask_GetAccountClientUrls : CNetNadeoServicesRequestTask
    (0x120C2000, 0x120B0000), // CNetNadeoServicesTask_GetAccountZone : CNetNadeoServicesRequestTask
    (0x120C3000, 0x01001000), // CWebServices : CMwNod
    (0x120C4000, 0x01001000), // CWebServicesUserInfo : CMwNod
    (0x120C5000, 0x1203C000), // CWebServicesTask_Connect : CWebServicesTaskSequence
    (0x120C6000, 0x120B0000), // CNetNadeoServicesTask_GetApiRequests : CNetNadeoServicesRequestTask
    (0x120C7000, 0x1203C000), // CWebServicesTask_ConnectToNadeoServices : CWebServicesTaskSequence
    (0x120C8000, 0x1203C000), // CWebServicesTask_UpdateClientConfig : CWebServicesTaskSequence
    (0x120C9000, 0x1203D000), // CWebServicesTaskResult_NSWebServicesIdentityList : CWebServicesTaskResult
    (0x120CA000, 0x120B0000), // CNetNadeoServicesTask_GetWebServicesIdentityFromAccountId : CNetNadeoServicesRequestTask
    (0x120CB000, 0x1203D000), // CWebServicesTaskResult_ApplicationConfig : CWebServicesTaskResult
    (0x120CC000, 0x120B0000), // CNetNadeoServicesTask_GetProfileChunkList : CNetNadeoServicesRequestTask
    (0x120CD000, 0x120B0000), // CNetNadeoServicesTask_SetProfileChunk : CNetNadeoServicesRequestTask
    (0x120CE000, 0x1203D000), // CWebServicesTaskResult_NSProfileChunk : CWebServicesTaskResult
    (0x120CF000, 0x120B0000), // CNetNadeoServicesTask_DeleteProfileChunk : CNetNadeoServicesRequestTask
    (0x120D0000, 0x120B0000), // CNetNadeoServicesTask_GetAccountStationList : CNetNadeoServicesRequestTask
    (0x120D2000, 0x120B0000), // CNetNadeoServicesTask_GetEncryptedPackageList : CNetNadeoServicesRequestTask
    (0x120D3000, 0x1203D000), // CWebServicesTaskResult_NSEncryptedPackageList : CWebServicesTaskResult
    (0x120D4000, 0x120B0000), // CNetNadeoServicesTask_GetClientFileList : CNetNadeoServicesRequestTask
    (0x120D5000, 0x1203D000), // CWebServicesTaskResult_NSClientFileList : CWebServicesTaskResult
    (0x120D6000, 0x120B0000), // CNetNadeoServicesTask_GetEncryptedPackageVersionList : CNetNadeoServicesRequestTask
    (0x120D7000, 0x1203D000), // CWebServicesTaskResult_NSEncryptedPackageVersionList : CWebServicesTaskResult
    (0x120D8000, 0x120B0000), // CNetNadeoServicesTask_GetEncryptedPackageVersionCryptKey : CNetNadeoServicesRequestTask
    (0x120D9000, 0x1203D000), // CWebServicesTaskResult_NSEncryptedPackageVersionCryptKey : CWebServicesTaskResult
    (0x120DA000, 0x120B0000), // CNetNadeoServicesTask_LoadStation : CNetNadeoServicesRequestTask
    (0x120DB000, 0x1203D000), // CWebServicesTaskResult_NSStation : CWebServicesTaskResult
    (0x120DC000, 0x120B0000), // CNetNadeoServicesTask_UnloadStation : CNetNadeoServicesRequestTask
    (0x120DD000, 0x120B0000), // CNetNadeoServicesTask_GetAccountLadderInfo : CNetNadeoServicesRequestTask
    (0x120DE000, 0x1203D000), // CWebServicesTaskResult_NSLadderAccountInfo : CWebServicesTaskResult
    (0x120DF000, 0x120B0000), // CNetNadeoServicesTask_GetAuthenticationToken : CNetNadeoServicesRequestTask
    (0x120E0000, 0x120B0000), // CNetNadeoServicesTask_GetAccountMapRecordList : CNetNadeoServicesRequestTask
    (0x120E1000, 0x1203D000), // CWebServicesTaskResult_NSMapRecordList : CWebServicesTaskResult
    (0x120E2000, 0x120B0000), // CNetNadeoServicesTask_SetMapRecordAttempt : CNetNadeoServicesRequestTask
    (0x120E4000, 0x120B0000), // CNetNadeoServicesTask_GetAccountGroupList : CNetNadeoServicesRequestTask
    (0x120E6000, 0x120B0000), // CNetNadeoServicesTask_GetAccountPolicyRuleValueList : CNetNadeoServicesRequestTask
    (0x120E7000, 0x1203D000), // CWebServicesTaskResult_NSPolicyRuleValueList : CWebServicesTaskResult
    (0x120E8000, 0x1203D000), // CWebServicesTaskResult_NSAccountZone : CWebServicesTaskResult
    (0x120E9000, 0x120B0000), // CNetNadeoServicesTask_SetAccountZone : CNetNadeoServicesRequestTask
    (0x120EB000, 0x120B0000), // CNetNadeoServicesTask_GetClientUpdaterFile : CNetNadeoServicesRequestTask
    (0x120ED000, 0x120B0000), // CNetNadeoServicesTask_GetAccountEncryptedPackageList : CNetNadeoServicesRequestTask
    (0x120EE000, 0x120B0000), // CNetNadeoServicesTask_GetAccountTitleList : CNetNadeoServicesRequestTask
    (0x120EF000, 0x1203D000), // CWebServicesTaskResult_NSTitleList : CWebServicesTaskResult
    (0x120F0000, 0x1203D000), // CWebServicesTaskResult_NSTitle : CWebServicesTaskResult
    (0x120F1000, 0x120B0000), // CNetNadeoServicesTask_SetTitle : CNetNadeoServicesRequestTask
    (0x120F2000, 0x1203D000), // CWebServicesTaskResult_NSEncryptedPackage : CWebServicesTaskResult
    (0x120F3000, 0x120B0000), // CNetNadeoServicesTask_SetEncryptedPackage : CNetNadeoServicesRequestTask
    (0x120F5000, 0x120B0000), // CNetNadeoServicesTask_CreateEncryptedPackageVersion : CNetNadeoServicesRequestTask
    (0x120F6000, 0x1203D000), // CWebServicesTaskResult_NSMap : CWebServicesTaskResult
    (0x120F7000, 0x120B0000), // CNetNadeoServicesTask_SetMap : CNetNadeoServicesRequestTask
    (0x120F9000, 0x120B0000), // CNetNadeoServicesTask_SetCampaign : CNetNadeoServicesRequestTask
    (0x120FA000, 0x1203D000), // CWebServicesTaskResult_NSMapList : CWebServicesTaskResult
    (0x120FB000, 0x120B0000), // CNetNadeoServicesTask_GetMapList : CNetNadeoServicesRequestTask
    (0x120FC000, 0x1203D000), // CWebServicesTaskResult_NSCampaignList : CWebServicesTaskResult
    (0x120FD000, 0x120B0000), // CNetNadeoServicesTask_GetCampaignList : CNetNadeoServicesRequestTask
    (0x120FE000, 0x1203D000), // CWebServicesTaskResult_NSEncryptedPackageVersionWithCryptKey : CWebServicesTaskResult
    (0x120FF000, 0x12040000), // CNetUbiServicesTask_RequestUserLegalOptinsStatus : CNetUbiServicesTask
    (0x12100000, 0x120B0000), // CNetNadeoServicesTask_GetAccountClientPluginList : CNetNadeoServicesRequestTask
    (0x12101000, 0x120B0000), // CNetNadeoServicesTask_AddMapListToCampaign : CNetNadeoServicesRequestTask
    (0x12102000, 0x120B0000), // CNetNadeoServicesTask_RemoveMapListFromCampaign : CNetNadeoServicesRequestTask
    (0x12103000, 0x120B0000), // CNetNadeoServicesTask_GetMap : CNetNadeoServicesRequestTask
    (0x12104000, 0x120B0000), // CNetNadeoServicesTask_GetAccountMapList : CNetNadeoServicesRequestTask
    (0x12105000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSkinFavoriteList : CNetNadeoServicesRequestTask
    (0x12106000, 0x1203D000), // CWebServicesTaskResult_NSAccountSkinFavoriteList : CWebServicesTaskResult
    (0x12107000, 0x120B0000), // CNetNadeoServicesTask_GetSeason : CNetNadeoServicesRequestTask
    (0x12108000, 0x1203D000), // CWebServicesTaskResult_NSSeason : CWebServicesTaskResult
    (0x12109000, 0x1203D000), // CWebServicesTaskResult_NSSeasonList : CWebServicesTaskResult
    (0x1210A000, 0x120B0000), // CNetNadeoServicesTask_GetSeasonList : CNetNadeoServicesRequestTask
    (0x1210B000, 0x120B0000), // CNetNadeoServicesTask_SetSeason : CNetNadeoServicesRequestTask
    (0x1210C000, 0x120B0000), // CNetNadeoServicesTask_AddMapListToSeason : CNetNadeoServicesRequestTask
    (0x1210D000, 0x120B0000), // CNetNadeoServicesTask_RemoveMapListFromSeason : CNetNadeoServicesRequestTask
    (0x1210E000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSeasonList : CNetNadeoServicesRequestTask
    (0x1210F000, 0x1203C000), // CNetUplayPCTask_JoinSession : CWebServicesTaskSequence
    (0x12110000, 0x1203C000), // CNetUplayPCTask_LeaveSession : CWebServicesTaskSequence
    (0x12111000, 0x1203C000), // CNetUplayPCTask_ShowInviteUI : CWebServicesTaskSequence
    (0x12112000, 0x1203D000), // CWebServicesTaskResult_NSServer : CWebServicesTaskResult
    (0x12113000, 0x120B0000), // CNetNadeoServicesTask_SetServer : CNetNadeoServicesRequestTask
    (0x12114000, 0x120B0000), // CNetNadeoServicesTask_GetServer : CNetNadeoServicesRequestTask
    (0x12115000, 0x120B0000), // CNetNadeoServicesTask_DeleteServer : CNetNadeoServicesRequestTask
    (0x12116000, 0x1203C000), // CWebServicesTask_CheckSubscription : CWebServicesTaskSequence
    (0x12117000, 0x1203C000), // CNetUplayPCTask_GetUserConsumableItemList : CWebServicesTaskSequence
    (0x12118000, 0x1203D000), // CWebServicesTaskResult_UPCConsumableItemList : CWebServicesTaskResult
    (0x12119000, 0x1203C000), // CWebServicesTask_UpdateUserConfig : CWebServicesTaskSequence
    (0x1211A000, 0x120B0000), // CNetNadeoServicesTask_AddSubscription : CNetNadeoServicesRequestTask
    (0x1211B000, 0x120B0000), // CNetNadeoServicesTask_CreateSkin : CNetNadeoServicesRequestTask
    (0x1211C000, 0x1203D000), // CWebServicesTaskResult_NSSkin : CWebServicesTaskResult
    (0x1211D000, 0x120B0000), // CNetNadeoServicesTask_GetSkinList : CNetNadeoServicesRequestTask
    (0x1211E000, 0x1203D000), // CWebServicesTaskResult_NSSkinList : CWebServicesTaskResult
    (0x1211F000, 0x120B0000), // CNetNadeoServicesTask_GetCreatorSkinList : CNetNadeoServicesRequestTask
    (0x12120000, 0x120B0000), // CNetNadeoServicesTask_SetAccountSkin : CNetNadeoServicesRequestTask
    (0x12121000, 0x1203D000), // CWebServicesTaskResult_NSAccountSkin : CWebServicesTaskResult
    (0x12122000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSkinList : CNetNadeoServicesRequestTask
    (0x12123000, 0x1203D000), // CWebServicesTaskResult_NSAccountSkinList : CWebServicesTaskResult
    (0x12124000, 0x120B0000), // CNetNadeoServicesTask_RefreshNadeoServicesAuthenticationToken : CNetNadeoServicesRequestTask
    (0x12125000, 0x120B0000), // CNetNadeoServicesTask_AddClientLog : CNetNadeoServicesRequestTask
    (0x12126000, 0x120B0000), // CNetNadeoServicesTask_GetAccountXp : CNetNadeoServicesRequestTask
    (0x12127000, 0x1203D000), // CWebServicesTaskResult_NSAccountXp : CWebServicesTaskResult
    (0x12128000, 0x120B0000), // CNetNadeoServicesTask_AddToWaitingQueue : CNetNadeoServicesRequestTask
    (0x12129000, 0x1203D000), // CWebServicesTaskResult_NSWaitingInfo : CWebServicesTaskResult
    (0x1212A000, 0x1203C000), // CWebServicesTask_CheckLoginExists : CWebServicesTaskSequence
    (0x1212B000, 0x120B0000), // CNetNadeoServicesTask_CheckLoginExists : CNetNadeoServicesRequestTask
    (0x1212C000, 0x1203C000), // CWebServicesTask_CreateAccount : CWebServicesTaskSequence
    (0x1212D000, 0x120B0000), // CNetNadeoServicesTask_CreateUserAccount : CNetNadeoServicesRequestTask
    (0x1212E000, 0x120B0000), // CNetNadeoServicesTask_AddAccountPasswordReset : CNetNadeoServicesRequestTask
    (0x1212F000, 0x120B0000), // CNetNadeoServicesTask_SetAccountPassword : CNetNadeoServicesRequestTask
    (0x12130000, 0x1203C000), // CWebServicesTask_SendResetPasswordRequest : CWebServicesTaskSequence
    (0x12131000, 0x120B0000), // CNetNadeoServicesTask_SetAccountPresence : CNetNadeoServicesRequestTask
    (0x12132000, 0x1203D000), // CWebServicesTaskResult_NSAccountPresence : CWebServicesTaskResult
    (0x12133000, 0x1203C000), // CWebServicesTask_Disconnect : CWebServicesTaskSequence
    (0x12134000, 0x120B0000), // CNetNadeoServicesTask_DeleteAccountPresence : CNetNadeoServicesRequestTask
    (0x12135000, 0x1203C000), // CWebServicesTask_DisconnectFromNadeoServices : CWebServicesTaskSequence
    (0x12136000, 0x120B0000), // CNetNadeoServicesTask_AddAccountSkinFavorite : CNetNadeoServicesRequestTask
    (0x12137000, 0x120B0000), // CNetNadeoServicesTask_RemoveAccountSkinFavorite : CNetNadeoServicesRequestTask
    (0x12138000, 0x120B0000), // CNetNadeoServicesTask_GetSkin : CNetNadeoServicesRequestTask
    (0x12139000, 0x1203C000), // CWebServicesTask_CheckWaitingQueue : CWebServicesTaskSequence
    (0x1213A000, 0x120B0000), // CNetNadeoServicesTask_UnsetAccountSkin : CNetNadeoServicesRequestTask
    (0x1213C000, 0x1203D000), // CWebServicesTaskResult_NSAccountTrophyLastYearSummary : CWebServicesTaskResult
    (0x1213D000, 0x120B0000), // CNetNadeoServicesTask_GetAccountTrophyLastYearSummary : CNetNadeoServicesRequestTask
    (0x1213E000, 0x120B0000), // CNetNadeoServicesTask_GetTrophySettings : CNetNadeoServicesRequestTask
    (0x1213F000, 0x1203D000), // CWebServicesTaskResult_NSTrophySettings : CWebServicesTaskResult
    (0x12140000, 0x120B0000), // CNetNadeoServicesTask_GetUserAccount : CNetNadeoServicesRequestTask
    (0x12141000, 0x1203D000), // CWebServicesTaskResult_NSUserAccount : CWebServicesTaskResult
    (0x12143000, 0x1203C000), // CWebServicesTask_SetUserZone : CWebServicesTaskSequence
    (0x12145000, 0x120B0000), // CNetNadeoServicesTask_GetAccountDisplayNameList : CNetNadeoServicesRequestTask
    (0x12146000, 0x120B0000), // CNetNadeoServicesTask_SetTrophyLiveTimeAttackAchievementResult : CNetNadeoServicesRequestTask
    (0x12147000, 0x1203D000), // CWebServicesTaskResult_NSAccountTrophyGainList : CWebServicesTaskResult
    (0x12148000, 0x120B0000), // CNetNadeoServicesTask_UpdateUserAccount : CNetNadeoServicesRequestTask
    (0x12149000, 0x1203C000), // CWebServicesTask_ResetPassword : CWebServicesTaskSequence
    (0x1214A000, 0x120B0000), // CNetNadeoServicesTask_ResetAccountPassword : CNetNadeoServicesRequestTask
    (0x1214B000, 0x120B0000), // CNetNadeoServicesTask_SetTrophyCompetitionMatchAchievementResult : CNetNadeoServicesRequestTask
    (0x1214C000, 0x1203D000), // CWebServicesTaskResult_NSAccountTrophyGainHistory : CWebServicesTaskResult
    (0x1214D000, 0x120B0000), // CNetNadeoServicesTask_GetAccountTrophyGainHistory : CNetNadeoServicesRequestTask
    (0x1214F000, 0x1203D000), // CWebServicesTaskResult_NSItemCollection : CWebServicesTaskResult
    (0x12150000, 0x120B0000), // CNetNadeoServicesTask_SetItemCollection : CNetNadeoServicesRequestTask
    (0x12151000, 0x1203D000), // CWebServicesTaskResult_NSItemCollectionList : CWebServicesTaskResult
    (0x12152000, 0x120B0000), // CNetNadeoServicesTask_GetItemCollection : CNetNadeoServicesRequestTask
    (0x12153000, 0x120B0000), // CNetNadeoServicesTask_GetItemCollectionList : CNetNadeoServicesRequestTask
    (0x12154000, 0x120B0000), // CNetNadeoServicesTask_GetAccountItemCollectionList : CNetNadeoServicesRequestTask
    (0x12155000, 0x1203D000), // CWebServicesTaskResult_NSItemCollectionVersion : CWebServicesTaskResult
    (0x12156000, 0x1203D000), // CWebServicesTaskResult_NSItemCollectionVersionList : CWebServicesTaskResult
    (0x12157000, 0x120B0000), // CNetNadeoServicesTask_CreateItemCollectionVersion : CNetNadeoServicesRequestTask
    (0x12158000, 0x120B0000), // CNetNadeoServicesTask_GetItemCollectionVersion : CNetNadeoServicesRequestTask
    (0x12159000, 0x120B0000), // CNetNadeoServicesTask_GetItemCollectionVersionList : CNetNadeoServicesRequestTask
    (0x1215A000, 0x1203D000), // CWebServicesTaskResult_NSAccountItemCollectionFavorite : CWebServicesTaskResult
    (0x1215B000, 0x1203D000), // CWebServicesTaskResult_NSAccountItemCollectionFavoriteList : CWebServicesTaskResult
    (0x1215C000, 0x120B0000), // CNetNadeoServicesTask_AddAccountItemCollectionFavorite : CNetNadeoServicesRequestTask
    (0x1215D000, 0x120B0000), // CNetNadeoServicesTask_GetAccountItemCollectionFavoriteList : CNetNadeoServicesRequestTask
    (0x1215E000, 0x120B0000), // CNetNadeoServicesTask_RemoveAccountItemCollectionFavorite : CNetNadeoServicesRequestTask
    (0x1215F000, 0x1203D000), // CWebServicesTaskResult_NSUpload : CWebServicesTaskResult
    (0x12160000, 0x120B0000), // CNetNadeoServicesTask_CreateUpload : CNetNadeoServicesRequestTask
    (0x12161000, 0x120B0000), // CNetNadeoServicesTask_GetUpload : CNetNadeoServicesRequestTask
    (0x12162000, 0x120B0000), // CNetNadeoServicesTask_UploadPart : CNetNadeoServicesRequestTask
    (0x12163000, 0x1203C000), // CNetNadeoServicesTask_Upload : CWebServicesTaskSequence
    (0x12164000, 0x120B0000), // CNetNadeoServicesTask_SetItemCollectionActivityId : CNetNadeoServicesRequestTask
    (0x12166000, 0x120B0000), // CNetNadeoServicesTask_GetClub : CNetNadeoServicesRequestTask
    (0x12167000, 0x1203D000), // CWebServicesTaskResult_NSClubList : CWebServicesTaskResult
    (0x12168000, 0x120B0000), // CNetNadeoServicesTask_GetClubList : CNetNadeoServicesRequestTask
    (0x12169000, 0x120B0000), // CNetNadeoServicesTask_GetItemCollectionListByUniqueIdentifierList : CNetNadeoServicesRequestTask
    (0x1216A000, 0x1203C000), // CWebServicesTask_UploadSessionReplay : CWebServicesTaskSequence
    (0x1216B000, 0x1203D000), // CWebServicesTaskResult_WSFriendInfoList : CWebServicesTaskResult
    (0x1216C000, 0x1203C000), // CWebServicesTask_RetrieveFriendList : CWebServicesTaskSequence
    (0x1216D000, 0x1203C000), // CNetUplayPCTask_GetFriendList : CWebServicesTaskSequence
    (0x1216E000, 0x1203D000), // CWebServicesTaskResult_UPCFriendList : CWebServicesTaskResult
    (0x1216F000, 0x12040000), // CNetUbiServicesTask_GetFriendList : CNetUbiServicesTask
    (0x12171000, 0x1203D000), // CWebServicesTaskResult_NSSquad : CWebServicesTaskResult
    (0x12172000, 0x120B0000), // CNetNadeoServicesTask_CreateSquad : CNetNadeoServicesRequestTask
    (0x12173000, 0x120B0000), // CNetNadeoServicesTask_GetSquad : CNetNadeoServicesRequestTask
    (0x12174000, 0x120B0000), // CNetNadeoServicesTask_AddSquadInvitation : CNetNadeoServicesRequestTask
    (0x12175000, 0x120B0000), // CNetNadeoServicesTask_RemoveSquadInvitation : CNetNadeoServicesRequestTask
    (0x12176000, 0x120B0000), // CNetNadeoServicesTask_AcceptSquadInvitation : CNetNadeoServicesRequestTask
    (0x12177000, 0x120B0000), // CNetNadeoServicesTask_DeclineSquadInvitation : CNetNadeoServicesRequestTask
    (0x12178000, 0x120B0000), // CNetNadeoServicesTask_RemoveSquadMember : CNetNadeoServicesRequestTask
    (0x12179000, 0x120B0000), // CNetNadeoServicesTask_SetSquadLeader : CNetNadeoServicesRequestTask
    (0x1217A000, 0x120B0000), // CNetNadeoServicesTask_LeaveSquad : CNetNadeoServicesRequestTask
    (0x1217B000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSquad : CNetNadeoServicesRequestTask
    (0x1217C000, 0x1203D000), // CWebServicesTaskResult_NSAccountZoneList : CWebServicesTaskResult
    (0x1217D000, 0x120B0000), // CNetNadeoServicesTask_GetAccountZoneList : CNetNadeoServicesRequestTask
    (0x1217E000, 0x1203D000), // CWebServicesTaskResult_NSAccountSubscriptionList : CWebServicesTaskResult
    (0x1217F000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSubscriptionList : CNetNadeoServicesRequestTask
    (0x12180000, 0x120B0000), // CNetNadeoServicesTask_GetAccountSkinListByAccountList : CNetNadeoServicesRequestTask
    (0x12181000, 0x1203D000), // CWebServicesTaskResult_NSAccountClubTag : CWebServicesTaskResult
    (0x12182000, 0x120B0000), // CNetNadeoServicesTask_GetAccountClubTag : CNetNadeoServicesRequestTask
    (0x12183000, 0x120B0000), // CNetNadeoServicesTask_SetAccountClubTag : CNetNadeoServicesRequestTask
    (0x12184000, 0x1203C000), // CWebServicesTask_GetUserClubTag : CWebServicesTaskSequence
    (0x12185000, 0x1203C000), // CWebServicesTask_SetUserClubTag : CWebServicesTaskSequence
    (0x12186000, 0x1203D000), // CWebServicesTaskResult_NSAccountClubTagList : CWebServicesTaskResult
    (0x12187000, 0x120B0000), // CNetNadeoServicesTask_GetAccountClubTagList : CNetNadeoServicesRequestTask
    (0x12188000, 0x1203C000), // CWebServicesTask_GetUserClubTagList : CWebServicesTaskSequence
    (0x12189000, 0x1203D000), // CWebServicesTaskResult_NSDriverBotGroupList : CWebServicesTaskResult
    (0x1218A000, 0x120B0000), // CNetNadeoServicesTask_GetDriverBotGroupList : CNetNadeoServicesRequestTask
    (0x1218B000, 0x120B0000), // CNetNadeoServicesTask_AddDriverBotGroupList : CNetNadeoServicesRequestTask
    (0x1218C000, 0x1203C000), // CWebServicesTask_GetSeasonPlayableList : CWebServicesTaskSequence
    (0x1218D000, 0x120B0000), // CNetNadeoServicesTask_GetAccountPlayableSeasonList : CNetNadeoServicesRequestTask
    (0x1218E000, 0x12040000), // CNetUbiServicesTask_Profile_RetrieveUplayProfileInfoList : CNetUbiServicesTask
    (0x12191000, 0x12040000), // CNetUbiServicesTask_Profile_RetrieveProfileInfoListFromPlatformTypeAndUserId : CNetUbiServicesTask
    (0x12195000, 0x1203C000), // CWebServicesTask_GetUserZone : CWebServicesTaskSequence
    (0x12196000, 0x1203C000), // CWebServicesTask_GetUserZoneList : CWebServicesTaskSequence
    (0x12197000, 0x1203C000), // CWebServicesTask_GetZoneList : CWebServicesTaskSequence
    (0x12198000, 0x1203D000), // CWebServicesTaskResult_WSZoneList : CWebServicesTaskResult
    (0x12199000, 0x1203D000), // CWebServicesTaskResult_WSZonePtrList : CWebServicesTaskResult
    (0x1219A000, 0x1203C000), // CWebServicesTask_Permission_CheckCrossPlay : CWebServicesTaskSequence
    (0x1219C000, 0x1203C000), // CWebServicesTask_Permission_CheckPlayMultiplayerAsync : CWebServicesTaskSequence
    (0x1219D000, 0x1203C000), // CWebServicesTask_Permission_CheckPlayMultiplayerMode : CWebServicesTaskSequence
    (0x1219E000, 0x1203C000), // CWebServicesTask_Permission_CheckPlayMultiplayerSession : CWebServicesTaskSequence
    (0x1219F000, 0x1203C000), // CWebServicesTask_Permission_CheckPrivilegeForAllUsers : CWebServicesTaskSequence
    (0x121A0000, 0x1203C000), // CWebServicesTask_Permission_CheckTargetedUseUserCreatedContent : CWebServicesTaskSequence
    (0x121A1000, 0x1203C000), // CWebServicesTask_Permission_CheckTargetedUseUserCreatedContentForAllUsers : CWebServicesTaskSequence
    (0x121A2000, 0x1203C000), // CWebServicesTask_Permission_CheckTargetedViewUserGameHistory : CWebServicesTaskSequence
    (0x121A4000, 0x1203C000), // CWebServicesTask_Permission_CheckViewOnlinePresence : CWebServicesTaskSequence
    (0x121A5000, 0x1203C000), // CWebServicesTask_Permission_CheckUseUserCreatedContent : CWebServicesTaskSequence
    (0x121A7000, 0x1203D000), // CWebServicesTaskResult_CheckTargetedPrivilegeResult : CWebServicesTaskResult
    (0x121A8000, 0x1203D000), // CWebServicesTaskResult_WSNewsList : CWebServicesTaskResult
    (0x121A9000, 0x1203C000), // CWebServicesTask_GetUserNewsList : CWebServicesTaskSequence
    (0x121AA000, 0x1203C000), // CNetUplayPCTask_ShowBrowserUrl : CWebServicesTaskSequence
    (0x121AB000, 0x1203C000), // CWebServicesTask_GetMapList : CWebServicesTaskSequence
    (0x121AC000, 0x1203D000), // CWebServicesTaskResult_WSMapPtrList : CWebServicesTaskResult
    (0x121AD000, 0x1203C000), // CWebServicesTask_StartMapRecordAttempt : CWebServicesTaskSequence
    (0x121AE000, 0x1203D000), // CWebServicesTaskResult_NSMapRecordAttempt : CWebServicesTaskResult
    (0x121AF000, 0x120B0000), // CNetNadeoServicesTask_CreateMapRecordSecureAttempt : CNetNadeoServicesRequestTask
    (0x121B0000, 0x1203C000), // CWebServicesTask_StopMapRecordAttempt : CWebServicesTaskSequence
    (0x121B1000, 0x120B0000), // CNetNadeoServicesTask_PatchMapRecordSecureAttempt : CNetNadeoServicesRequestTask
    (0x121B2000, 0x1203D000), // CWebServicesTaskResult_NSMapRecordSecureAttempt : CWebServicesTaskResult
    (0x121B3000, 0x12040000), // CNetUbiServicesTask_GetStatList : CNetUbiServicesTask
    (0x121B4000, 0x1203D000), // CWebServicesTaskResult_UbiServicesStatList : CWebServicesTaskResult
    (0x121B5000, 0x1203C000), // CWebServicesTask_GetFirstPartyAchievementList : CWebServicesTaskSequence
    (0x121B6000, 0x1203C000), // CWebServicesTask_UpdateFirstPartyAchievementCompletion : CWebServicesTaskSequence
    (0x121B8000, 0x1203D000), // CWebServicesTaskResult_WSPrestigeList : CWebServicesTaskResult
    (0x121B9000, 0x1203D000), // CWebServicesTaskResult_WSUserPrestigeList : CWebServicesTaskResult
    (0x121BA000, 0x1203D000), // CWebServicesTaskResult_WSUserPrestige : CWebServicesTaskResult
    (0x121C0000, 0x1203D000), // CWebServicesTaskResult_UPCAchievementCompletionList : CWebServicesTaskResult
    (0x121C1000, 0x1203C000), // CNetUplayPCTask_Achievement_GetCompletionList : CWebServicesTaskSequence
    (0x121C8000, 0x1203C000), // CWebServicesTask_UserProfile_GetAvatarUrl : CWebServicesTaskSequence
    (0x121CB000, 0x1203D000), // CWebServicesTaskResult_WSBlockedUserList : CWebServicesTaskResult
    (0x121CC000, 0x1203D000), // CWebServicesTaskResult_NSAccountPrestigeList : CWebServicesTaskResult
    (0x121CD000, 0x1203D000), // CWebServicesTaskResult_NSCurrentAccountPrestige : CWebServicesTaskResult
    (0x121CE000, 0x1203D000), // CWebServicesTaskResult_NSPrestigeList : CWebServicesTaskResult
    (0x121CF000, 0x120B0000), // CNetNadeoServicesTask_GetAccountPrestigeList : CNetNadeoServicesRequestTask
    (0x121D0000, 0x120B0000), // CNetNadeoServicesTask_GetCurrentAccountPrestige : CNetNadeoServicesRequestTask
    (0x121D1000, 0x120B0000), // CNetNadeoServicesTask_GetPrestigeList : CNetNadeoServicesRequestTask
    (0x121D2000, 0x120B0000), // CNetNadeoServicesTask_GetPrestigeListFromModeYearType : CNetNadeoServicesRequestTask
    (0x121D3000, 0x120B0000), // CNetNadeoServicesTask_SetAccountPrestigeCurrent : CNetNadeoServicesRequestTask
    (0x121D4000, 0x1203C000), // CWebServicesTask_GetUserPrestigeList : CWebServicesTaskSequence
    (0x121D5000, 0x1203C000), // CWebServicesTask_SetUserPrestigeSelected : CWebServicesTaskSequence
    (0x121D6000, 0x120B0000), // CNetNadeoServicesTask_UnsetAccountPrestigeCurrent : CNetNadeoServicesRequestTask
    (0x121D7000, 0x1203C000), // CWebServicesTask_GetPrestigeList : CWebServicesTaskSequence
    (0x121D8000, 0x1203D000), // CWebServicesTaskResult_WSPrestige : CWebServicesTaskResult
    (0x121D9000, 0x1203C000), // CWebServicesTask_GetPrestige : CWebServicesTaskSequence
    (0x121DA000, 0x12040000), // CNetUbiServicesTask_PlayerPreferences_SetStandardPreferences : CNetUbiServicesTask
    (0x121DB000, 0x12040000), // CNetUbiServicesTask_PlayerPreferences_GetStandardPreferences : CNetUbiServicesTask
    (0x121DC000, 0x1203D000), // CWebServicesTaskResult_UbiServicesPlayerPreferencesStandard : CWebServicesTaskResult
    (0x121DD000, 0x1203D000), // CWebServicesTaskResult_NSAccountPrestige : CWebServicesTaskResult
    (0x121DE000, 0x120B0000), // CNetNadeoServicesTask_GetAccountPrestigeCurrent : CNetNadeoServicesRequestTask
    (0x121DF000, 0x1203C000), // CWebServicesTask_GetUserPrestigeSelectedForUser : CWebServicesTaskSequence
    (0x121E0000, 0x120B0000), // CNetNadeoServicesTask_GetCurrentAccountPrestigeList : CNetNadeoServicesRequestTask
    (0x121E1000, 0x1203C000), // CWebServicesTask_GetUserPrestigeSelectedForUserList : CWebServicesTaskSequence
    (0x121E2000, 0x1203D000), // CWebServicesTaskResult_UbiServicesPartyInfo : CWebServicesTaskResult
    (0x121E3000, 0x1203D000), // CWebServicesTaskResult_WSPartyInfo : CWebServicesTaskResult
    (0x121E4000, 0x1203C000), // CWebServicesTask_Party_RetrievePartyInfo : CWebServicesTaskSequence
    (0x121E5000, 0x12040000), // CNetUbiServicesTask_Party_GetPartyInfo : CNetUbiServicesTask
    (0x121E6000, 0x12040000), // CNetUbiServicesTask_Party_CreateParty : CNetUbiServicesTask
    (0x121E7000, 0x1203C000), // CWebServicesTask_Party_Create : CWebServicesTaskSequence
    (0x121E8000, 0x1203C000), // CWebServicesTask_Party_Leave : CWebServicesTaskSequence
    (0x121E9000, 0x1203C000), // CWebServicesTask_Party_RenewPartyExpiration : CWebServicesTaskSequence
    (0x121EA000, 0x12040000), // CNetUbiServicesTask_Party_AutoRemovePartyMemberOnDisconnect : CNetUbiServicesTask
    (0x121EB000, 0x1203D000), // CWebServicesTaskResult_UbiServicesPartyInvitationList : CWebServicesTaskResult
    (0x121EC000, 0x1203D000), // CWebServicesTaskResult_UbiServicesPartyJoinRequestList : CWebServicesTaskResult
    (0x121ED000, 0x1203D000), // CWebServicesTaskResult_UbiServicesPartyMemberList : CWebServicesTaskResult
    (0x121EE000, 0x12040000), // CNetUbiServicesTask_Party_GetPartyInvitationList : CNetUbiServicesTask
    (0x121EF000, 0x12040000), // CNetUbiServicesTask_Party_GetPartyJoinRequestList : CNetUbiServicesTask
    (0x121F0000, 0x12040000), // CNetUbiServicesTask_Party_GetPartyMemberList : CNetUbiServicesTask
    (0x121F1000, 0x12040000), // CNetUbiServicesTask_Party_LeaveParty : CNetUbiServicesTask
    (0x121F2000, 0x12040000), // CNetUbiServicesTask_Party_RenewExpiration : CNetUbiServicesTask
    (0x121F3000, 0x1203C000), // CWebServicesTask_Party_RetrievePartyInvitationList : CWebServicesTaskSequence
    (0x121F4000, 0x1203C000), // CWebServicesTask_Party_RetrievePartyJoinRequestList : CWebServicesTaskSequence
    (0x121F5000, 0x1203C000), // CWebServicesTask_Party_RetrievePartyMemberList : CWebServicesTaskSequence
    (0x121F6000, 0x1203D000), // CWebServicesTaskResult_WSPartyInvitationList : CWebServicesTaskResult
    (0x121F7000, 0x1203D000), // CWebServicesTaskResult_WSPartyJoinRequestList : CWebServicesTaskResult
    (0x121F8000, 0x1203D000), // CWebServicesTaskResult_WSPartyMemberList : CWebServicesTaskResult
    (0x121F9000, 0x1203C000), // CWebServicesTask_Party_RequestAutoRemovePartyMemberOnDisconnect : CWebServicesTaskSequence
    (0x121FA000, 0x1203D000), // CWebServicesTaskResult_WSPartyCompleteInfo : CWebServicesTaskResult
    (0x121FB000, 0x1203C000), // CWebServicesTask_Party_RetrievePartyCompleteInfo : CWebServicesTaskSequence
    (0x121FC000, 0x120B0000), // CNetNadeoServicesTask_GetVisualNotificationUrlInfo : CNetNadeoServicesRequestTask
    (0x121FD000, 0x1203D000), // CWebServicesTaskResult_UbiServicesVisualNotificationUrlInfo : CWebServicesTaskResult
    (0x121FF000, 0x1203C000), // CWebServicesTask_UbisoftConnect_Show : CWebServicesTaskSequence
    (0x12200000, 0x1203C000), // CWebServicesTask_UserProfile_ShowUbisoftConnectProfile : CWebServicesTaskSequence
    (0x12205000, 0x12040000), // CNetUbiServicesTask_Party_GetFirstPartySessionInfo : CNetUbiServicesTask
    (0x12207000, 0x12040000), // CNetUbiServicesTask_Party_UpdateParty : CNetUbiServicesTask
    (0x12208000, 0x1203C000), // CWebServicesTask_Party_Update : CWebServicesTaskSequence
    (0x12214000, 0x12040000), // CNetUbiServicesTask_Party_UpdateLockState : CNetUbiServicesTask
    (0x12215000, 0x1203C000), // CWebServicesTask_Party_SetLocked : CWebServicesTaskSequence
    (0x12216000, 0x12040000), // CNetUbiServicesTask_Profile_RetrieveProfileInfoListFromPlatform : CNetUbiServicesTask
    (0x12217000, 0x1203C000), // CWebServicesTask_GetWebServicesUserIdFromWebIdentity : CWebServicesTaskSequence
    (0x12220000, 0x120B0000), // CNetNadeoServicesTask_GetMapRecordList : CNetNadeoServicesRequestTask
    (0x12221000, 0x120B0000), // CNetNadeoServicesTask_AddClientDebugInfo : CNetNadeoServicesRequestTask
    (0x12223000, 0x1203C000), // CWebServicesTask_Preference_RetrieveUserPreference : CWebServicesTaskSequence
    (0x12224000, 0x1203C000), // CWebServicesTask_Event_AddMapSession : CWebServicesTaskSequence
    (0x12225000, 0x120B0000), // CNetNadeoServicesTask_AddTelemetryMapSession : CNetNadeoServicesRequestTask
    (0x12226000, 0x120B0000), // CNetNadeoServicesTask_GetMapVote : CNetNadeoServicesRequestTask
    (0x12227000, 0x120B0000), // CNetNadeoServicesTask_VoteMap : CNetNadeoServicesRequestTask
    (0x12228000, 0x120B0000), // CNetNadeoServicesTask_Activity_CreateMatch : CNetNadeoServicesRequestTask
    (0x12229000, 0x120B0000), // CNetNadeoServicesTask_Activity_ReportMatchResult : CNetNadeoServicesRequestTask
    (0x1222A000, 0x1203D000), // CWebServicesTaskResult_WSMapRecordList : CWebServicesTaskResult
    (0x1222B000, 0x1203C000), // CWebServicesTask_GetMapRecordListByMapRecordContextAndUserList : CWebServicesTaskSequence
    (0x1222D000, 0x12040000), // CNetUbiServicesTask_Party_ChangeFirstPartySessionId : CNetUbiServicesTask
    (0x12231000, 0x120B0000), // CNetNadeoServicesTask_Activity_UpdateMatch : CNetNadeoServicesRequestTask
    (0x12232000, 0x1203D000), // CWebServicesTaskResult_NSDriverBotGroupAddLimit : CWebServicesTaskResult
    (0x12233000, 0x120B0000), // CNetNadeoServicesTask_AddDriverBotGroupList_GetLimit : CNetNadeoServicesRequestTask
    (0x12234000, 0x1203D000), // CWebServicesTaskResult_UbiServicesBlockList : CWebServicesTaskResult
    (0x12235000, 0x12040000), // CNetUbiServicesTask_Blocklist_Get : CNetUbiServicesTask
    (0x12236000, 0x1203D000), // CWebServicesTaskResult_UbiServicesProfileConsent : CWebServicesTaskResult
    (0x12237000, 0x12040000), // CNetUbiServicesTask_PlayerConsents_GetConsent : CNetUbiServicesTask
    (0x12238000, 0x1203D000), // CWebServicesTaskResult_UbiServicesProfileAcceptanceList : CWebServicesTaskResult
    (0x12239000, 0x12040000), // CNetUbiServicesTask_PlayerConsents_GetAcceptanceList : CNetUbiServicesTask
    (0x1223A000, 0x1203C000), // CWebServicesTask_Permission_GetPlayerInteractionRestriction : CWebServicesTaskSequence
    (0x1223B000, 0x1203C000), // CWebServicesTask_Permission_GetPlayerInteractionStatusList : CWebServicesTaskSequence
    (0x1223D000, 0x1203D000), // CWebServicesTaskResult_Integer : CWebServicesTaskResult
    (0x1223E000, 0x1203C000), // CWebServicesTask_RetrieveUserPrestigeLevelList : CWebServicesTaskSequence
    (0x1223F000, 0x1203C000), // CWebServicesTask_RetrievePrestigeInfoList : CWebServicesTaskSequence
    (0x12240000, 0x120B0000), // CNetNadeoServicesTask_GetAccountMapFavoriteList : CNetNadeoServicesRequestTask
    (0x12241000, 0x1203D000), // CWebServicesTaskResult_NSAccountMapFavoriteList : CWebServicesTaskResult
    (0x12242000, 0x120B0000), // CNetNadeoServicesTask_GetAccountMapFavoriteListByMapUid : CNetNadeoServicesRequestTask
    (0x12243000, 0x120B0000), // CNetNadeoServicesTask_AddAccountMapFavorite : CNetNadeoServicesRequestTask
    (0x12245000, 0x120B0000), // CNetNadeoServicesTask_RemoveAccountMapFavorite : CNetNadeoServicesRequestTask
    (0x12246000, 0x1203C000), // CWebServicesTask_GetPrestigeListByYear : CWebServicesTaskSequence
    (0x12247000, 0x120B0000), // CNetNadeoServicesTask_GetPrestigeListFromYear : CNetNadeoServicesRequestTask
    (0x12248000, 0x1203D000), // CWebServicesTaskResult_AdditionalFileList : CWebServicesTaskResult
    (0x12249000, 0x120B0000), // CNetNadeoServicesTask_GetAccountAdditionalFileList : CNetNadeoServicesRequestTask
    (0x1224A000, 0x120B0000), // CNetNadeoServicesTask_GetAccountMapZen : CNetNadeoServicesRequestTask
    (0x1224B000, 0x120B0000), // CNetNadeoServicesTask_IncrAccountMapZen : CNetNadeoServicesRequestTask
    (0x1224C000, 0x1203D000), // CWebServicesTaskResult_NSAccountMapZen : CWebServicesTaskResult
    (0x1224D000, 0x1203C000), // CWebServicesTask_GetMapZen : CWebServicesTaskSequence
    (0x1224E000, 0x1203C000), // CWebServicesTask_IncrMapZen : CWebServicesTaskSequence
    (0x13001000, 0x01001000), // CInputPort : CMwNod
    (0x13002000, 0x13001000), // CInputPortDx8 : CInputPort
    (0x13003000, 0x13001000), // CInputPortNull : CInputPort
    (0x13004000, 0x01001000), // CInputScriptEvent : CMwNod
    (0x13006000, 0x01001000), // CInputBindingsConfig : CMwNod
    (0x13007000, 0x01001000), // CInputDevice : CMwNod
    (0x13008000, 0x13007000), // CInputDeviceMouse : CInputDevice
    (0x1300A000, 0x13008000), // CInputDeviceDx8Mouse : CInputDeviceMouse
    (0x1300B000, 0x13007000), // CInputDeviceDx8Keyboard : CInputDevice
    (0x1300C000, 0x13007000), // CInputDeviceDx8Pad : CInputDevice
    (0x1300D000, 0x01001000), // CInputReplay : CMwNod
    (0x13011000, 0x01001000), // CInputScriptManager : CMwNod
    (0x13012000, 0x01001000), // CInputScriptPad : CMwNod
    (0x1400F000, 0x01001000), // CXmlScriptParsingManager : CMwNod
    (0x14010000, 0x01001000), // CXmlScriptParsingDocumentXml : CMwNod
    (0x14011000, 0x01001000), // CXmlScriptParsingNodeXml : CMwNod
    (0x14012000, 0x01001000), // CXmlScriptParsingDocumentJson : CMwNod
    (0x14013000, 0x01001000), // CXmlScriptParsingNodeJson : CMwNod
    (0x24001000, 0x03013000), // CTrackMania : CGameManiaPlanet
    (0x2402E000, 0x030FA000), // CTrackManiaMenus : CGameCtnMenusManiaPlanet
    (0x2402F000, 0x03011000), // CTrackManiaNetwork : CGameManiaPlanetNetwork
    (0x24035000, 0x030BB000), // CTrackManiaNetworkServerInfo : CGameCtnNetServerInfo
    (0x24036000, 0x0308A000), // CTrackManiaPlayerInfo : CGamePlayerInfo
    (0x24041000, 0x03037000), // CTrackManiaMatchSettings : CGameFid
    (0x2406E000, 0x07016000), // CTrackManiaControlCheckPointList : CControlFrame
    (0x2408C000, 0x2408F000), // CTrackManiaControlPlayerInfoCard : CTrackManiaControlCard
    (0x2408F000, 0x0309A000), // CTrackManiaControlCard : CGameControlCard
    (0x240C4000, 0x2408F000), // CTrackManiaControlMatchSettingsCard : CTrackManiaControlCard
    (0x240D5000, 0x0312B000), // CGamePlayerProfileChunk_TrackManiaSettings : CGamePlayerProfileChunk
    (0x2D000000, 0x03012000), // CShootMania : CGameManiaTitleCore
    (0x2D003000, 0x030ED000), // CSmArenaInterfaceManialinkScripHandler : -
    (0x2D004000, 0x03100000), // CSmArenaClient : CGamePlaygroundCommon
    (0x2D005000, 0x03111000), // CSmArenaScore : CGamePlaygroundScore
    (0x2D006000, 0x032D2000), // CSmActionInstance : CGameAction
    (0x2D007000, 0x0302F000), // CSmNetForm : CGameNetForm
    (0x2D008000, 0x03002000), // CSmPlayer : CGamePlayer
    (0x2D009000, 0x030E2000), // CSmArenaInterfaceUI : CGamePlaygroundInterface
    (0x2D00A000, 0x01001000), // CSmPlayerDriver : CMwNod
    (0x2D00B000, 0x11005000), // CSmActionInstanceEvent : CScriptBaseConstEvent
    (0x2D00C000, 0x01001000), // CSmArenaRules : CMwNod
    (0x2D00F000, 0x0305B000), // CSmChallengeParameters : CGameCtnChallengeParameters
    (0x2D011000, 0x032E5000), // CSmObject : CGameScriptEntity
    (0x2D014000, 0x030E1000), // CSmAnalyzer : CGameAnalyzer
    (0x2D015000, 0x01001000), // CSmArenaPhysics : CMwNod
    (0x2D017000, 0x01001000), // CSmClient : CMwNod
    (0x2D018000, 0x01001000), // CSmArenaServer : CMwNod
    (0x2D019000, 0x01001000), // CSmArena : CMwNod
    (0x2D01A000, 0x03138000), // CSmArenaRulesMode : CGamePlaygroundScript
    (0x2D01B000, 0x01001000), // CSmServer : CMwNod
    (0x2D01C000, 0x11004000), // CSmArenaRulesEvent : CScriptBaseEvent
    (0x2D01F000, 0x03154000), // CSmEditorPluginMapType : CGameEditorPluginMapMapType
    (0x2D020000, 0x01001000), // CSmArenaResource : CMwNod
    (0x2D029000, 0x031C8000), // CSmModuleScoresTable : CGamePlaygroundModuleClientScoresTable
    (0x2D02A000, 0x01001000), // CSmScriptMapBase : CMwNod
    (0x2D02B000, 0x01001000), // CSmScriptMapGate : CMwNod
    (0x2D02C000, 0x01001000), // CSmScriptMapGauge : CMwNod
    (0x2D02D000, 0x032D5000), // CSmScriptMapLandmark : CGameScriptMapLandmark
    (0x2D02E000, 0x032E6000), // CSmScriptPlayer : CGameScriptPlayer
    (0x2D033000, 0x031C9000), // CSmModuleManager : CGamePlaygroundModuleManagerClient
    (0x2D035000, 0x032CE000), // CSmActionMgr : CGameMgrAction
    (0x2D036000, 0x0302F000), // CSmNetFormBroadcastable : CGameNetForm
    (0x2D037000, 0x03387000), // CSmArenaInterfaceManialinkScriptHandler_ReadOnly : -
    (0x2E001000, 0x01001000), // CGameCtnCollector : CMwNod
    (0x2E002000, 0x2E001000), // CGameItemModel : CGameCtnCollector
    (0x2E005000, 0x01001000), // CGameModulePlaygroundPlayerStateComponentModel : CMwNod
    (0x2E006000, 0x01001000), // CGameObjectPhyModel : CMwNod
    (0x2E007000, 0x01001000), // CGameObjectVisModel : CMwNod
    (0x2E008000, 0x01001000), // CGameActionModel : CMwNod
    (0x2E009000, 0x01001000), // CGameWaypointSpecialProperty : CMwNod
    (0x2E00A000, 0x01001000), // CGameActionFxResources : CMwNod
    (0x2E00B000, 0x01001000), // CGameGateModel : CMwNod
    (0x2E00C000, 0x01001000), // CGameTeleporterModel : CMwNod
    (0x2E00D000, 0x01001000), // CGameArmorModel : CMwNod
    (0x2E00F000, 0x01001000), // CGameCaptureZoneModel : CMwNod
    (0x2E010000, 0x01001000), // CGameTurbineModel : CMwNod
    (0x2E011000, 0x01001000), // CGameGhostTMData : CMwNod
    (0x2E012000, 0x2E015000), // CGameModulePlaygroundModel : CGameModuleModelCommon
    (0x2E013000, 0x01001000), // CGameModuleNodForPropertyList : CMwNod
    (0x2E014000, 0x2E015000), // CGameModuleMenuModel : CGameModuleModelCommon
    (0x2E015000, 0x01001000), // CGameModuleModelCommon : CMwNod
    (0x2E016000, 0x2E012000), // CGameModulePlaygroundInventoryModel : CGameModulePlaygroundModel
    (0x2E017000, 0x2E012000), // CGameModulePlaygroundScoresTableModel : CGameModulePlaygroundModel
    (0x2E018000, 0x2E012000), // CGameModulePlaygroundStoreModel : CGameModulePlaygroundModel
    (0x2E019000, 0x2E015000), // CGameModulePlaygroundHudModel : CGameModuleModelCommon
    (0x2E01A000, 0x2E015000), // CGameModuleMenuPageModel : CGameModuleModelCommon
    (0x2E01B000, 0x01001000), // CGameEditorModel : CMwNod
    (0x2E01C000, 0x01001000), // CGameVehicleModel : CMwNod
    (0x2E01D000, 0x01001000), // CGameObjectModel : CMwNod
    (0x2E01E000, 0x2E012000), // CGameModulePlaygroundChronoModel : CGameModulePlaygroundModel
    (0x2E01F000, 0x2E012000), // CGameModulePlaygroundSpeedMeterModel : CGameModulePlaygroundModel
    (0x2E020000, 0x01001000), // CGameItemPlacementParam : CMwNod
    (0x2E021000, 0x2E012000), // CGameModulePlaygroundPlayerStateModel : CGameModulePlaygroundModel
    (0x2E022000, 0x2E012000), // CGameModulePlaygroundTeamStateModel : CGameModulePlaygroundModel
    (0x2E023000, 0x2E005000), // CGameModulePlaygroundPlayerStateGaugeModel : CGameModulePlaygroundPlayerStateComponentModel
    (0x2E024000, 0x01001000), // CGamePixelArtModel : CMwNod
    (0x2E025000, 0x01001000), // CGameBlockItem : CMwNod
    (0x2E026000, 0x01001000), // CGameCommonItemEntityModelEdition : CMwNod
    (0x2E027000, 0x01001000), // CGameCommonItemEntityModel : CMwNod
    (0x2E028000, 0x01001000), // CGameCharacterModel : CMwNod
    (0x2E029000, 0x01001000), // CGameManiaAppTextSet : CMwNod
    (0x2E02A000, 0x01001000), // CGameObjectPhyCompoundModel : CMwNod
    (0x2E02B000, 0x01001000), // CGameModulePlaygroundHudModelModule : CMwNod
    (0x2E02C000, 0x2E005000), // CGameModulePlaygroundPlayerStateListModel : CGameModulePlaygroundPlayerStateComponentModel
    (0x2E033000, 0x2E015000), // CGameModuleEditorModel : CGameModuleModelCommon
    (0x2E034000, 0x2E033000), // CGameModuleEditorGraphEditionModel : CGameModuleEditorModel
    (0x2F021000, 0x2F01B000), // CPlugAnimNodeAim : -
    (0x2F029000, 0x2F028000), // GmSurfPrimitive : -
    (0x2F02A000, 0x2F029000), // GmSurfSphere : -
    (0x2F02B000, 0x2F029000), // GmSurfSphereLocated : -
    (0x2F02C000, 0x2F029000), // GmSurfEllipsoid : -
    (0x2F02E000, 0x2F029000), // GmSurfMultiSphere : -
    (0x2F02F000, 0x2F029000), // GmSurfVCylinder : -
    (0x2F030000, 0x2F029000), // GmSurfCylinder : -
    (0x2F031000, 0x2F029000), // GmSurfBox : -
    (0x2F035000, 0x2F029000), // GmSurfConvexPolyhedron : -
    (0x2F036000, 0x2F029000), // GmSurfCapsule : -
    (0x2F037000, 0x2F029000), // GmSurfPlane : -
    (0x2F039000, 0x2F028000), // GmSurfMesh : -
    (0x2F03A000, 0x2F028000), // GmSurfCompound : -
    (0x2F03B000, 0x2F028000), // GmSurfCompoundInstance : -
    (0x2F03D000, 0x2F028000), // GmSurfCircle : -
    (0x2F042000, 0x2F01B000), // CPlugAnimNodeJump : -
    (0x2F043000, 0x2F01B000), // CPlugAnimNodeSequence : -
    (0x2F044000, 0x2F01B000), // CPlugAnimNodeProceduralAttractor : -
    (0x2F047000, 0x2F01B000), // CPlugAnimNodeClip : -
    (0x2F048000, 0x2F01B000), // CPlugAnimNodeLocoGroup : -
    (0x2F04F000, 0x2F029000), // GmSurfSphericalShell : -
    (0x2F058000, 0x01001000), // CGameMenuScene : CMwNod
    (0x2F05A000, 0x2F08E000), // CPlugAnimGraphNode_JointInertia : -
    (0x2F061000, 0x2F08E000), // CPlugAnimGraphNode_AirTrajectoryPrediction : -
    (0x2F06B000, 0x2F01B000), // CPlugAnimNodeBlend2d : -
    (0x2F06C000, 0x01001000), // CPlugVegetMaterialVariation : CMwNod
    (0x2F06D000, 0x01001000), // CPlugVegetSubSurfaceParams : CMwNod
    (0x2F081000, 0x01001000), // CGameManialinkNavigationScriptHandler : CMwNod
    (0x2F083000, 0x01001000), // CScriptEvent : CMwNod
    (0x2F084000, 0x2F083000), // CEventMenuNavigation : -
    (0x2F085000, 0x2F084000), // CEventMenuNavigationOnAction : -
    (0x2F086000, 0x01001000), // CPlugVegetTreeModel : CMwNod
    (0x2F08A000, 0x0902B000), // CPlugGrassMatterArray : CPlug
    (0x2F08D000, 0x0902B000), // NPlugPainterLayer::SList : CPlug
    (0x2F08E000, 0x01001000), // CPlugGraphNode : CMwNod
    (0x2F08F000, 0x2F08E000), // CPlugAnimGraphNode_JointRotateFrom : -
    (0x2F090000, 0x2F08E000), // CPlugAnimGraphNode_JointRotate : -
    (0x2F091000, 0x2F08E000), // CPlugAnimGraphNode_JointAlignTo : -
    (0x2F092000, 0x2F08E000), // CPlugAnimGraphNode_JointTranslate : -
    (0x2F093000, 0x2F08E000), // CPlugAnimGraphNode_JointIK2 : -
    (0x2F094000, 0x2F08E000), // CPlugAnimGraphNode_JointKeepRefGlobalRot : -
    (0x2F095000, 0x2F08E000), // CPlugAnimGraphNode_JointTranslateDistConstraint : -
    (0x2F096000, 0x2F08E000), // CPlugAnimGraphNode_LocalToGlobal : -
    (0x2F097000, 0x2F08E000), // CPlugAnimGraphNode_Blend : -
    (0x2F098000, 0x2F08E000), // CPlugAnimGraphNode_ClipPlay : -
    (0x2F099000, 0x2F08E000), // CPlugAnimGraphNode_Blend2d : -
    (0x2F09B000, 0x2F0BE000), // CPlugAnimGraphNode_Graph : -
    (0x2F09C000, 0x2F0BF000), // CPlugAnimGraphNode_StateMachine : -
    (0x2F09D000, 0x2F08E000), // CPlugAnimGraphNode_LodSwitch : -
    (0x2F09E000, 0x2F0C0000), // CPlugAnimGraphNode_Group : -
    (0x2F09F000, 0x2F08E000), // CPlugAnimGraphNode_RefLocalPose : -
    (0x2F0A0000, 0x2F08E000), // CPlugAnimGraphNode_GraphOutput : -
    (0x2F0A2000, 0x01001000), // CPlugVFXNode : CMwNod
    (0x2F0A3000, 0x2F0A2000), // CPlugVFXNode_Emit : -
    (0x2F0A4000, 0x2F0A2000), // CPlugVFXNode_EmitterModel : -
    (0x2F0A5000, 0x2F0A2000), // CPlugVFXNode_Graph : -
    (0x2F0A6000, 0x2F0A2000), // CPlugVFXNode_SubEmitterModel : -
    (0x2F0A7000, 0x2F08E000), // CPlugAnimGraphNode_ExtractMotion : -
    (0x2F0A8000, 0x2F08E000), // CPlugAnimGraphNode_RefGlobalPose : -
    (0x2F0AA000, 0x2F08E000), // CPlugAnimGraphNode_SetVar : -
    (0x2F0AB000, 0x2F08E000), // CPlugAnimGraphNode_ClipGroupPlay : -
    (0x2F0AC000, 0x2F0A2000), // CPlugVFXNode_VortexEmitterModel : -
    (0x2F0AD000, 0x2F0A2000), // CPlugVFXNode_EmissionGroup : -
    (0x2F0AE000, 0x2F08E000), // CPlugAnimGraphNode_SetSkel : -
    (0x2F0B1000, 0x2F08E000), // CPlugAnimGraphNode_LayeredBlend : -
    (0x2F0B2000, 0x2F08E000), // CPlugAnimGraphNode_GlobalToLocal : -
    (0x2F0B3000, 0x2F08E000), // CPlugAnimGraphNode_PoseGrid : -
    (0x2F0B4000, 0x2F08E000), // CPlugAnimGraphNode_GraphInput : -
    (0x2F0B5000, 0x01001000), // NPlugAnim::SSceneClip : CMwNod
    (0x2F0B8000, 0x01001000), // NPlugItemPlacement::SGroups : CMwNod
    (0x2F0B9000, 0x2F08E000), // CPlugAnimGraphNode_JointLock : -
    (0x2F0BC000, 0x01001000), // NPlugItem::SVariantList : CMwNod
    (0x2F0BE000, 0x2F08E000), // CPlugGraphNode_Graph : -
    (0x2F0BF000, 0x2F08E000), // CPlugGraphNode_StateMachine : -
    (0x2F0C0000, 0x2F08E000), // CPlugGraphNode_Group : -
    (0x2F0C1000, 0x01001000), // CPlugFxSystemNode : CMwNod
    (0x2F0C2000, 0x2F0C1000), // CPlugFxSystemNode_Parallel : -
    (0x2F0C3000, 0x2F0C1000), // CPlugFxSystemNode_Condition : -
    (0x2F0C4000, 0x2F0C1000), // CPlugFxSystemNode_ParticleEmitter : -
    (0x2F0C5000, 0x2F0C1000), // CPlugFxSystemNode_SubFxSystem : -
    (0x2F0C6000, 0x2F0C1000), // CPlugFxSystemNode_UpdateVar : -
    (0x2F0C7000, 0x2F0C1000), // CPlugFxSystemNode_SoundEmitter : -
    (0x2F0CA000, 0x01001000), // NPlugDyna::SKinematicConstraint : CMwNod
    (0x2F0CC000, 0x2F08E000), // CPlugAnimGraphNode_DebugHelper : -
    (0x2F0CD000, 0x2F08E000), // CPlugAnimGraphNode_AssertVar : -
    (0x2F0CE000, 0x2F08E000), // CPlugAnimGraphNode_Funnel : -
    (0x2F0D0000, 0x2F08E000), // CPlugAnimGraphNode_ExtractUnit : -
    (0x2F0D4000, 0x2F08E000), // CPlugAnimGraphNode_SetJointExpr : -
    (0x2F0D5000, 0x2F08E000), // CPlugAnimGraphNode_JointTransConstraint : -
    (0x2F0D6000, 0x2F08E000), // CPlugAnimGraphNode_JointRotConstraint : -
    (0x2F0DB000, 0x01001000), // CPlugShaderCBufferStatic : CMwNod
    (0x2F0E5000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Global : -
    (0x2F0FF000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Locomotion : -
    (0x2F100000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Jump : -
    (0x2F101000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Swim : -
    (0x2F102000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Idle : -
    (0x2F103000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV0 : -
    (0x2F104000, 0x2F08E000), // CPlugAnimGraphNode_AvatarPoseEditor : -
    (0x2F105000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Seated : -
    (0x2F106000, 0x2F08E000), // CPlugAnimGraphNode_AvatarV3_Resting : -
    (0x2F107000, 0x01001000), // SSmWorldSaveFile : CMwNod
    (0x2F108000, 0x2F08E000), // CPlugAnimGraphNode_Avatar_Climb : -
];

/// (engine class id, the id written in files) — `0x1402f3570`.
pub const REMAP: &[(u32, u32)] = &[
    (0x03002000, 0x2404E000),
    (0x0301B000, 0x2403C000),
    (0x0301D000, 0x2400E000),
    (0x0301E000, 0x2400F000),
    (0x0301F000, 0x2400D000),
    (0x0302D000, 0x24027000),
    (0x03033000, 0x24004000),
    (0x03036000, 0x24006000),
    (0x03038000, 0x2401F000),
    (0x03039000, 0x2401A000),
    (0x0303A000, 0x24050000),
    (0x0303B000, 0x24040000),
    (0x03043000, 0x24003000),
    (0x03044000, 0x2400B000),
    (0x03045000, 0x24054000),
    (0x03046000, 0x240BD000),
    (0x03047000, 0x24047000),
    (0x03048000, 0x2409B000),
    (0x0304E000, 0x24005000),
    (0x0304F000, 0x24020000),
    (0x03050000, 0x24021000),
    (0x03051000, 0x24022000),
    (0x03052000, 0x24023000),
    (0x03053000, 0x24024000),
    (0x03054000, 0x24025000),
    (0x03055000, 0x24029000),
    (0x03056000, 0x24064000),
    (0x03057000, 0x24007000),
    (0x03058000, 0x24008000),
    (0x03059000, 0x2403A000),
    (0x0305A000, 0x24011000),
    (0x0305B000, 0x2400C000),
    (0x0305C000, 0x2401C000),
    (0x0305D000, 0x2401D000),
    (0x0305E000, 0x2401E000),
    (0x03073000, 0x2404F000),
    (0x03077000, 0x2406A000),
    (0x03078000, 0x24061000),
    (0x03079000, 0x24076000),
    (0x0307A000, 0x24077000),
    (0x0307C000, 0x24069000),
    (0x0307D000, 0x24091000),
    (0x0307E000, 0x2402D000),
    (0x0307F000, 0x24065000),
    (0x03080000, 0x2405A000),
    (0x03081000, 0x2405F000),
    (0x03082000, 0x2406B000),
    (0x03083000, 0x240CF000),
    (0x03084000, 0x2406D000),
    (0x03085000, 0x24066000),
    (0x03086000, 0x2405E000),
    (0x03087000, 0x24063000),
    (0x03088000, 0x240C7000),
    (0x03089000, 0x240C0000),
    (0x0308A000, 0x2404D000),
    (0x0308C000, 0x2404A000),
    (0x0308D000, 0x24034000),
    (0x0308E000, 0x240A0000),
    (0x0308F000, 0x24039000),
    (0x03090000, 0x24038000),
    (0x03091000, 0x240CC000),
    (0x03092000, 0x2401B000),
    (0x03093000, 0x2403F000),
    (0x03094000, 0x24072000),
    (0x03095000, 0x24095000),
    (0x03096000, 0x240AD000),
    (0x03097000, 0x240AE000),
    (0x03098000, 0x240B8000),
    (0x03099000, 0x240C9000),
    (0x0309A000, 0x24099000),
    (0x0309B000, 0x240A2000),
    (0x0309C000, 0x240A3000),
    (0x0309F000, 0x2408B000),
    (0x030A0000, 0x24070000),
    (0x030A1000, 0x2407A000),
    (0x030A2000, 0x24067000),
    (0x030A3000, 0x24084000),
    (0x030A4000, 0x24088000),
    (0x030A5000, 0x24081000),
    (0x030A6000, 0x24089000),
    (0x030A7000, 0x2406F000),
    (0x030A8000, 0x24068000),
    (0x030A9000, 0x24075000),
    (0x030AA000, 0x24082000),
    (0x030AB000, 0x24083000),
    (0x030AC000, 0x24094000),
    (0x030AD000, 0x2408A000),
    (0x030AE000, 0x24052000),
    (0x030AF000, 0x24048000),
    (0x030B1000, 0x2405D000),
    (0x030B2000, 0x2406C000),
    (0x030B3000, 0x2407B000),
    (0x030B4000, 0x2407C000),
    (0x030B5000, 0x2407D000),
    (0x030B6000, 0x240B9000),
    (0x030B7000, 0x240BA000),
    (0x030B8000, 0x24059000),
    (0x030B9000, 0x240A4000),
    (0x030BA000, 0x240A5000),
    (0x030BB000, 0x2402A000),
    (0x030BC000, 0x2409A000),
    (0x030BD000, 0x240A8000),
    (0x030BE000, 0x240A1000),
    (0x030BF000, 0x240A6000),
    (0x030C1000, 0x240AC000),
    (0x030C2000, 0x240CB000),
    (0x030C7000, 0x240CE000),
    (0x030C8000, 0x240C3000),
    (0x030C9000, 0x24053000),
    (0x030CA000, 0x240CA000),
    (0x030CB000, 0x24028000),
    (0x030CC000, 0x2403B000),
    (0x030CE000, 0x24019000),
    (0x030D1000, 0x24012000),
    (0x030D2000, 0x2402B000),
    (0x030D3000, 0x24033000),
    (0x030D5000, 0x240C5000),
    (0x030D6000, 0x240C2000),
    (0x030D7000, 0x240B2000),
    (0x030D8000, 0x240BC000),
    (0x030D9000, 0x240C8000),
    (0x030DA000, 0x240CD000),
    (0x030DB000, 0x240A9000),
    (0x030DD000, 0x240C1000),
    (0x030DE000, 0x24097000),
    (0x030DF000, 0x24098000),
    (0x030E0000, 0x24049000),
    (0x09026000, 0x09068000),
    (0x090C7000, 0x0A016000),
    (0x090C9000, 0x0A07C000),
    (0x090CA000, 0x0A024000),
    (0x090CD000, 0x03015000),
    (0x090E5000, 0x0A06A000),
    (0x090E6000, 0x0A010000),
    (0x090E7000, 0x0A04D000),
    (0x090E8000, 0x0A039000),
    (0x090E9000, 0x0A015000),
    (0x090EA000, 0x0A02D000),
    (0x090EB000, 0x0A02E000),
    (0x090EC000, 0x0A030000),
    (0x090ED000, 0x0A029000),
    (0x090EF000, 0x0A01F000),
    (0x090F1000, 0x0A031000),
    (0x090F2000, 0x0A071000),
    (0x090F4000, 0x03031000),
    (0x0915D000, 0x0303C000),
    (0x0917A000, 0x2E00E000),
    (0x0917E000, 0x05034000),
    (0x0917F000, 0x05035000),
    (0x09180000, 0x0503A000),
    (0x09181000, 0x05045000),
    (0x09182000, 0x05046000),
    (0x09183000, 0x05047000),
    (0x09184000, 0x05036000),
    (0x09185000, 0x0501A000),
    (0x0C030000, 0x0A03D000),
    (0x0C031000, 0x0A078000),
    (0x0C032000, 0x0A07F000),
    (0x11001000, 0x0313C000),
    (0x2E001000, 0x2400A000),
    (0x2E002000, 0x2403E000),
];

/// (id as read from a file, engine class id) — `0x1402f2610`.
pub const NORMALISE: &[(u32, u32)] = &[
    (0x03004000, 0x2E008000),
    (0x03015000, 0x090CD000),
    (0x0301A000, 0x2E001000),
    (0x0301C000, 0x2E002000),
    (0x03031000, 0x090F4000),
    (0x03074000, 0x2E005000),
    (0x0307B000, 0x03078000),
    (0x030FD000, 0x2E004000),
    (0x030FE000, 0x2E003000),
    (0x0313B000, 0x2E009000),
    (0x0313C000, 0x11001000),
    (0x03168000, 0x09189000),
    (0x0501A000, 0x09185000),
    (0x05034000, 0x0917E000),
    (0x05035000, 0x0917F000),
    (0x05036000, 0x09184000),
    (0x0503A000, 0x09180000),
    (0x05045000, 0x09181000),
    (0x05046000, 0x09182000),
    (0x05047000, 0x09183000),
    (0x06005000, 0x06004000),
    (0x0702B000, 0x09093000),
    (0x0800D000, 0x090B4000),
    (0x08010000, 0x090B5000),
    (0x08011000, 0x090B6000),
    (0x08012000, 0x090B7000),
    (0x08050000, 0x090E5000),
    (0x08051000, 0x09126000),
    (0x08053000, 0x090BF000),
    (0x08055000, 0x09125000),
    (0x0805A000, 0x090B2000),
    (0x0805B000, 0x090B3000),
    (0x0900D000, 0x0900F000),
    (0x09012000, 0x09047000),
    (0x09063000, 0x09026000),
    (0x09068000, 0x09026000),
    (0x090E3000, 0x2E006000),
    (0x090E4000, 0x2E007000),
    (0x09120000, 0x2E028000),
    (0x0A010000, 0x090E6000),
    (0x0A015000, 0x090E9000),
    (0x0A016000, 0x090C7000),
    (0x0A01F000, 0x090EF000),
    (0x0A024000, 0x090CA000),
    (0x0A029000, 0x090ED000),
    (0x0A02D000, 0x090EA000),
    (0x0A02E000, 0x090EB000),
    (0x0A030000, 0x090EC000),
    (0x0A031000, 0x090F1000),
    (0x0A039000, 0x090E8000),
    (0x0A03D000, 0x0C030000),
    (0x0A04D000, 0x090E7000),
    (0x0A06A000, 0x090E5000),
    (0x0A071000, 0x090F2000),
    (0x0A078000, 0x0C031000),
    (0x0A07C000, 0x090C9000),
    (0x0A07F000, 0x0C032000),
    (0x24003000, 0x03043000),
    (0x24004000, 0x03033000),
    (0x24005000, 0x0304E000),
    (0x24006000, 0x03036000),
    (0x24007000, 0x03057000),
    (0x24008000, 0x03058000),
    (0x2400A000, 0x2E001000),
    (0x2400B000, 0x03044000),
    (0x2400C000, 0x0305B000),
    (0x2400D000, 0x0301F000),
    (0x2400E000, 0x0301D000),
    (0x2400F000, 0x0301E000),
    (0x24011000, 0x0305A000),
    (0x24012000, 0x030D1000),
    (0x24019000, 0x030CE000),
    (0x2401A000, 0x03039000),
    (0x2401B000, 0x03092000),
    (0x2401C000, 0x0305C000),
    (0x2401D000, 0x0305D000),
    (0x2401E000, 0x0305E000),
    (0x2401F000, 0x03038000),
    (0x24020000, 0x0304F000),
    (0x24021000, 0x03050000),
    (0x24022000, 0x03051000),
    (0x24023000, 0x03052000),
    (0x24024000, 0x03053000),
    (0x24025000, 0x03054000),
    (0x24027000, 0x0302D000),
    (0x24028000, 0x030CB000),
    (0x24029000, 0x03055000),
    (0x2402A000, 0x030BB000),
    (0x2402B000, 0x030D2000),
    (0x2402D000, 0x0307E000),
    (0x24033000, 0x030D3000),
    (0x24034000, 0x0308D000),
    (0x24038000, 0x03090000),
    (0x24039000, 0x0308F000),
    (0x2403A000, 0x03059000),
    (0x2403B000, 0x030CC000),
    (0x2403C000, 0x0301B000),
    (0x2403E000, 0x2E002000),
    (0x2403F000, 0x03093000),
    (0x24040000, 0x0303B000),
    (0x24047000, 0x03047000),
    (0x24048000, 0x030AF000),
    (0x24049000, 0x030E0000),
    (0x2404A000, 0x0308C000),
    (0x2404D000, 0x0308A000),
    (0x2404E000, 0x03002000),
    (0x2404F000, 0x03073000),
    (0x24050000, 0x0303A000),
    (0x24052000, 0x030AE000),
    (0x24053000, 0x030C9000),
    (0x24054000, 0x03045000),
    (0x24059000, 0x030B8000),
    (0x2405A000, 0x03080000),
    (0x2405D000, 0x030B1000),
    (0x2405E000, 0x03086000),
    (0x2405F000, 0x03081000),
    (0x24061000, 0x03078000),
    (0x24062000, 0x03078000),
    (0x24063000, 0x03087000),
    (0x24064000, 0x03056000),
    (0x24065000, 0x0307F000),
    (0x24066000, 0x03085000),
    (0x24067000, 0x030A2000),
    (0x24068000, 0x030A8000),
    (0x24069000, 0x0307C000),
    (0x2406A000, 0x03077000),
    (0x2406B000, 0x03082000),
    (0x2406C000, 0x030B2000),
    (0x2406D000, 0x03084000),
    (0x2406F000, 0x030A7000),
    (0x24070000, 0x030A0000),
    (0x24072000, 0x03094000),
    (0x24075000, 0x030A9000),
    (0x24076000, 0x03079000),
    (0x24077000, 0x0307A000),
    (0x2407A000, 0x030A1000),
    (0x2407B000, 0x030B3000),
    (0x2407C000, 0x030B4000),
    (0x2407D000, 0x030B5000),
    (0x24081000, 0x030A5000),
    (0x24082000, 0x030AA000),
    (0x24083000, 0x030AB000),
    (0x24084000, 0x030A3000),
    (0x24088000, 0x030A4000),
    (0x24089000, 0x030A6000),
    (0x2408A000, 0x030AD000),
    (0x2408B000, 0x0309F000),
    (0x24091000, 0x0307D000),
    (0x24094000, 0x030AC000),
    (0x24095000, 0x03095000),
    (0x24097000, 0x030DE000),
    (0x24098000, 0x030DF000),
    (0x24099000, 0x0309A000),
    (0x2409A000, 0x030BC000),
    (0x2409B000, 0x03048000),
    (0x240A0000, 0x0308E000),
    (0x240A1000, 0x030BE000),
    (0x240A2000, 0x0309B000),
    (0x240A3000, 0x0309C000),
    (0x240A4000, 0x030B9000),
    (0x240A5000, 0x030BA000),
    (0x240A6000, 0x030BF000),
    (0x240A8000, 0x030BD000),
    (0x240A9000, 0x030DB000),
    (0x240AB000, 0x0303C000),
    (0x240AC000, 0x030C1000),
    (0x240AD000, 0x03096000),
    (0x240AE000, 0x03097000),
    (0x240B2000, 0x030D7000),
    (0x240B5000, 0x0308C000),
    (0x240B8000, 0x03098000),
    (0x240B9000, 0x030B6000),
    (0x240BA000, 0x030B7000),
    (0x240BC000, 0x030D8000),
    (0x240BD000, 0x03046000),
    (0x240C0000, 0x03089000),
    (0x240C1000, 0x030DD000),
    (0x240C2000, 0x030D6000),
    (0x240C3000, 0x030C8000),
    (0x240C5000, 0x030D5000),
    (0x240C7000, 0x03088000),
    (0x240C8000, 0x030D9000),
    (0x240C9000, 0x03099000),
    (0x240CA000, 0x030CA000),
    (0x240CB000, 0x030C2000),
    (0x240CC000, 0x03091000),
    (0x240CD000, 0x030DA000),
    (0x240CE000, 0x030C7000),
    (0x240CF000, 0x03083000),
    (0x240D4000, 0x030F3000),
    (0x2E00E000, 0x0917A000),
    (0x2E035000, 0x09189000),
];

/// Classes whose virtual `Archive` (vtable slot 14) never reaches
/// `CMwNod::Archive` 0x1402d0720 — a plain-struct body without the chunk
/// loop, hence without the dummy write: `asmdig vtables tm.asm
/// Trackmania.exe 1402d0720 --reg2 1402ea9e0 --rust` (92 of 1857 vtables;
/// CPlugVegetTreeModel, CPlugDynaObjectModel, CPlugPrefab,
/// CPlugStaticObjectModel, the NPlug*::S* structs, the CPlugFile* loaders).
pub const NO_FOLD_CLASSES: &[u32] = &[
    0x0300A000, // vtable 141c3cb28, Archive 140befa00
    0x0300E000, // vtable 141c42770, Archive 140101a20
    0x03010000, // vtable 141c61708, Archive 140d19040
    0x0302E000, // vtable 141c51138, Archive 140101a20
    0x0302F000, // vtable 141c615d0, Archive 140d18d00
    0x03067000, // vtable 141c61368, Archive 140d18720
    0x03069000, // vtable 141c614a0, Archive 140310980
    0x0306A000, // vtable 141c5e1c0, Archive 140cf8a20
    0x0308A000, // vtable 141c3c9d8, Archive 140101a20
    0x030BB000, // vtable 141c3d160, Archive 140101a20
    0x030CB000, // vtable 141c3d338, Archive 140bf4830
    0x030EC000, // vtable 141c61840, Archive 140d193b0
    0x030F5000, // vtable 141c61978, Archive 140d1abd0
    0x03192000, // vtable 141c305a8, Archive 140d28270
    0x0324C000, // vtable 141c61ab0, Archive 140d1b3a0
    0x09007000, // vtable 141bcd8a0, Archive 1405ae890
    0x09019000, // vtable 141bc4cd8, Archive 140101a20
    0x09020000, // vtable 141bbba60, Archive 140101a20
    0x09022000, // vtable 141bb28a0, Archive 14044bdc0
    0x09023000, // vtable 141bbb7e8, Archive 14044bdc0
    0x09024000, // vtable 141baf2c0, Archive 14044bdc0
    0x09025000, // vtable 141badfb8, Archive 14044bdc0
    0x0902D000, // vtable 141bcc518, Archive 140101a20
    0x0902F000, // vtable 141ba6b18, Archive 140417990
    0x09030000, // vtable 141bbc140, Archive 140101a20
    0x09031000, // vtable 141bc3f20, Archive 140101a20
    0x09035000, // vtable 141bb26e0, Archive 140101a20
    0x0903D000, // vtable 141bbb660, Archive 14044bdc0
    0x09040000, // vtable 141baaf98, Archive 140101a20
    0x09041000, // vtable 141bb9768, Archive 1404d3b40
    0x09049000, // vtable 141bc4ee0, Archive 14055f960
    0x09054000, // vtable 141bc4498, Archive 1404d3b40
    0x09055000, // vtable 141bc5038, Archive 140101a20
    0x0905A000, // vtable 141bc3c10, Archive 140101a20
    0x0905F000, // vtable 141bc45e0, Archive 14044bdc0
    0x09060000, // vtable 141ba7508, Archive 14044bdc0
    0x0906C000, // vtable 141bbd480, Archive 140101a20
    0x09074000, // vtable 141bb3968, Archive 140101a20
    0x09075000, // vtable 141ba6350, Archive 140101a20
    0x09076000, // vtable 141bb2ea8, Archive 140101a20
    0x09077000, // vtable 141bb9a80, Archive 140101a20
    0x09084000, // vtable 141bb1db0, Archive 140101a20
    0x09085000, // vtable 141bc3d80, Archive 140101a20
    0x0908B000, // vtable 141bc38b0, Archive 140101a20
    0x09098000, // vtable 141be2e48, Archive 140101a20
    0x09099000, // vtable 141bc5170, Archive 140101a20
    0x0909B000, // vtable 141bc52b8, Archive 140101a20
    0x090A5000, // vtable 141bc5548, Archive 140101a20
    0x090AD000, // vtable 141bbef38, Archive 14044bdc0
    0x090BC000, // vtable 141bc5400, Archive 140101a20
    0x090C0000, // vtable 141bb2550, Archive 140101a20
    0x090DB000, // vtable 141baef60, Archive 14044bdc0
    0x09108000, // vtable 141bc3730, Archive 14044bdc0
    0x0911D000, // vtable 141bb7140, Archive 1404baf20
    0x0912F000, // vtable 141bb43b8, Archive 1404a7a40
    0x09144000, // vtable 141bd6348, Archive 14061bc40
    0x09145000, // vtable 141bafd50, Archive 14059a090
    0x09149000, // vtable 141baf8a0, Archive 140465e70
    0x09159000, // vtable 141bb1328, Archive 140465380
    0x0915E000, // vtable 141ba6010, Archive 14040c580
    0x09164000, // vtable 141bd0ea8, Archive 1405c9050
    0x09166000, // vtable 141bcd230, Archive 14059b4b0
    0x09178000, // vtable 141bc88f8, Archive 14057f400
    0x09179000, // vtable 141bc8808, Archive 14057f420
    0x0917B000, // vtable 141bd7778, Archive 140621e20
    0x0917D000, // vtable 141ba5f20, Archive 14040ca90
    0x09187000, // vtable 141bd9c90, Archive 140629dc0
    0x09188000, // vtable 141bd7688, Archive 1406221e0
    0x12001000, // vtable 141b7ad80, Archive 1418d62f4
    0x12004000, // vtable 141b7c4f0, Archive 140310980
    0x12007000, // vtable 141b7d748, Archive 140318dd0
    0x12008000, // vtable 141b7d0a8, Archive 140310740
    0x12009000, // vtable 141b7c3d0, Archive 140310ac0
    0x12010000, // vtable 141b7ce68, Archive 14030f6f0
    0x12015000, // vtable 141d15838, Archive 140101a20
    0x1201B000, // vtable 141b7d628, Archive 140312910
    0x12021000, // vtable 141b7c2b0, Archive 140310da0
    0x12038000, // vtable 141b7c190, Archive 140310be0
    0x24035000, // vtable 141cfbea8, Archive 140101a20
    0x24036000, // vtable 141d04370, Archive 140101a20
    0x2D007000, // vtable 141cea2b0, Archive 1412b9dd0
    0x2D036000, // vtable 141cea3d0, Archive 1412ba330
    0x2F06C000, // vtable 141bb5180, Archive 1404ab650
    0x2F06D000, // vtable 141bb4fa0, Archive 1404ab660
    0x2F086000, // vtable 141bb5090, Archive 1404ab640
    0x2F08A000, // vtable 141bbbc70, Archive 1404ec760
    0x2F08D000, // vtable 141c8e498, Archive 1414ba590
    0x2F0B5000, // vtable 141bc7180, Archive 140578620
    0x2F0B8000, // vtable 141bd9d80, Archive 140629d60
    0x2F0BC000, // vtable 141bd9f60, Archive 140629e20
    0x2F0CA000, // vtable 141bb42c8, Archive 1404a8410
    0x2F107000, // vtable 141cec818, Archive 141355ee0
];
