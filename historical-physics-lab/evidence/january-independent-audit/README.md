# January 2022 profile — independent audit bundle

Verdict, findings and hashes: AUDIT.md
Source changes required to disable January: gate-patch.md
Live-map test plan for WhiteStick: TESTPLAN.md

tools/      five std-only Rust verifiers, independent of the generator
corrected/  corrected fail-closed payload (NOT selectable, NOT certified)
evidence/   captured output of every tool, plus the shipped island and manifest

Reproduce (rustc 1.94.1, objdump 2.37):
  rustc --edition=2021 -O tools/janverify.rs -o janverify
  ./janverify Profile_Jan2022.as Trackmania-current-whitestick.exe --emit-dir emit
  rustc --edition=2021 -O tools/janasm.rs -o janasm
  ./janasm emit emit/island-patched.bin emit/island-unpatched.bin
