#!/bin/bash
V=~/persistent/private-30d/tm-player/tiny/video-audit-20260910/argentina-6b1b790f
O=~/persistent/private-30d/tm-route/gen/sections/21
ts=${1:-20260910T2035Z}
# (arg-int.txt prepared by the caller)
{
echo "# SECTIONS — map 21 Argentina — DEFINITIVE CONTRACT SET (GEOM, $ts) — supersedes SPEC-20260910T1815Z.md"
echo "Source of truth: tm-player/tiny/video-audit-20260910/argentina-6b1b790f/ (FINAL-HANDOFF.md, INVENTORY-111.md: 111 ids = 41 hypotheses, 58 protected, 12 retracted; OPENING-FIRST-BRIEF.md). This file maps every id to a GEOM section (candidate C / guard G) and carries the geometry. Where anything differs, the audit files win."
echo
echo "## Contract (from FINAL-HANDOFF): OPENING FIRST — INPUT verified that 117.791 and 115.478 share inputs through 27.670 s (first change 27.680; position/speed identical through 26.000 s); 115.147 is NOT INPUT-certified and carries NO prefix-identity proof (any 115.147 claim is unverified-to-audit). The opening ids therefore apply to 117.791 and 115.478; a B0 lane with a CHANGED prefix must establish its own applicability, and F830 (27.670 → 27.680) straddles the first changed control of 115.478 — post-boundary equality is not established. Fork rule: prefix frozen through anchor tick K inclusive, fixture pre-K+1, first editable K+1 or later (INPUT mints the state). ONE actual-exit refit after the opening is won. Exact negatives already known: C1 (brake onset 3.99 -> 4.01) -> G1 10.23 vs 9.34; C2 (steer 4.99 -> 4.97) -> G1 9.38, no G2/G3 — fixed-suffix edits fail; F210's brake-only 7.99 -> 8.01 is unrun. Export phase: positions align at label − 0.010 s; the old yaw_deg column = velocity bearing (body yaw now separate). F210 = a REVERSE-TO-FORWARD rotation (backwards only 7.000–7.720 s; forward from 7.730) — not a uniformly backwards slide."
echo
echo "## Retractions (12) — guards, no work: the lag-shift (camera lag, not a tape offset), F81 barrel strike, F94 bank crash (the bank ENTRY is protected), F759 right-lip clearance, F786 left-ramp entry and the others marked retracted below. Protected findings (58) are constraints and refit sensitivities, not removal targets."
echo
echo "## Id -> section map (all 111). Interval = the UNION of the frame ranges the audit block itself cites, divided by 30 fps (frames are the audit'"'"'s own columns; nothing hand-copied); a zero-length interval = the id'"'"'s frame anchor / 30 where the block cites no range. Telemetry/control windows inside the audit entries (e.g. F830 27.670–27.680 AND 27.790–27.800; F1482 49.400–49.420 AND 49.580–49.590; F1544 edit window; F1770 59.990–60.140 second pulse; F3131 104.350–108.350 → 111.367; F3246 108.190–108.220 arrest) are canonical and are NOT re-stated here — this map is a section crosswalk, not the range source."
echo "| id | disposition | interval (s) | GEOM section | role | geometry note |"
echo "|---|---|---|---|---|---|"
awk -F'\t' '{split($1,a," — "); id=a[1]; disp=a[2]; iv=$2; s=iv; gsub(/≈ /,"",s); sub(/ \(.*/,"",s); sub(/\.\..*/,"",s); s=s+0; sec="—"; note=""; if (iv=="?") s=-1;
 if (s<0) sec="(whole-lap provenance)";
 else if (s<9.5) {sec="OPENING S1 (spawn -> wp0): launch, turbo transfer, landing pitch, bank entry, the banked hairpin wall-cycle, flat yaw"; note="A1–A3: gentle start (author 36 m/s at 3.2 s vs base 58 at 2.3), hairpin at <= 48 on the author arc (base 56 -> 25 on the bank)"}
 else if (s<19.0) {sec="OPENING S1/S2 (wp0 -> wp7): coast, the north road edges, the dip exit"; note="road x 1515–1526 with the stepped METAL bank east (x 1528/1532/1536 at 41.5/43.5/45.5); dip exit (1486, 36.7, 656) heading −42 at 55"}
 else if (s<27.7) {sec="OPENING S2 (wp7 -> top turn -> wp17): the rubber-curb divider, the west run, the wp17 climb"; note="divider = the RUBBER CURB line at z 777 (x 1430–1438) crossed at <= 28 heading −70…−100 after braking 45 -> 29 by z 767; never x > 1446 for z > 776"}
 else if (s<43.3) {sec="S3 (wp17 -> wp2 -> wp1): deck turbo, the north-edge drop, the bowl wall"; note="turbo line x 1318 north; drop off the north edge at (1330, 60.5, 848); the wall up to wp1"}
 else if (s<62.4) {sec="S5 (wp1/wp8 -> wp11 -> wp15): deck drop, hump, dive, slab"; note="crest (1268, 40.1, 688) >= 40 heading 153; the ResonantMetal slab seam (1287, 29, 650)"}
 else if (s<71.1) {sec="S6 (wp15 -> wp9 -> wp12): ice climb"; note="RoadIce 14 -> 44; author 56 in, ~1 m/s out"}
 else if (s<95.3) {sec="S7 (wp12 -> wp13 -> wp3)"; note="no geometry read yet"}
 else if (s<103.5) {sec="S8 (wp3 -> wp4 -> wp16)"}
 else {sec="S9 (wp16 -> wp14 -> wp10 -> wp5): the 240 m run + turbo climb"; note="14° climb north, turbo at z 1080–1100, gas on"};
 role=(disp=="hypothesis")?"C":"G"; printf "| %s | %s | %s | %s | %s | %s |\n", id, disp, iv, sec, role, note}' /tmp/arg-int.txt
echo
echo "## OPENING contract set for GEN's B0 lanes: F78 (landing pitch), F114 (wall cycle), F210 (reverse-to-forward yaw), F270 (coast), F318 (left-edge loss), F357 (right-edge loss), F447 (boundary loss), F642 (divider pivot), F830 (the wp17 climb region) — with F54/F94 (turbo transfer, bank entry), F193, F260 (G1 exit), F477/F558 (recovery, G2) and the ring transfer F813 as guards. The rest of the map is GUARDS until the opening is won."
echo
echo "## Section specs (geometry, forks, probes) — carried from SPEC-20260910T1815Z.md"
sed -n '/^## S1 \|^# OPENING ENVELOPE/,$p' $O/SPEC-20260910T1815Z.md | sed 's/^# /## /'
} > $O/SPEC-$ts.md
wc -l $O/SPEC-$ts.md
grep -c "^| A21" $O/SPEC-$ts.md
awk -F'|' '/^\| A21/ {print $6}' $O/SPEC-$ts.md | sort | uniq -c
grep -c "?" /tmp/arg-int.txt
md5sum $O/SPEC-$ts.md | cut -c1-12
