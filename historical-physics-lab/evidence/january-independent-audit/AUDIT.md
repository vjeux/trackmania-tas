# Independent audit — Historical Physics Lab, January 2022 Stadium profile

Auditor: independent session, no reuse of the generator's own tooling or counts.
Date: 2026-09-03. Rust only; `objdump` used solely as a decoder oracle.

## Verdict

**January must be disabled immediately.** It is the only uncertified native
island in the package that is actually installable, and it writes to wrong
fields of the live vehicle object.

Two independent grounds, either sufficient:

1. **26 structure accesses in the copied January handler read or write the
   wrong field of the current object.** 16 are proven; 10 more are unproven only
   because the January executable is missing. Three of the proven ones are
   *writes* into the live object, and three more take the *address* of a wrong
   sub-object and hand it to callees.
2. **January has no behavior gate.** Fall 2022 is held closed by
   `PROFILE_FALL2022_BEHAVIOR_CERTIFIED = false` and an explicit refusal in
   `InstallJanuary2022`'s sibling. January has no such constant and no such
   check, and its catalog entry is `Selectable = true`. The exact failure that
   closed Fall — an island that runs but whose trajectory matches stock — is
   unguarded for January.

## Inputs and hashes

| Role | Path | SHA-256 |
|---|---|---|
| V5 fail-closed source tar.zst | `tm-historical-physics-official-v5-failclosed.tar.zst` | `8cc088de3425697071dff85dd4527b0cb935b2a176f4cf28c2d8e53a172e6a42` |
| Current client, build 128130 | `Trackmania-current-whitestick.exe` | `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda` |
| January profile, as shipped | `Profile_Jan2022.as` | `ec8888eaaf13d0cde5dde90e60b8fa08eac969ccbfcbdd30d61069715596ef8b` |
| January island, as shipped (12,876 B) | `island-patched.bin` | `3af4b5c266abf3f05301f61aec1d3c81aa4fb1d5707ec467f2edbc9fa60ddf68` |
| January profile, corrected | `Profile_Jan2022.as` (corrected) | `a635f04a6fbbfc5a1fb2828697d8ce1792d695c06af1a92410b356e5d7500d87` |
| January island, corrected (12,876 B) | `island-corrected.bin` | `f8a14a062fb2ff0ac2b1cf13b67dd3e3b71f2f3acf29de4cb49ebfc729b2677a` |
| Generator under audit | `tools/januarygen.rs` | `760afbce57a3a79ccbda8383d8c6528aae875a417e586bb431fa6a7c18af1d64` |
| Shipped verifier under audit | `tools/january_verify.rs` | `a3c0c3467631e25b7528140778ec02cd828cc9723e520e879f8b452c5681c836` |

The tar and the executable hashes match the two the parent independently placed
in `/home/vjeux/tmchg/inbox/`, byte for byte.

## Missing bytes

Named precisely, because everything below was done without them.

| Missing | Identity | What it would settle |
|---|---|---|
| `Trackmania-2022-01-21.exe` | build 105899, SHA-256 `e2255c415f0f7fc2d0a66512fa7609256c42cf639a5380b7a5bcdbb4486ab75b` (declared in `evidence/SOURCE-SHA256SUMS`; not in the bundle, not on the devserver) | copied-region fidelity; the 10 residual omissions; whether any *applied* remap is itself wrong |
| `JAN.objdump` | disassembly of the above over `0x1405EDEB0..0x1405EFF40` plus the twelve helper ranges | independent re-derivation of the aligner's pairings |
| `january-vs-current-fields.tsv` | direct field map | field-map completeness against the evidence that produced it |
| `january-via-spring-fields.tsv` | composed January→Spring→current map | the same, for composed entries |

Consequence: **the shipped verifier `january_verify.rs` cannot be run at all in
this bundle** — it requires all four. Its passing status is not reproducible
from what ships.

The Spring/Sep. 30 client (`1f5ce987…`) *is* present, so a Jan→Spring→current
recomposition becomes possible the moment the January executable arrives.

## What was verified, and how

Five Rust tools, all std-only, in `tools/`:

| Tool | Question it answers |
|---|---|
| `janverify.rs` | Are the manifests internally consistent, in bounds, non-overlapping, and do the declared counts survive re-derivation from the payload? |
| `janasm.rs` | Differential disassembly: did each field patch actually rewrite a ModRM displacement, and are there struct offsets left unremapped or carried as immediates? |
| `janflow.rs` | Does the base register of a contested access hold one object across the region? |
| `janreloc.rs` | Does every rel32/RIP reference carry a relocation, and is every call target a real function entry? |
| `jancorrect.rs` | Apply only the provable repairs, re-verify each differentially, and emit a fail-closed corrected profile. |

### Results against the eight Fall defect classes

| Class | Result |
|---|---|
| Wrong field remaps | **DEFECT.** See below. No *applied* remap could be falsified without the January binary, but the map is internally contradictory at 26 sites. |
| Immediate carriers omitted | **DEFECT (displacement form).** 26 displacement carriers omitted. No *immediate* carrier of a remapped offset exists — but note the generator never audits immediates at all (see "structural cause"). |
| ModRM alias false positives | **CLEAN.** All 161 shipped patches (177 corrected) were shown by differential disassembly to change exactly one memory displacement, with identical mnemonic, length and operand shape. Not one landed on an immediate or a prefix. |
| Helper identity mismatches | **CLEAN by reachability.** 14 copied regions; every one of the 13 helpers is called exactly once; no dead copies, no double redirects. 82 external calls leave through 41 absolute thunks, every target inside an executable section and aligned/padded/prologue-shaped. Semantic identity of each current-side callee remains unverifiable without the January binary. |
| Frame-local / output-buffer ABI | **UNRESOLVED, two concrete risks.** The 28-byte interpolation adapter is shape-correct and its Win64 stack alignment is right (`sub rsp,0x38` leaves the inner call 16-byte aligned, 32 bytes of shadow space, out-pointer restored into `rax`). But it forwards the caller's `r8` — the old output pointer — unchanged into the current scalar helper, so a helper that reads a third integer argument receives a pointer; and it preserves no xmm register across the call. Both need the January binary and the current helper's signature. |
| Bad cave jumps | **CLEAN.** All 105 in-island call targets land on a thunk entry, a copied-region entry, or the adapter. None lands mid-region or in filler. |
| Incomplete calls / RIP refs | **CLEAN.** All 83 relative references inside copied regions carry a relocation; no unrelocated direct branch anywhere in the generated adapter, pool or thunk area. The declared `unresolved=0` survives an independent re-derivation from the island itself. |
| Behavior aliases | **DEFECT (structural).** No `PROFILE_JAN2022_BEHAVIOR_CERTIFIED`, no gate, `Selectable = true`. |

Counts re-derived from the payload, all matching the declarations: 12,876
payload bytes; 161 field relocations; 105 call and 83 RIP relocations (188
combined); 41 absolute thunks; 14 copied regions; 4 initialization values. The
counts were never the problem.

## Defect 1 — 26 omitted field remaps

The generator rewrites a structure displacement only at sites named by its
evidence. Sites it never visits keep the **January** offset and are executed
against the **current** object layout.

Proof that this is a contradiction and not a difference of objects: in the root
handler, `rdi` has exactly **two** definitions — `mov rdi,QWORD PTR [r8+0x8]` at
the prologue (island 30) and `pop rdi` in the epilogue (island 8325). It holds
one object for the whole region. So two sites with the same `[rdi+X]` are the
same field, and treating them differently cannot be right under any field map.

| January offset on `rdi` | remapped to | sites remapped | sites left at the January offset |
|---|---|---:|---:|
| `0x1378` | `0x1280` | 9 | 2 |
| `0x14c4` | `0x1364` | 1 | 2 |
| `0x14ec` | `0x138c` | 1 | 1 |
| `0x1504` | `0x13a4` | 1 | 3 |
| `0x15dc` | `0x147c` | 1 | 1 |
| `0x1628` | `0x14c8` | 1 | 1 |
| `0x1758` | `0x1600` | 9 | 5 |
| `0x18d0` | `0x1780` | 5 | 1 |

Sixteen wrong accesses. Severity is not uniform:

* **Writes into the live object at a wrong offset** — `mov DWORD PTR
  [rdi+0x1758],0x0` twice (island 3685, 3744) and `movss DWORD PTR
  [rdi+0x1504],xmm1` (island 1073). These corrupt whatever the current layout
  keeps at those offsets. This is state corruption, not merely wrong physics.
* **Wrong pointers handed onward** — `lea rbx,[rdi+0x1378]` twice and `lea
  rbx,[rdi+0x18d0]` once produce the address of a wrong sub-object, which is
  then used and passed to callees; the blast radius is whatever those callees
  write.
* **Wrong reads** — the remaining ten silently feed wrong values into the force
  path, which is exactly the failure that would masquerade as "historical
  physics".

A further 10 sites on `rax` and `rdx` show the same signature but those
registers are redefined many times in the region, so object identity is not
provable without the January binary. They are recorded, not repaired.

Note the shape of the omissions: at `0x1378` every remapped site is `lea
rcx/r11,…` and both missed sites are `lea rbx,…`; at `0x1758` every remapped
site is a `mov` store and four of the five missed are `cmp`. The evidence that
drove the rewrite was produced by instruction alignment against the current
image, and it dropped the sites whose shape did not pair.

## Defect 2 — no behavior gate, and a release check that cannot fail

`Profiles.as` registers January with `Selectable = true`.
`Main.as::InstallJanuary2022` gates only on the Experimental toggle, the build
check, the manifest length check and the 41-byte entry preimage; it then
installs and reports *"January 2022 static-complete preview active; behavior
certification pending"*.

`InstallFall2022` opens with `if (!PROFILE_FALL2022_BEHAVIOR_CERTIFIED) return
false;`. Fall was closed because its "booster trajectory matches stock current
on every measured longitudinal event" — and the shipped Fall control table shows
why that verdict was right: stock, graph-only and V5 agree to six significant
figures on every channel, differing only in `finish_plane_kmh` (345.488 /
345.437 / 345.408). January has never been put through that instrument, and
nothing stops it installing.

`verify_release.rs` asserts the literal strings
`PROFILE_JAN2022_FIELD_RELOCATION_COUNT = 161`, `…CALL_RELOCATION_COUNT = 105`,
`…RIP_RELOCATION_COUNT = 83`. It re-asserts the generator's own output, so it
passes while the payload is incomplete. It is not an independent check.

## Defect 3 — the initializer shadow is a zero block

The removed helper's model block is a `0x7a4`-byte allocation that is **all
zero** except four copied January values (`5.0`, `5.0`, `25.0`, disabled). Every
other field the helper reads from that block therefore reads `0.0`, not the
January value. The record claims four values were "initialized by January
executable code" — it does not establish that the helper reads only those four.
Without the January binary, the rest of the block is unaudited, and zero is a
guess, not a measurement.

## Structural cause

The Fall remapper carries explicit accounting for exactly these classes:
`PROFILE_FALL2022_AUDITED_IMMEDIATE_COUNT = 15`,
`REWRITTEN_IMMEDIATE_COUNT = 9`, `PROVEN_UNCHANGED_IMMEDIATE_COUNT = 6`,
`PROVEN_UNCHANGED_MODRM_COUNT = 9`, `ABI_ADAPTER_COUNT = 2`.

`januarygen.rs` has none of them. It has no immediate audit, no
proven-unchanged ModRM accounting, and no exhaustive per-instruction sweep; its
helper field remaps are a hand-written 27-entry table plus an 11-entry
`root_extra_fields` table, and its `find_disp` **silently increments
`field_failed` and drops the row** whenever a displacement's bytes are not
unique in the instruction. That counter is printed to stderr and is not carried
into the payload, not checked by the shipped verifier, and not visible to the
release gate.

**The Fall fixes were never back-ported to January.** January is not a newer
profile that regressed; it is an older one that never received the corrections.

## Corrected payload

`corrected/Profile_Jan2022.as`, SHA-256
`a635f04a6fbbfc5a1fb2828697d8ce1792d695c06af1a92410b356e5d7500d87`.

* 16 omitted remaps repaired — only where the base register is provably one
  object and the target offset is unambiguous. 32 bytes changed; island size
  unchanged at 12,876.
* Every repair differentially re-verified: 16/16 decode with identical
  mnemonic, length and operand shape, differing only in the displacement.
* Field relocations 161 → 177. All 177 re-pass the alias test.
* 10 residual omissions recorded and deliberately **not** patched: repairing a
  site whose base object is unproven could install a new wrong access.
* Emitted with `PROFILE_JAN2022_BEHAVIOR_CERTIFIED = false`,
  `PROFILE_JAN2022_STATIC_COMPLETE = false`,
  `PROFILE_JAN2022_AUDIT_REPAIRED_OMISSIONS = 16`,
  `PROFILE_JAN2022_AUDIT_RESIDUAL_OMISSIONS = 5` (groups).

**This payload is not selectable and not certified.** It is strictly less wrong
than the shipped one and is the right base for the next iteration; it is not a
release candidate. Ten known-wrong accesses remain in it.

## Required source changes (`gate-patch.md`)

Apply before anything else: January must stop being installable.
