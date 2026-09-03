# January 2022 Stadium native profile — static implementation record

## Scope and certification status

This package implements a build-128130 native preview of the January 2022
Stadium profile. The payload is **statically complete for the copied native
regions and their field, direct-call, and RIP-relative references**. It has not
been behavior-certified in the 2026 client, because there is not yet an
authoritative deterministic client-side input replay. A crash-free run or ghost
playback would not establish semantic equivalence.

The authoritative historical boundary control was rerun separately:

- 2022-03-25 server build 112349: `WRONG_SIMU`, validated time `-1`.
- 2022-03-29 server build 112449: valid, exact time `63.546`.

That control proves the historical behavior boundary follows executable code.
It does **not** prove this client payload reproduces the January trajectory.

## Inputs

| Role | Build / date | SHA-256 | Relevant root |
|---|---|---|---|
| January source | 105899 / 2022-01-21 | `e2255c415f0f7fc2d0a66512fa7609256c42cf639a5380b7a5bcdbb4486ab75b` | `0x1405EDEB0..0x1405EFF40` |
| Spring comparison | 115078 / 2022-09-30 | `1f5ce9877e327d690cc9d6b5ab02fbe6157452e85e15e622ce46deeac91fe1be` | `0x1406F2030..0x1406F4068` |
| Supported target | 128130 / 2026-01-28 | `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda` | `0x140851F00..0x1408540E0` |

The build gate verifies the unique 41-byte target handler entry and the unique
build banner before any native write.

## Why the top-level handler is insufficient

The January root differs materially from Spring and current, but its first
call also enters a separate state initializer:

- January: `0x1405EDE00..0x1405EDEAC`
- Spring: `0x1406F1F80..0x1406F202C`
- current: `0x14083DE60..0x14083DF0C`

January additionally reaches native code that disappeared or was inlined later:

- helper `0x1405EDCF0..0x1405EDE00`;
- averaging helper `0x1405E7730..0x1405E77F2`;
- nine legacy curve wrappers in `0x1413BE660..0x1413BEAD5`, used by ten root
  call sites.

The legacy wrappers use an output-pointer interpolation ABI. The generated
island preserves each wrapper's field selection and unit conversion and routes
its old call through a measured adapter to the current scalar interpolation
helper.

The removed helper also reads four fields initialized by January executable
code at `0x1413B4D43`, `0x1413B4D4D`, `0x1413B4D57`, and `0x1413B4DA6`:

| Meaning in copied helper | January bits | Value |
|---|---:|---:|
| clamp / maximum | `0x40A00000` | `5.0` |
| step | `0x40A00000` | `5.0` |
| threshold | `0x41C80000` | `25.0` |
| enable flag | `0x00000000` | disabled |

Current initializes the corresponding fields differently. The island therefore
carries a private January shadow of these values and rewrites the removed
helper's first load to that shadow. It does not mutate current model state.

## Generated payload

- bytes: **12,876**
- copied source regions: **14** (one legacy wrapper has two call-site-specific copies)
- structure-field relocations: **161**
- direct-call relocations: **105**
- RIP-relative relocations: **83**
- absolute current-image thunk targets: **41**
- unresolved direct calls: **0**
- unresolved RIP-relative references: **0**
- generated compatibility adapters: **1**
- copied executable initialization values: **4**

`Profile_Jan2022.as` contains separate machine-readable manifests for fields,
calls, RIP references, source regions, initialization provenance, and absolute
current-image targets, plus the combined runtime relocation arrays.

## Static verification

`tools/january_verify.rs` independently checks:

1. the unique build-128130 banner and 41-byte handler signature;
2. payload size and entry-prologue compatibility;
3. every field relocation's source instruction, operand preimage, width, and
   patched value;
4. direct and composed January→Spring→current field-map coverage;
5. complete direct-call and RIP-reference coverage over every copied region;
6. initialization-shadow values and source provenance;
7. relocation-array consistency, bounds, rel32 reachability, and absolute-thunk
   target validity in the current image;
8. a fully relocated synthetic image, emitted as
   `Profile_Jan2022.as.verified.bin` for independent disassembly.

The generic `islandverify.rs` provides a second relocation replay. The release
verifier checks fail-closed selection, no automatic installation, and the
certification disclaimer.

## Reproduction commands

All custom tools are Rust and use only the standard library.

```text
rustc --edition=2021 -O tools/januarygen.rs -o januarygen
rustc --edition=2021 -O tools/january_verify.rs -o january_verify
rustc --edition=2021 -O tools/control_verify.rs -o control_verify

january_verify Profile_Jan2022.as JAN.exe CURRENT.exe JAN.objdump \
  january-vs-current-fields.tsv january-via-spring-fields.tsv
control_verify control-pre.stdout.txt control-post.stdout.txt
```
