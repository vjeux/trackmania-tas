enum PhysicsProfileId {
    StadiumJanuary2022 = 0,
    StadiumSpring2022 = 1,
    StadiumFall2022 = 2,
    StadiumSummer2023Current = 3,
    SnowRelease2023 = 4,
    SnowJanuary2024 = 5,
    SnowFebruary2024 = 6,
    SnowMay2024Current = 7,
    RallyRelease2024 = 8,
    RallyApril2024 = 9,
    RallyMay2024Current = 10,
    DesertMay2024Current = 11,
}

class PhysicsProfile {
    PhysicsProfileId Id;
    string Name;
    string Period;
    string Representative;
    string Mechanism;
    string Evidence;
    bool Selectable;
    bool NativeIsland;

    PhysicsProfile(PhysicsProfileId id, const string &in name, const string &in period,
                   const string &in representative, const string &in mechanism,
                   const string &in evidence, bool selectable, bool nativeIsland) {
        Id = id;
        Name = name;
        Period = period;
        Representative = representative;
        Mechanism = mechanism;
        Evidence = evidence;
        Selectable = selectable;
        NativeIsland = nativeIsland;
    }
}

array<PhysicsProfile@> BuildProfileCatalog() {
    array<PhysicsProfile@> profiles;
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::StadiumJanuary2022,
        "Stadium — January 2022", "representative pre-2022-03-29 profile",
        "Client build 105899, 2022-01-21; dynamic boundary anchor: server build 112349, 2022-03-25.",
        "Executable-side January island retained for audit only; direct installation is fail-closed.",
        "Independent audit found 16 provably wrong current-layout accesses, 10 more unresolved accesses, and two unresolved ABI risks. Exact January executable/disassembly is required before correction and matched trajectory testing.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::StadiumSpring2022,
        "Stadium — Spring 2022", "2022-03-29 through 2022-09-30",
        "Dynamic source build 112449; practical client source build 115078, 2022-09-30.",
        "No faithful client payload is available: the public full-client archive has no snapshot in 2022-03-29..2022-09-20.",
        "March 29 through June 21 servers reproduce 63.546 exactly, but the Sep. 21/Sep. 30 client is already Fall-staged and is a deterministic HPLTRC3 negative. Spring remains fail-closed pending an external/private Apr–Aug full client.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::StadiumFall2022,
        "Stadium — Fall 2022", "staged by the 2022-09-21 executable; public update 2022-10-01 through 2023-06-22",
        "Client build 115078 in both the 2022-09-30 and 2022-10-06 full profiles.",
        "Measured 9,916-byte V5 native island from the Sep. 21 Fall-staged client; disabled after matched historical mismatch.",
        "Three-way matched map proves V5 is farther from exact Sep. 30 than stock current: mean position error 7.135 m for V5 versus 6.109 m for stock, with >1 m divergence at 3.700 s versus 3.840 s. V5 remains fail-closed; no V6 was created.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::StadiumSummer2023Current,
        "Stadium — Summer 2023 / Current", "2023-06-23 onward",
        "Earliest staged source build 121457, 2023-06-23; this option uses the installed supported client.",
        "Current installed code/data. The 2023 causal split is unresolved because both the handler and tracked packs changed.",
        "June 23 and July 10 share handler and tracked pack bytes. No later Stadium force-law change is behaviorally confirmed.",
        true, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::SnowRelease2023,
        "Snow — release", "2023-11-21 through 2024-01-08",
        "SnowCar public-release behavior.",
        "Current-build compatibility uses three preimage-gated delayed-setter no-op sites, the pre-Feb input path, and an in-memory Snow CPlugSurface sphere transaction.",
        "Baseline code/data delta before the January delayed-player fix, February hitbox/action-key changes, and May analog-input fix. Patch transactions are live-verified, but no matched release-client trajectory exists; fail-closed.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::SnowJanuary2024,
        "Snow — January 2024", "2024-01-09 through 2024-02-26",
        "SnowCar with delayed-player scripting fixes.",
        "Current-build compatibility uses the pre-Feb input path plus an in-memory Snow CPlugSurface sphere transaction.",
        "Nadeo confirmed three SetPlayer_Delayed_ functions were fixed for SnowCar; collision/code transactions are live-verified but historical behavior lacks a matched January-client trajectory, so this profile is fail-closed.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::SnowFebruary2024,
        "Snow — February 2024", "2024-02-27 through 2024-05-21",
        "SnowCar with re-ranged action keys and improved hitbox.",
        "Exact-build input rollback: restore the pre-May Xi smoothing path while retaining February action-key routing and installed collision data.",
        "The collision geometry delta is measured exactly and the May input boundary is localized, but no matched February-client trajectory has certified their combined behavior; fail-closed.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::SnowMay2024Current,
        "Snow — May 2024 / Current", "2024-05-22 onward",
        "Current installed SnowCar behavior.",
        "Provided by the installed supported game; choose CarSnow in Official vehicle family.",
        "Includes the global analog smooth-steering 100% fix. No later Snow behavior change is confirmed.",
        true, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::RallyRelease2024,
        "Rally — release", "2024-02-27 through 2024-04-01",
        "Client build 126849, 2024-02-26 release payload.",
        "Exact 3,056-byte release CarRally item override, plus the pre-May smooth-steering input path.",
        "The release and March 19 items are identical; the exact item/input transactions pass live integration tests, but no matched release-client trajectory certifies behavior, so this profile is fail-closed.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::RallyApril2024,
        "Rally — post custom-ice fix", "2024-04-02 through 2024-05-21",
        "First archived post-fix payload: client build 127010, 2024-04-30.",
        "Uses the supported current CarRally item FID while suppressing only the later shared analog snap-to-target store.",
        "The encrypted Rally physics-model payload is bit-identical across the custom-ice boundary and the adjacent item path delta is preserved. Integration is live-verified, but no matched April-client trajectory certifies behavior; fail-closed.",
        false, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::RallyMay2024Current,
        "Rally — May 2024 / Current", "2024-05-22 onward",
        "Current installed RallyCar behavior.",
        "Uses the corrected CarRally item path and installed smooth-steering input code.",
        "Includes the global analog smooth-steering 100% fix. No later Rally behavior change is confirmed.",
        true, false));
    profiles.InsertLast(PhysicsProfile(PhysicsProfileId::DesertMay2024Current,
        "Desert — release / Current", "2024-05-22 onward",
        "Current installed DesertCar behavior.",
        "Uses installed DesertCar data and installed smooth-steering input code; choose CarDesert when authoring the map.",
        "No confirmed post-release Desert driving/physics change through 2026.",
        true, false));
    return profiles;
}
