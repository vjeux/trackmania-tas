# A ghost file, completely accounted for

Every byte of a `.Ghost.Gbx` is identified. This replaces the working
assumption that a ghost is ~27 chunks of which we understand six.

## The classes (openplanet.dev, Trackmania Next API)

`next.openplanet.dev/Game/<Class>` documents the runtime nods, and their class
IDs are exactly our chunk-id prefixes:

    CGameGhost           0x0303F000
    CGameCtnGhost        0x03092000   inherits CGameGhost
    CPlugEntRecordData   0x0911F000   the telemetry record

`CGameCtnGhost`'s full member list, from the 2026-01-26 build:

    uint Duration            uint Size
    MwId ModelIdentName      MwId ModelIdentAuthor
    EDummyCollectionIdent ModelIdentCollection
    SConstString ModelIdentCollection_Text
    string GhostLogin        string GhostTrigram
    wstring GhostCountryPath wstring GhostNickname
    EGhostNameLogoType m_GhostNameLogoType
    wstring GhostAvatarName  string RecordingContext
    vec3 LightTrailColor
    uint RaceTime            uint NbRespawns          uint StuntsScore
    MwId Validate_ChallengeUid
    SConstString Validate_ScopeType
    string Validate_ScopeId
    string Validate_GameMode string Validate_GameModeCustomData
    string Validate_ExeVersion
    uint Validate_ExeChecksum
    string Validate_TitleId
    string Validate_ExtraTool_Info
    uint Validate_OsKind     uint Validate_CpuKind

Those map one-to-one onto the validation block we already read and write.

## The file is ELEVEN PARTS, not 27 chunks

`all_skip_chunks` byte-scans for the `PIKS` marker, so it finds skippable
chunks AND false positives inside their payloads -- which is where the
"27 chunks" came from. Several of those "payloads" decode to text fragments
like `ic, ` and `d Ki`, i.e. slices landing mid-string.

Merging overlaps gives FIVE top-level spans covering 99.1-100.0 % of the body,
and six gaps totalling 123 bytes. On 287431:

    gap    28 B   0x0303F006  CGameGhost -- flag + a 12-byte zlib blob;
                              BYTE-IDENTICAL across all five test ghosts
    span 10874 B  0x0303F007  CGameGhost -- wraps CPlugEntRecordData,
                              the telemetry: WE AUTHOR THIS
    gap    16 B   0x0309200C (u32 0) + 0x0309200E (u32, varies -- a hash)
    gap     4 B   payload tail
    span     4 B  0x0309200F
    span     4 B  0x03092010
    gap    35 B   u32 flag, u32 len=27, then the 27-char MAP UID
                  ("En6ZbR6_Kun3gRufEymOOUo8DYm")
    span   110 B  0x03092013
    gap    36 B   0x0309201C + a 32-byte hash
    span 17407 B  0x0309201D  the INPUT TAPE: WE AUTHOR THIS
    gap     4 B   0xFACADE01  the GBX body end marker

## Proof the model is complete

Decompose the body at those boundaries, concatenate the parts back, compare:

    parts: 11   original md5 e09ca0c10d5c0b20d5a83c765445c2eb
                 rebuilt md5 e09ca0c10d5c0b20d5a83c765445c2eb
    BYTE-IDENTICAL: True

Lossless decomposition is the precondition for synthesis: anything that can be
taken apart into named parts and put back byte-for-byte can be rebuilt with
different parts.

## So why do we still transplant?

Not because the format is opaque -- it is not. Because of ONE MEASURED FACT:
a container we assembled ourselves passed every offline gate and then CRASHED
THE CLIENT on import:

    staged 1 ghost(s) into _shoot
    read: Connection reset by peer (os error 104)

The offline gates cannot see whatever the loader objects to. Until something
reproduces that check offline, "it validates" is not evidence it will load.

Two items in the list above are the honest remaining unknowns, and both are
hashes we currently copy rather than compute: the u32 in 0x0309200E and the
32 bytes after 0x0309201C. If the client verifies either, a synthesised ghost
fails there -- and that is exactly the kind of thing the crash would look like.

## What synthesis would take, concretely

1. Emit the 0x0303F006 literal (constant across every ghost seen).
2. Emit 0x0303F007 wrapping a CPlugEntRecordData we already build.
3. Emit the scalars, the map UID string, and 0x03092013.
4. COMPUTE the two hashes, or prove the client ignores them.
5. Emit the tape chunk we already build, then 0xFACADE01.

Steps 1-3 and 5 are mechanical. Step 4 is the whole risk, and it is testable
in isolation: corrupt each hash in a known-good ghost, import it, and see
which one the client rejects.
