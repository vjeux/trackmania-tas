# A ghost file, completely accounted for

Every byte of a `.Ghost.Gbx` is identified, and `ghost synth` proves it by
re-emitting one: it decomposes a donor into named chunks and writes every byte
back from parsed values and built-in constants. Byte-identical on all five
staged maps, with ZERO unnamed bytes on four of them.

## The classes (next.openplanet.dev/Game/<Class>)

The class ids are exactly our chunk-id prefixes:

    CGameGhost           0x0303F000
    CGameCtnGhost        0x03092000   inherits CGameGhost
    CPlugEntRecordData   0x0911F000   the telemetry record

`CGameCtnGhost`'s 28 documented members (2026-01-26 build) are what those
chunks serialise:

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

That list decoded a chunk on sight: **0x03092010 is Validate_ChallengeUid**, and
an MwId is a GBX lookback string -- a 0x40000000 marker, a length, then the map
uid in the clear.

## The structure

A ghost body is ~25 ordinary chunks: a run of skippable ones
(`id | PIKS | len | payload`) and a handful of non-skippable ones between them.
The notable pieces:

    0x0303F006   28 B   CGameGhost -- a flag and a 12-byte zlib blob.
                        BYTE-IDENTICAL in all five ghosts, so it is a constant
                        of this build, not something a run determines.
    0x0303F007    4 B
    0x03092000  big     wraps CPlugEntRecordData -- the telemetry WE AUTHOR
    0x0309200C/E  4 B   scalars; 0x0309200E is a hash we copy
    0x0309200F    4 B
    0x03092010   MwId   Validate_ChallengeUid = the map uid
    0x03092013 … 0x0309202E   the metadata chunks, incl. the validation block
    0x0309201C   32 B   a hash we copy
    0x0309201D  big     the INPUT TAPE, which we also author
    0xFACADE01    4 B   body end marker

### Two analysis bugs worth remembering

An earlier version of this document claimed "27 chunks, five giant spans".
Both halves were artefacts of our own tooling:

* `all_skip_chunks` byte-scans for `PIKS`, so it reports phantom chunks inside
  non-skippable payloads. Several phantom "payloads" decode to text fragments
  landing mid-string -- that was the "27 chunks, 21 opaque" picture.
* Collapsing phantoms by merging overlapping spans, merging on `<=` also glues
  ADJACENT chunks together: 0x0303F007 is 4 bytes, and the merge made it look
  10874 bytes long by swallowing 0x03092000 behind it. Merge on `<` --
  strictly inside is a phantom, adjacent is a real neighbour.

The fix for both was to make the tool REPRODUCE THE BYTES. A structural theory
that cannot re-emit its input is not a structure, it is a guess.

## What is still not derivable

Two values, both copied rather than computed:

    the u32 after 0x0309200E
    the 32 bytes after 0x0309201C

`ghost synth --zero-hashes` zeroes both. The dedicated server does not care:

    zeroed   20.756 cps=7
    control  20.756 cps=7

Flipping every bit of both in a donor also still validates.

## So can we build one from scratch?

For everything the SERVER checks: yes, and `ghost synth` is the tool.

The untested surface is the CLIENT loader, and that is precisely where the
historical failure was -- a container we assembled ourselves passed every
offline gate and then crashed the game on import:

    staged 1 ghost(s) into _shoot
    read: Connection reset by peer (os error 104)

No offline gate sees whatever the loader objects to. Until a synthesised ghost
is imported by a running client, "we can synthesise a ghost" means "the server
accepts it", not "the game will load it". That test needs the game up on the
render box and is the next step.
