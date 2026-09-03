use std::env;
use std::fs;

fn must(haystack: &str, needle: &str, what: &str) {
    assert!(haystack.contains(needle), "missing {what}: {needle}");
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn main() {
    let root = env::args().nth(1).expect("package source directory");
    let main_as = fs::read_to_string(format!("{root}/Main.as")).unwrap();
    let profiles = fs::read_to_string(format!("{root}/Profiles.as")).unwrap();
    let january = fs::read_to_string(format!("{root}/Profile_Jan2022.as")).unwrap();
    let families = fs::read_to_string(format!("{root}/VehicleFamilies.as")).unwrap();
    let rally = fs::read_to_string(format!("{root}/RallyProfiles.as")).unwrap();
    let rally_overlay = fs::read_to_string(format!("{root}/tools/rally_item_overlay.rs")).unwrap();
    let analog = fs::read_to_string(format!("{root}/Profile_RallyAnalog.as")).unwrap();
    let snow = fs::read_to_string(format!("{root}/SnowPatches.as")).unwrap();
    let collision = fs::read(format!("{root}/payloads/snow/pre-feb/SnowCar.Shape.Gbx")).unwrap();
    let data = fs::read_to_string(format!("{root}/Profile_Fall2022.as")).unwrap();
    let catalog = fs::read_to_string(format!("{root}/profiles.json")).unwrap();
    let manifest = fs::read_to_string(format!("{root}/payloads/manifest.tsv")).unwrap();
    let rally_release =
        fs::read(format!("{root}/payloads/rally-release/CarRally.Item.Gbx")).unwrap();
    let rally_post = fs::read(format!(
        "{root}/payloads/rally-post-custom-ice/CarRally.Item.Gbx"
    ))
    .unwrap();
    let info = fs::read_to_string(format!("{root}/info.toml")).unwrap();

    for (name, source) in [
        ("Main.as", &main_as),
        ("Profiles.as", &profiles),
        ("Profile_Jan2022.as", &january),
        ("VehicleFamilies.as", &families),
        ("RallyProfiles.as", &rally),
        ("Profile_RallyAnalog.as", &analog),
        ("SnowPatches.as", &snow),
    ] {
        assert_eq!(
            source.matches('{').count(),
            source.matches('}').count(),
            "unbalanced braces in {name}"
        );
    }

    assert_eq!(
        profiles
            .matches("profiles.InsertLast(PhysicsProfile(")
            .count(),
        12,
        "the UI must expose every behaviorally confirmed official profile: four Stadium, four Snow, three Rally, one Desert"
    );
    assert_eq!(profiles.matches("false, false));").count(), 8,
        "every historical profile must remain catalog-only until behavior-certified");
    assert_eq!(profiles.matches("true, false));").count(), 4,
        "only four installed-current profiles may be selectable");
    assert!(!profiles.contains("true, true));"),
        "no native historical profile may be selectable before certification");
    for id in [
        "StadiumJanuary2022",
        "StadiumSpring2022",
        "StadiumFall2022",
        "StadiumSummer2023Current",
        "SnowRelease2023",
        "SnowJanuary2024",
        "SnowFebruary2024",
        "SnowMay2024Current",
        "RallyRelease2024",
        "RallyApril2024",
        "RallyMay2024Current",
        "DesertMay2024Current",
    ] {
        must(&profiles, id, "profile id");
    }
    for forbidden in ["Snow car", "Rally car", "Desert car"] {
        assert!(
            !profiles.contains(forbidden),
            "{forbidden} is a vehicle family, not a Stadium epoch"
        );
    }

    must(
        &main_as,
        "g_ExperimentalUnlocked = false",
        "per-process activation lock",
    );
    must(
        &main_as,
        "GetApp().CurrentPlayground !is null",
        "playground switch gate",
    );
    must(&main_as, "GetApp().Editor !is null", "editor switch gate");
    must(&main_as, "TARGET_BUILD_BANNER_PATTERN", "build gate");
    must(&main_as, "TARGET_HANDLER_PATTERN", "handler gate");
    must(
        &main_as,
        "g_EntryBackup != TARGET_ENTRY_ORIGINAL",
        "entry preimage check",
    );
    must(
        &main_as,
        "Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != g_EntryJump",
        "owned jump check",
    );
    let startup = main_as
        .split("void Main() {")
        .nth(1)
        .unwrap_or("")
        .split("void RenderMenu()")
        .next()
        .unwrap_or("");
    assert!(!startup.contains("InstallFall2022()"), "release Main must not auto-install Fall physics");
    assert!(!startup.contains("InstallJanuary2022()"), "release Main must not auto-install January physics");
    must(
        &main_as,
        "IsRallyProfile(g_SelectedProfile)",
        "Rally profile dispatcher",
    );
    must(
        &main_as,
        "arm Experimental native profiles before applying historical Rally behavior",
        "historical Rally activation lock",
    );
    must(
        &main_as,
        "ApplyRallyAxes(releasePath, legacyAnalog)",
        "independent Rally axes",
    );
    must(
        &main_as,
        "ApplyRallyAxes(false, false)",
        "current-profile restoration",
    );
    must(
        &main_as,
        "RestoreRallyProfilesOnUnload()",
        "unload rollback",
    );
    must(&main_as, "RenderRallyProfileControls()", "runtime controls");
    must(&main_as, "InitializeSnowPatchSites()", "Snow patch-site initialization");
    must(&main_as, "InstallSnowProfile(g_SelectedProfile)", "Snow profile dispatch");
    must(&main_as, "g_SnowCodeInstalled", "Snow rollback on unload");
    for needle in [
        "SNOW_ACTION_KEY_BRANCH_RVA = 0x2B8C4C",
        "SNOW_SMOOTH_STEERING_STORE_RVA = 0x2C360E",
        "SNOW_DELAYED_ADHERENCE_CALL_RVA = 0x1342927",
        "SNOW_DELAYED_ACCEL_CALL_RVA = 0x1342AB7",
        "SNOW_DELAYED_CONTROL_CALL_RVA = 0x1342C47",
        "PreflightSnowCodeTransaction",
        "RollBackSnowCodeChanges",
        "SNOW_COLLISION_PRE_FEB_SIZE = 1123",
        "SNOW_COLLISION_CURRENT_SIZE = 1151",
        "ValidateSnowCollisionEpoch",
        "SetSnowHistoricalCollision",
        "GmSurfType = EGmSurfType::Sphere",
        "GmSurfType = EGmSurfType::Ellipsoid",
        "RemoveSnowProfilePatches",
    ] {
        must(&snow, needle, "transactional Snow implementation");
    }
    assert_eq!(collision.len(), 1_123, "historical Snow shape size");
    assert_eq!(&collision[..3], b"GBX", "historical Snow shape magic");
    assert_eq!(
        u32::from_le_bytes(collision[9..13].try_into().unwrap()),
        0x0900_C000,
        "historical Snow shape class"
    );

    for needle in [
        "PROFILE_JAN2022_UNRESOLVED_CALLS = 0",
        "PROFILE_JAN2022_UNRESOLVED_RIP = 0",
        "PROFILE_JAN2022_FIELD_RELOCATION_COUNT = 161",
        "PROFILE_JAN2022_CALL_RELOCATION_COUNT = 105",
        "PROFILE_JAN2022_RIP_RELOCATION_COUNT = 83",
        "PROFILE_JAN2022_STATIC_COMPLETE = false",
        "PROFILE_JAN2022_BEHAVIOR_CERTIFIED = false",
        "PROFILE_JAN2022_INIT_SOURCE_VAS",
        "PROFILE_JAN2022_INTERPOLATION_ADAPTER_OFFSET",
    ] {
        must(&january, needle, "January native manifest");
    }
    must(&main_as, "InstallJanuary2022()", "January selector integration");
    must(&main_as, "if (!PROFILE_JAN2022_STATIC_COMPLETE || !PROFILE_JAN2022_BEHAVIOR_CERTIFIED)", "direct January install safety gate");
    must(&snow, "historical Snow profiles are fail-closed until matched old-client trajectory certification", "direct Snow install safety gate");
    for needle in [
        "PROFILE_FALL2022_UNRESOLVED_CALLS = 0",
        "PROFILE_FALL2022_UNRESOLVED_RIP = 0",
        "PROFILE_FALL2022_FIELD_REMAP_COUNT = 44",
        "PROFILE_FALL2022_ABI_ADAPTER_COUNT = 2",
        "PROFILE_FALL2022_RELOCATED_CALL_COUNT = 155",
        "PROFILE_FALL2022_ABS64_THUNK_COUNT = 40",
        "PROFILE_FALL2022_BEHAVIOR_CERTIFIED = false",
    ] {
        must(&data, needle, "Fall V5 native manifest");
    }
    must(&main_as, "if (!PROFILE_FALL2022_BEHAVIOR_CERTIFIED)", "direct Fall install safety gate");
    must(&main_as, "if (!selected.Selectable)", "catalog selector safety gate");
    must(&main_as, "g_SelectedProfile == PhysicsProfileId::StadiumFall2022", "Fall selector integration");
    must(&main_as, "Fall 2022 is fail-closed", "Fall runtime safety gate");
    let fall_dispatch = main_as
        .split("if (g_SelectedProfile == PhysicsProfileId::StadiumFall2022)")
        .nth(1)
        .unwrap_or("")
        .split("if (IsSnowProfile")
        .next()
        .unwrap_or("");
    assert!(!fall_dispatch.contains("InstallFall2022()"),
        "Fall must not dispatch a behaviorally mismatched native island");
    must(&profiles, "Three-way matched map proves V5 is farther from exact Sep. 30 than stock current", "Fall mismatch gate");
    must(&catalog, "fail_closed_behavior_mismatch_proven", "Fall machine-readable mismatch gate");
    must(&main_as, "FALL_EXPECTED_TUNING_NAMES", "measured Fall tuning-name preimage");
    must(&main_as, "ValidateCurrentFallGraph", "Fall runtime graph validator");
    must(&main_as, "FALL_TARGET_COUNT", "Fall tuning-count target");
    must(&main_as, "RestoreFallRuntimeGraph", "transactional Fall runtime rollback");
    assert!(!main_as.contains("g_SelectedProfile == PhysicsProfileId::StadiumSpring2022"),
        "Spring must remain fail-closed until a matching full client exists");
    must(&profiles, "No faithful client payload is available", "Spring archive-gap gate");
    must(&profiles, "Measured 9,916-byte V5 native island", "Fall V5 catalog evidence");

    for needle in [
        "Fids::GetGame",
        "RALLY_ITEM_RELEASE_SIZE = 3056",
        "RALLY_ITEM_CURRENT_SIZE = 2058",
        "ValidateRallyItemPathPreimage",
        "ValidateRallyAnalogSite",
        "transaction rolled back",
        "rally_item_overlay restore/install",
    ] {
        must(&rally, needle, "transactional Rally implementation");
    }
    for needle in ["TARGET_EXE_SHA256", "PAYLOAD_SHA256", "installed Rally item readback failed"] {
        must(&rally_overlay, needle, "Rally pre-launch item installer");
    }
    must(
        &analog,
        "RALLY_ANALOG_LEGACY_PATCH_RVA = 0x2C360E",
        "exact analog RVA",
    );
    must(&analog, "F3 0F 11 64 8D 74", "analog snap-store preimage");
    must(&analog, "90 90 90 90 90 90", "legacy analog replacement");
    assert!(
        !analog.contains("2B8F2A"),
        "rejected DirectInput dispatch edit leaked into release"
    );
    assert_eq!(rally_release.len(), 3056, "Rally release item payload size");
    assert_eq!(rally_post.len(), 3057, "Rally post-fix item payload size");
    assert!(contains_bytes(&rally_release, b"Models\\RallyCar\\"));
    assert!(!contains_bytes(&rally_release, b"Models\\CarRally\\"));
    assert!(contains_bytes(&rally_post, b"Models\\CarRally\\"));
    assert!(!contains_bytes(&rally_post, b"Models\\RallyCar\\"));
    must(&manifest, "rally_custom_ice", "custom-ice provenance axis");
    must(&manifest, "shared_analog_input", "analog provenance axis");
    must(
        &manifest,
        "bit-identical across boundary",
        "physics-model negative control",
    );

    for name in ["sport", "stadium", "snow", "rally", "desert"] {
        must(
            &families,
            &format!("n.Contains(\"{name}\")"),
            "official vehicle family filter",
        );
    }
    for forbidden in [
        "BayCar",
        "CanyonCar",
        "CoastCar",
        "IslandCar",
        "LagoonCar",
        "ValleyCar",
        "TrafficCar",
        "Zai/Auris",
    ] {
        assert!(
            !families.contains(forbidden),
            "legacy/community family leaked into official-only selector: {forbidden}"
        );
        assert!(
            !catalog.contains(forbidden),
            "legacy/community family leaked into official-only catalog: {forbidden}"
        );
    }

    must(
        &data,
        "PROFILE_FALL2022_UNRESOLVED_CALLS = 0",
        "resolved call manifest",
    );
    must(
        &data,
        "PROFILE_FALL2022_UNRESOLVED_RIP = 0",
        "resolved RIP manifest",
    );
    must(&catalog, "STADIUM_PRE_2022_03_29", "January evidence");
    must(
        &catalog,
        "STADIUM_2022_03_29_TO_PRE_FALL_STAGING",
        "Spring evidence",
    );
    must(
        &catalog,
        "STADIUM_FALL_STAGED_2022_09_21_TO_2023_SUMMER",
        "Fall evidence",
    );
    must(&catalog, "STADIUM_SUMMER_2023_ONWARD", "Summer evidence");
    must(&catalog, "in-memory sphere geometry transaction ready", "selectable in-memory Snow collision profiles");
    assert!(!catalog.contains("blocked_collision_asset"), "Snow collision payload must not remain blocked");
    must(
        &info,
        "category = \"Developer\"",
        "Developer-mode containment",
    );

    println!(
        "release source verified: 12 official profiles; all historical entries catalog-only; four installed-current profiles selectable"
    );
}
