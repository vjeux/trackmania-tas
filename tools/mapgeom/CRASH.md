# `mapgeom crash` — reading a Trackmania client crash dump

When the client dies loading one of our items, Windows Error Reporting leaves
`C:\Users\vjeux\AppData\Local\CrashDumps\Trackmania.exe.<pid>.dmp` (~48 MB) on
the render box. Together with the game exe (no symbols; VMProtect scrambled the
`.pdata` function table, so nothing standard unwinds it) that is all the
evidence. `mapgeom crash` reads both with the standard library only and shells
out to `objdump` (binutils ≥ 2.37, `pei-x86-64`) for disassembly.

```
mapgeom crash DUMP.dmp [--exe Trackmania.exe] [options]
    (default)            dump summary, exception + registers, faulting instruction, stack walk
    --read ADDR LEN      hex dump LEN (hex) bytes at ADDR
    --find PATTERN       search all captured memory: ASCII string (also tried as UTF-16),
                         `hex:DEADBEEF`, or `wide:text`; lists who points at each hit
    --disasm ADDR [N]    N instructions (default 16) around ADDR via objdump on --exe
    --ranges [N]         the N largest captured memory ranges, labelled
    --all-frames         also list the stale return addresses the chain skipped
    --no-stack           skip the stack walk
    --quiet              skip the module / thread listing
    --limit N            cap --find hits (default 20)
```

`--exe` defaults to `Trackmania.exe` next to the dump, then
`/tmp/tmexe/Trackmania.exe`. The exe on disk is checked against the module in
the dump (PE timestamp + SizeOfImage) and a mismatch is warned about — RVAs from
a different build mean nothing.

## Addresses

The exe is loaded at a random base (ASLR; `0x7ff679100000` in e5.dmp) but
objdump prints on-disk addresses (ImageBase `0x140000000`). Every exe address is
printed three ways:

```
0x00007ff679556c35  Trackmania.exe+0x456c35 (objdump 0x140456c35)
```

`ADDR` arguments accept any of the three forms: a loaded VA, an objdump address
(anything inside the on-disk image range that the dump itself does not map is
slid to the loaded base), and for `--disasm` a bare RVA. Frames in the
VMProtect sections are tagged `[.A2U]` / `[.D."]` — virtualised code, do not
expect readable disassembly there.

## What it reads

Streams: ThreadList (3), ModuleList (4), MemoryList (5), Exception (6),
SystemInfo (7), Memory64List (9), MemoryInfoList (16), ThreadInfoList (17).
The exception record gives code, faulting address, read/write and the data
address; the AMD64 CONTEXT gives the 16 GPRs + rip, eflags, mxcsr and xmm0–15
(shown as four f32 lanes). Registers that point into the dump are dereferenced;
small values are also shown in decimal (a loop counter is easier to recognise as
`71` than as `0x47`).

## The stack walk (no unwind info)

1. Every 8-byte slot from rsp to the top of the thread's stack whose value lands
   in code and is immediately preceded by a `call` (E8 rel32, or the FF /2
   forms: `call reg`, `call [reg+disp]`, `call [rip+disp32]`, `call [rsp+disp]`)
   is a candidate return address. For the exe the call bytes are read from the
   exe on disk, so it works even though the dump does not carry the code pages.
2. Candidates are chained: starting from rip, the first candidate whose
   *direct* callee owns the current address is the real caller frame; then from
   its return address, and so on. "Owns" means: the address lies after the
   callee's start and no `int3 int3` padding — which MSVC emits only between
   functions — lies between the two in the exe's bytes. That is how the tool
   knows `0x140456c35` belongs to the function at `0x1404559f0` (5 KB earlier)
   and not to a nearer stale frame.
3. When no direct call owns the address the caller used a virtual/indirect call
   whose target is unknowable. The tool then takes the indirect candidate whose
   own return address a later direct frame vouches for (`v`), and shows the
   indirect candidates it skipped on the way as `#-?` — possibly real, not
   provable.
4. The faulting function's prologue is parsed (pushes, `lea rbp,[rsp±N]`, `sub
   rsp,N` or `mov eax,N; call __chkstk; sub rsp,rax`) and cross-checked: the
   return slot it predicts must be frame #1 (frame size confirmed), and the rbp
   it predicts is compared with the context's rbp.

Every frame prints `[in fn X +off]`: the function it executes in (the callee
of the frame above it) — a function start address to disassemble, in a binary
with no function table.

## e5.dmp, read with the tool (2026-09-06)

Exception `0xc0000005` read from `0x0` at `Trackmania.exe+0x456c35`
(`0x140456c35`), thread `0x8728`:

```
140456c0a:  mov    ecx,r10d                       ; r10 = vertex index = 0
140456c0d:  imul   ecx,DWORD PTR [rbp+0xd0]       ; * element stride
140456c14:  add    rcx,QWORD PTR [rbp+0xc8]       ; + element data pointer  -> NULL
140456c1b:  cmp    DWORD PTR [rbp+0xd8],0x2       ; element type 2 = Float3 ?
140456c22:  jne    0x140456c35
   ...
140456c35:  mov    ecx,DWORD PTR [rcx]            ; <== packed (DEC3N) read, rcx = 0
140456c37:  shl/sar 0x16, shl 0xc / sar 0x16, lea *4 / sar 0x16   ; 3 × 10-bit signed fields
140456c68:  divss  xmm8,xmm10                     ; / 511 -> unit vector
```

Registers: `rcx=0`, `r10=0` (vertex index), `r12=0x47` = **71 vertices**
(`mov r12d,[rbp-0x10]` at `0x140456b75`), `r15=0x20736004a0c`,
`r11 = r15+0x14`, `r13=0x206d519cad0` (arg 1, the object whose `+0xb8` flags
are tested), `rsi=0x206d519caf0`.

Verified chain (frames #0–#10 all direct-call verified):

```
#0  rip           Trackmania.exe+0x456c35   in fn 0x1404559f0 +0x1245   per-vertex element decode loop
#1  [rsp+0x1728]  Trackmania.exe+0x458050   in fn 0x140457fb0 +0xa0     (0x140457fb0 calls 0x1404559f0 at 0x14045804b)
#2  [rsp+0x1818]  Trackmania.exe+0x221363   in fn 0x140221280 +0xe3
#3  [rsp+0x18d8]  Trackmania.exe+0x222efb   in fn 0x140222cb0 +0x24b
#4  [rsp+0x1c48]  Trackmania.exe+0x2201d4   in fn 0x14021e340 +0x1e94
#5  [rsp+0x1f98]  Trackmania.exe+0x21dfb6   in fn 0x14021db10 +0x4a6
#6  [rsp+0x2068]  Trackmania.exe+0x21b33d   in fn 0x14021a9b0 +0x98d
#7  [rsp+0x2248]  Trackmania.exe+0xc54781   in fn 0x140c53c70 +0xb11
#8  [rsp+0x24f8]  Trackmania.exe+0xc53a3c   in fn 0x140c53900 +0x13c
#9  [rsp+0x25a8]  Trackmania.exe+0xc526e2   in fn 0x140c525c0 +0x122
#10 [rsp+0x2618]  Trackmania.exe+0xea7167   (caller reached through a virtual call)
#11 v [rsp+0x3ff8] Trackmania.exe+0x2f563a  in fn 0x1402f5530 +0x10a
#12   [rsp+0x4038] Trackmania.exe+0x2d3565  in fn 0x1402d3420 +0x145
#13   [rsp+0x4088] Trackmania.exe+0x8f77ce  in fn 0x1408f7730 +0x9e
#14   [rsp+0x40b8] Trackmania.exe+0x103169
#15 v [rsp+0x40e8] Trackmania.exe+0xaa53b0  in fn 0x140aa5370 +0x40
#16   [rsp+0x4128] Trackmania.exe+0xaa8b67
```

Prologue of `0x1404559f0`: 5 pushes, `sub rsp,0x1700` via `__chkstk`,
`rbp = rsp_after_pushes - 0x1600`. Return slot `rsp+0x1728` == frame #1 —
frame size confirmed. **The context's rbp (`0x50486fb260`) is 0x150 below the
frame pointer the code used (`0x50486fb3b0`)**; the locals only make sense at
the reconstructed value (`[rbp-0x10] = 0x47 = r12`, `[rbp-0x30] = r15`). Read
`[rbp+X]` at `0x50486fb3b0+X`, not at the context's rbp.

What the loop was decoding — three `{data ptr, stride, type}` triples the
function fetched from the vertex stream through its vtable slot `+0x128`
(`GetElement(out, semantic, 0)`), at `0x140456453`, `0x1404564b4`,
`0x1404564ce`:

| local | semantic | in memory (`--read 0x50486fb3e0 0x100`) | meaning |
|---|---|---|---|
| `[rbp+0x30]` | 5 = Normal | ptr `0x206d519cae0`, stride `0x30`, type `0xe` (DEC3N) | present; vertex stride 48 B |
| `[rbp+0xc8]` | 0x12 = TangentU | ptr **NULL**, stride 1, type 1 | **missing** → the crash |
| `[rbp+0xf8]` | 0x14 = TangentV | ptr **NULL**, stride 5, type 4 (defaults written at `0x140456236`) | missing |

The tangent lookups run only when bit 1 of `[r13+0xb8]` is clear
(`mov edi,[r13+0xb8]` at `0x140455eae`; `test dil,2` at `0x14045649a`), and
the loop dereferences the results without a null check. So: **the visual being
processed (71 vertices, stride 48, normals as DEC3N) had a vertex declaration
without TangentU/TangentV while the owning object's flags said tangents were
expected** — exactly the "null vertex element pointer" of commit bf9981d
(DecoBeachMangrove split, minimal repro two TransitionToLand visuals of 71 + 29
vertices; 71 is the visual it died on). The fix is on our side: either every
split sub-visual carries the tangent elements of its parent (semantics 18 and
20, DEC3N), or the flag bit that declares "no tangents" is set on the object.
The consumer is fn `0x1404559f0`, called from `0x140457fb0 +0x9b`, which in
turn is called from the loader path `0x140221280 → 0x140222cb0 → 0x14021e340 →
0x14021db10 → 0x14021a9b0 → 0x140c53c70 → 0x140c53900 → 0x140c525c0`.

### Strings — what this dump does *not* have

`--find TransitionToLand`, `--find AC00000008`, `--find Mangrove` all return
zero hits (ASCII and UTF-16). This is not a search bug (`--find hex:<return
address bytes>` finds the stack slot; `--find Summer` finds a heap SSO string
with its own self-pointer) — the dump simply does not contain the heap:

```
captured memory: 5141 ranges, 45 MiB
  inside modules 41208 KiB, thread stacks 285 KiB, other (heap windows, data) 5231 KiB
```

That is WER's default dump type: module data segments, thread stacks, and
small windows of memory referenced from the stacks. The item's name, its
material links and the parsed visual structures (`r13`, `r15`, `rsi` all
"not mapped in dump") live on the heap and were not captured. To make the next
dump answer "which item / which material" directly, set on the render box:

```
HKLM\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps\Trackmania.exe
    DumpType  (DWORD) = 2        ; full memory dump (~ several GB)
    DumpFolder        = C:\Users\vjeux\AppData\Local\CrashDumps
```

With a full dump `--find` locates the strings and the reverse-pointer scan
prints every 8-byte slot holding their address — the string objects, and from
there (`--read`) the structures that own them.

## Tips

- `--disasm 0x1404559f0 20` — objdump addresses straight from the walk.
- `--read 0x50486fb3b0+X …` — qwords in a hex dump that point into the exe are
  annotated (`q3: Trackmania.exe+…`), and ones that point into captured memory
  get `->` — a fast way to spot return addresses and object pointers in a frame.
- Function start for any exe address: the walk's `[in fn …]`, or run
  `--disasm` at the address and look backwards for `ret; int3`.
- Function boundaries: `int3 int3` padding separates MSVC functions. Two
  consecutive `0xCC` inside a function body are rare but not impossible
  (`--all-frames` shows what the chain rejected if a walk looks short).
