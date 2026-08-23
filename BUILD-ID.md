# Does the engine carry old builds' physics, and can a build id select them?

Arm `disasm`, node `119536.od.fbinfra.net`, 2026-08-22. **Static analysis of the
freely-distributed dedicated server** `TrackmaniaServer_Latest`
(30 113 288 B, `date=2026-05-15_18_00 git=128182-0de74ece09e GameVersion=3.3.0`,
md5 `0f0f4b25f31f80c60c81404366c95e68`), with the 2022-06-21 server
(`git=113135`, md5 `9038fe1ffddccf3b0a3f52901fe63cc0`) as the comparison build.
Nothing was patched in either binary. Claim tags follow `CLAIMS.md`.

**The answer, in one line: no. The binary contains exactly three date-keyed
branches, none of them selects a simulation, and the recording's build stamp
reaches nothing but a message. Old physics needs an old binary.**

---

## 0. What the question reduces to

The build identity enters the program in exactly two forms:

* **the running build's own version**, a global `std::string` at `0x1e5ea28`
  (`g_ExeVersion`), initialised at `0x1a88844` from the literal at `0x34c7cb`,
  `"date=2026-05-15_18_00 git=128182-0de74ece09e GameVersion=3.3.0"`;
* **a recording's or a map's stored version**, a per-object string field
  (`CGameCtnGhost` `+0x1c0`, `CGameCtnChallenge` `+0x78`).

So "is there a version switch" is answerable by enumerating what those two
things reach. Both enumerations are below, and both are exhaustive over their
form.

*Addresses are file/objdump addresses. Ghidra's listing is offset by
`+0x100000` from these (its `FUN_0105e8f0` is this file's `0xf5e8f0`).*

## 1. `Validate_ExeVersion` / `Validate_ExeChecksum`: where they are read

**MEASURED — the field offsets, from the reflection table.** The
`CGameCtnGhost` class descriptor is built at `0xe48cf4`–`0xe494f2`; each member
registers as *(C++ expression string, byte offset, script name)*:

| script name | offset | expression |
|---|---|---|
| `Validate_ChallengeUid` | `0x164` | |
| `Validate_ScopeId` | `0x170` | |
| `Validate_GameMode` | `0x180` | |
| `Validate_GameModeCustomData` | `0x190` | |
| **`Validate_ExeVersion`** | **`0x1c0`** | `m_ValidationData.ExeVersion` (`0xe49348`) |
| **`Validate_ExeChecksum`** | **`0x1d0`** | `m_ValidationData.ExeChecksum` (`0xe4939d`) |
| `Validate_OsKind` | `0x1d4` | |
| `Validate_CpuKind` | `0x1d8` | |
| `Validate_TitleId` | `0x1e8` | |
| `Validate_ExtraTool_Info` | `0x230` | |

*Control, from a second and independent place in the binary:* the ghost's own
GBX archiver at `0xe4a884` writes `+0x1c0` with the **string** archiver
(`0x1b3f2c0`), then `+0x1d0`, `+0x1d4`, `+0x1d8` with the **u32** archiver
(`0x1b3ef10`), then `+0x1e8` and `+0x230` as strings — the same offsets, in the
same order, with types that match the names. Two unrelated code paths agree, so
the offsets are not a reading of one ambiguous site.

**MEASURED — `ExeVersion` has exactly two consumers.** Searching the whole
21 MB `.text` for the field's small-string triple (`+0x1c0` / `+0x1cb` /
`+0x1cc`) and reading every ghost-typed hit:

1. **`0x113bba0`** — the JSON writer, emitting it as `"GameBuild"` (the literal
   `GameBuild` at `0x32460d` is referenced once in the binary, here).
2. **`0x11841c8`** — the validation verdict, §2 below.

**MEASURED — `ExeChecksum` is never compared.** Its only readers are the
archiver above and `0xc85996`, which copies `ExeVersion` and `ExeChecksum` side
by side into a buffer being built for upload (the function's own strings are
`Unable to retrieve map info` and `Unable to build buffer`). No branch anywhere
tests it. *Control for the search method: the same offset-window search over
`+0x1c0`+`+0x1d0` does return the archiver and the upload site, so it is not a
search that finds nothing.*

## 2. The one branch the recording's stamp reaches — and it runs after the race

The verdict builder is `0x1183b40`. Decompiled, the relevant tail is:

```c
if (*status != 1) {                                   /* NOT already valid   */
    ...                                               /* simulation hazards  */
    i = find(ghost->ExeVersion, "date=", 5);          /* 0x1184206          */
    sub = (i == -1) ? ghost->ExeVersion : substr(i);  /* 0x1184220          */
    if (compare(sub, "date=2021-09-22", 15) < 0) {    /* 0x1184248          */
        *status = 3;                                  /* 0x1184263          */
        printf("using known-flawed game exe '%s'\n", ghost->ExeVersion);
    }                                                 /* 0x118427e          */
}
```

Three things follow from the shape of that code, and all three were then
measured on the live server:

* it runs **after** the simulation, on the result record;
* it is **skipped entirely when the run validated** (`cmp [status],1 ; je` at
  `0x1184158`);
* its only effect is `status 2 → 3` plus a line of text.

**MEASURED — the 2×2, on KEKL- SAUSAGE ICE with our own 67.200 TAS tape, whose
stamp is `Trackmania date=2026-02-02_17_51 git=128149-...`.** The stamp was
rewritten inside the compressed body with `tools/strpatch` (same length, 2
occurrences, nothing else touched); the declared time was moved to 60.000 with
`ghost declare` to produce a failing run:

| file | stamp | declared | server verdict |
|---|---|---|---|
| as recorded | 2026-02-02 | 67.200 | `IsValid: true`, **Time 67.200** |
| stamp patched | **2020-01-01** | 67.200 | `IsValid: true`, **Time 67.200**, no message |
| stamp patched | 2029-09-09 | 67.200 | `IsValid: true`, **Time 67.200**, no message |
| declared 60.000 | 2026-02-02 | 60.000 | `Is Invalid`, `wrong simu, but reached some checkpoints (4 out of 5)` |
| declared 60.000 | **2020-01-01** | 60.000 | `Unvalidable`, same line **plus** `using known-flawed game exe 'Trackmania date=2020-01-01_17_51 ...'` |

*This is the positive control for the whole method.* The message appears in
exactly the cell the disassembly predicts — old stamp **and** an already-failing
run — and in none of the other three; the server echoes the patched stamp in
`GameBuild` in every row, so it is demonstrably reading the byte that was
changed; and **the simulated time is identical in every row**. A build stamp
cannot change what the engine computes; on a failing run it can only change
which failure bucket the result is filed under.

Raw server output: `raw/live_2x2/`.

## 3. The other two date gates: the map's era, not the ghost's

**MEASURED — the whole binary contains exactly three `date=YYYY-MM-DD`
literals** (`raw/date_literals_2026.txt`): `2021-09-22` (§2), **`2023-10-15`**
and **`2023-11-30`**. Both of the 2023 ones are referenced once, from one
function, `CGameCtnChallenge`'s post-load method at `0xf5e8f0` (its vtable at
`0x1c0dd20` also holds `0xf5ece0`, the chunk-id builder that ORs `0x3043000`,
which is what identifies the class). Decompiled:

```c
if (challenge->exeVersion.empty())            /* 0xf5e96e */
    challenge->exeVersion = g_ExeVersion;     /* the running build */
if (compare(exeVersion, "date=2023-10-15", 15) < 0)
    challenge->flags_0x300 |= 2;              /* 0xf5e9a6 */
if (compare(exeVersion, "date=2023-11-30", 15) < 0) {
    for (each of three object lists at +0x278, +0x288, +0x2a8)
        if (IdStartsWith(obj->id_0x18, "Snow", 4))   /* 0x19a9180: strncmp==0 */
            obj->byte_0x9c = 0;               /* +0xc0 for the third list */
}
```

`0x19a9180` resolves an interned GBX `Id` and returns `strncmp(name, "Snow", 4)
== 0`, so the match is on names **beginning with `Snow`**, and the byte is
cleared **for** those. The enum table at `0x1cb1f48` names the neighbourhood:
`NoSteering`, `ForceAcceleration`, `Bumper`, `ReactorBoost_Legacy`, `Bouncy`,
`ReactorBoost_Oriented`, `VehicleTransform_Reset`, `VehicleTransform_CarSnow`,
`…CarRally`, `…CarDesert` — gameplay-surface effects. So this is content
compatibility for the 2023 alternate-car update: a map saved before those
gameplay effects existed does not get them.

**MEASURED — on KEKL- SAUSAGE ICE the gate is inert.** The map's stored version
is `date=2022-07-06_11_37`, i.e. it is *already* on the old side of both gates.
Rewriting it to `date=2024-07-06` (`strpatch`, 1 occurrence, body only) and
re-validating the 67.200 tape returns **67.200, `IsValid: true`** — the same
result as the unpatched map. *Control: the instrument is the one that did move
the outcome in §2, and the server loaded the patched map normally.*

**UNKNOWN — what reads `flags_0x300 & 2`.** The bit is written once and I did
not find its reader (offset-only search is ambiguous across classes). *What
would settle it: type the class in Ghidra and take references on the field.*
**UNKNOWN — whether the `Snow` byte can change a simulation**, since no map in
this project's corpus carries Snow-prefixed gameplay content. *What would settle
it: a map with a `VehicleTransform_CarSnow` surface, validated with its stamp on
each side of 2023-11-30.* Neither UNKNOWN is a candidate era switch: both are
map-content flags, keyed on the **map's** save stamp, not on any tape's.

## 4. The running build's own version is never compared

**MEASURED — `g_ExeVersion` (`0x1e5ea28`) has 16 code references; 6 are its
construction and teardown (`0x1a4febb`, `0x1a4fec6`, `0x1a88844`, `0x1a888c7`,
`0x1a89ca9`, `0x1a89cb2`) and the other 10 are string formatting.** Read
individually:
`0x90e0c5` (`[Gbx] Exe date = %s, game name = %s, game build info = %s`),
`0x90f1b5`, `0x913b01` (`exe: %s`), `0x914f19`, `0xf09ed5`, `0xf606f4`,
`0x1495088`, `0x169ea66` (stamping a saved file's own `+0x1c0`), `0x1a05e3e`
(`* BuildInfo :`), and `0xf5e96e` (the default in §3). Every one is an assign,
an append or a `printf`. The same holds for the two sibling globals
`0x1e5ea38` and `0x1e5ea48` (`2026-05-15_18_00`).

*Control, and it is the point of the measurement:* a window scan for the four
string-compare helpers (`0x1b28a50`, `0x1b288e0`, `0x1b28bd0`, `0x19a9180`)
around each reference returns **4 hits at the three date-literal sites of §2–§3
and 0 hits at all 53 build-identity references (16 + 15 + 22).** The scan can see comparisons;
there are none here.

**MEASURED — the build *number* is never parsed.** There is no `git=` literal in
the binary (only the full banner contains the substring), so nothing splits
`128182` out of the version string to compare it. *Control: `date=`, the prefix
that IS parsed, is present as its own literal at `0x3430a4` and is referenced
exactly once — at `0x11841f8`, the known-flawed check.*

## 5. `IsLegacy` and `ReactorBoost_Legacy` are not eras

**MEASURED — `IsLegacy` is a buddy-list field.** It is registered at
`0xc4e9c9` as member `0x0b`, offset `0x30`, of class id `0x0317C000`, whose
member list (same descriptor, `0xc4e440`+) reads: `IsAlly`,
`IsBuddyInManiaPlanet`, `IsXmpp`, `IsSteam`, **`IsLegacy`**, `IsOnlineInXmpp`,
`IsOnlineInSteam`, **`IsOnlineInLegacy`**, `CurrentServerName`,
`CurrentServer_IsLobby`, `PresenceStatus`, `ESubscription`, and the class name
string in the same descriptor is **`CGameScriptChatContact`**. "Legacy" here is
the old Nadeo account/chat backend, as opposed to XMPP and Steam. It has
nothing to do with the simulation.

**MEASURED — `ReactorBoost_Legacy` is a value, not a mode.** It has no code
reference at all; it appears twice in data, in the enum-name tables at
`0x1cb1fb0` and `0x1cb5a98`, one slot away from `ReactorBoost2_Legacy` and four
from `ReactorBoost_Oriented` / `ReactorBoost2_Oriented`. All four are values of
the same gameplay-surface enum and **coexist in one build**: they are the
classic up-boost and the ramp-oriented boost that a mapper places, not two eras
of one boost.

## 6. There is no second engine in the binary

| | 2022-06-21 (`113135`) | 2026-05-15 (`128182`) |
|---|---|---|
| `.text` | 20 835 232 B | 21 082 150 B (**+1.19 %**) |
| functions (split at `int3` padding) | 59 493 | 59 754 |
| float-heavy functions (≥150 insns, ≥20 % SSE) | **1 089** | **1 043** |
| `date=YYYY-MM-DD` gate literals | `2021-08-11`, `2021-09-02` | `2021-09-22`, `2023-10-15`, `2023-11-30` |

**MEASURED, with `tools/asmshape`** (new; a function-shape index over an
`objdump` text, cosine over mnemonic histograms). Four years of updates added
1.19 % of code and the count of float-heavy functions went **down**. Searching
the 2026 build for near-duplicate float-heavy pairs — the shape a second
vehicle solver would have — returns **20 pairs at cosine ≥ 0.995
(`raw/dups_cur.txt`)**, and they are read/write twins and library math:
`0xb942e0`/`0xb949e0` are `state → struct` and `struct → state` for the same
fields; `0x1ab9e30`/`0x1aba270`/`0x1abad90` and `0x1ac9a70`/`0x1aca250` sit
above `0x1a90000`, in the vendored runtime. No large game-code pair.

*Control: the same measure, run across the two binaries, matches 168 of the
2022 build's 1 089 float-heavy functions to a 2026 function at ≥ 0.995
(`raw/match_old_cur.txt`).* It can pair a function with its self in another
build — so "no duplicate pairs inside one build" is a result, not a blind
instrument.

**MEASURED — the gate inventory changes in both directions.** The 2026 build
does not contain `date=2021-08-11` or `date=2021-09-02`, which the 2022 build
compares against; the 2022 build has no map-era gate at all. Compatibility
gates are added when a content change lands and **deleted** later. Nothing
accumulates.

*Cross-check on a different executable format, with a different tool:* plain
`strings` on the shipped Windows server `TrackmaniaServer.exe` (PE32+, same
zip) finds the same three literals and the same `known-flawed` message; the
2022 zip's `.exe` finds its own two. Not an artefact of my objdump pipeline.

## 7. The shape of the answer

**(b), and firmly.** The engine does not carry old builds' physics, and there is
no version switch to drive. What exists is:

* **one** post-hoc verdict gate on the recording's stamp (2021-09-22), which
  cannot change a simulated time and cannot fail a passing run;
* **two** map-content gates on the map's own save stamp (2023-10-15,
  2023-11-30), which turn off 2023 alternate-car gameplay for maps saved before
  it existed;
* nothing else. The build's own version string is formatted, sent and stamped
  into files, never compared; the build number is never parsed; `ExeChecksum` is
  archived and uploaded, never tested.

That is exactly consistent with what the black-box arm measured independently
(`tm-oldbuild/OLDBUILD.md` §7: rewriting the stamp moves no outcome) and it
explains *why*: the branch it would have to hit does not exist. **To run 2022
physics you run the 2022 binary** — the route that arm already verified 15 of
15 on KEKL- SAUSAGE ICE.

### Boundaries of this result, stated plainly

* **Scope: the dedicated server only.** The retail client was not examined and
  no protection on it was touched. The server is a complete oracle for its own
  era — `/validatepath` re-simulates — so a client-only era switch would have to
  be a client-only feature; strictly, **UNKNOWN**. *What would settle it: the
  same three-literal enumeration on the client binary, which needs a decision
  about the client's protection and is not this arm's to make.*
* The two UNKNOWNs of §3 (the `0x300` bit-1 reader; whether the `Snow` byte can
  move a simulation) are open tasks, not conclusions.
* The date-literal enumeration is exhaustive **over that form**. A gate written
  as an integer comparison against a parsed build number would not be in it —
  which is why §4 measures separately that no such parse exists.

## 8. Reproducing this

```bash
objdump -d --no-show-raw-insn -M intel TrackmaniaServer > cur.asm
strings -a -t x TrackmaniaServer > cur.strings.txt
grep -nE '^ [0-9a-f]+ date=' cur.strings.txt          # the three gates
grep -n '# 3487cf' cur.asm                             # -> 0x1184248
tools/h.sh fn 1184248                                  # the verdict function
```

`tools/h.sh` (`fn` / `xs` / `str`) is the flat-text navigator used throughout;
`tools/asmshape` is the duplicate-function index of §6. Ghidra 11.3.1 headless
was used for the decompilations, with `DecompAt.java` and the `+0x100000`
address offset noted in §0.
