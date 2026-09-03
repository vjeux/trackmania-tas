const uint RALLY_ITEM_RELEASE_SIZE = 3056;
const uint RALLY_ITEM_CURRENT_SIZE = 2058;

uint64 g_RallyAnalogPatchSite = 0;
string g_RallyAnalogBackup;
bool g_RallyAnalogLegacyInstalled = false;
bool g_RallyReleasePathInstalled = false;

bool WantsLegacyRallyAnalog(PhysicsProfileId id) {
    return id == PhysicsProfileId::RallyRelease2024 ||
        id == PhysicsProfileId::RallyApril2024;
}

bool WantsRallyReleasePath(PhysicsProfileId id) {
    return id == PhysicsProfileId::RallyRelease2024;
}

bool IsRallyProfile(PhysicsProfileId id) {
    return id == PhysicsProfileId::RallyRelease2024 ||
        id == PhysicsProfileId::RallyApril2024 ||
        id == PhysicsProfileId::RallyMay2024Current;
}

CSystemFidFile@ FindRallyItemFid() {
    array<string> candidates = {
        "GameData/Vehicles/Items/Cars/CarRally.Item.Gbx",
        "Vehicles/Items/Cars/CarRally.Item.Gbx",
        "/Vehicles/Items/Cars/CarRally.Item.Gbx"
    };
    for (uint i = 0; i < candidates.Length; i++) {
        CSystemFidFile@ fid = Fids::GetGame(candidates[i]);
        if (fid !is null && fid.ByteSize > 0) return fid;
    }
    return null;
}

uint ObservedRallyItemSize() {
    auto fid = FindRallyItemFid();
    return fid is null ? 0 : fid.ByteSize;
}

bool ValidateRallyItemPathPreimage() {
    uint observed = ObservedRallyItemSize();
    if (observed == RALLY_ITEM_RELEASE_SIZE) {
        g_RallyReleasePathInstalled = true;
        return true;
    }
    if (observed == RALLY_ITEM_CURRENT_SIZE) {
        g_RallyReleasePathInstalled = false;
        return true;
    }
    g_Status = "Rally item FID has unsupported size " + observed
        + "; use rally_item_overlay restore/install and restart Trackmania";
    return false;
}

bool SetRallyReleasePath(bool historical) {
    uint observed = ObservedRallyItemSize();
    uint expected = historical ? RALLY_ITEM_RELEASE_SIZE : RALLY_ITEM_CURRENT_SIZE;
    if (observed != expected) {
        g_Status = historical
            ? "Rally release item is not active; run rally_item_overlay install, then restart Trackmania"
            : "Rally release item is still active; run rally_item_overlay restore, then restart Trackmania";
        return false;
    }
    g_RallyReleasePathInstalled = historical;
    return true;
}

bool ValidateRallyAnalogSite() {
    uint64 expected = Dev::BaseAddress() + RALLY_ANALOG_LEGACY_PATCH_RVA;
    if (!g_RallyAnalogLegacyInstalled) {
        uint64 context = Dev::FindPattern(RALLY_ANALOG_LEGACY_CONTEXT_PATTERN);
        if (context == 0 || context + 11 != expected) {
            g_Status = "build 128130 analog-input signature is missing or moved";
            return false;
        }
    } else if (g_RallyAnalogPatchSite != expected) {
        g_Status = "analog-input patch address ownership check failed";
        return false;
    }
    g_RallyAnalogPatchSite = expected;
    string wanted = g_RallyAnalogLegacyInstalled
        ? RALLY_ANALOG_LEGACY_PATCH_REPLACEMENT
        : RALLY_ANALOG_LEGACY_PATCH_PREIMAGE;
    if (Dev::Read(g_RallyAnalogPatchSite, RALLY_ANALOG_LEGACY_PATCH_BYTES) != wanted) {
        g_Status = "analog-input patch ownership check failed";
        return false;
    }
    return true;
}

bool SetRallyLegacyAnalog(bool historical) {
    if (historical == g_RallyAnalogLegacyInstalled) {
        if (!historical) return true;
        return ValidateRallyAnalogSite();
    }
    if (!ValidateRallyAnalogSite()) return false;

    if (historical) {
        string observed = Dev::Patch(
            g_RallyAnalogPatchSite,
            RALLY_ANALOG_LEGACY_PATCH_REPLACEMENT);
        if (observed != RALLY_ANALOG_LEGACY_PATCH_PREIMAGE ||
            Dev::Read(g_RallyAnalogPatchSite, RALLY_ANALOG_LEGACY_PATCH_BYTES) !=
                RALLY_ANALOG_LEGACY_PATCH_REPLACEMENT) {
            Dev::Patch(g_RallyAnalogPatchSite, observed);
            g_Status = "analog-input preimage changed concurrently; write rolled back";
            return false;
        }
        g_RallyAnalogBackup = observed;
        g_RallyAnalogLegacyInstalled = true;
        return true;
    }

    if (Dev::Read(g_RallyAnalogPatchSite, RALLY_ANALOG_LEGACY_PATCH_BYTES) !=
            RALLY_ANALOG_LEGACY_PATCH_REPLACEMENT) {
        g_Status = "analog-input bytes are no longer owned by this plugin";
        return false;
    }
    Dev::Patch(g_RallyAnalogPatchSite, g_RallyAnalogBackup);
    if (Dev::Read(g_RallyAnalogPatchSite, RALLY_ANALOG_LEGACY_PATCH_BYTES) !=
            RALLY_ANALOG_LEGACY_PATCH_PREIMAGE) {
        g_Status = "analog-input rollback verification failed";
        return false;
    }
    g_RallyAnalogBackup = "";
    g_RallyAnalogLegacyInstalled = false;
    return true;
}

bool RestoreRallyAxes(bool historicalPath, bool historicalAnalog) {
    bool ok = true;
    if (g_RallyAnalogLegacyInstalled != historicalAnalog &&
        !SetRallyLegacyAnalog(historicalAnalog)) ok = false;
    if (g_RallyReleasePathInstalled != historicalPath &&
        !SetRallyReleasePath(historicalPath)) ok = false;
    return ok;
}

bool ApplyRallyAxes(bool historicalPath, bool historicalAnalog) {
    bool previousPath = g_RallyReleasePathInstalled;
    bool previousAnalog = g_RallyAnalogLegacyInstalled;
    if (SetRallyReleasePath(historicalPath) && SetRallyLegacyAnalog(historicalAnalog)) {
        return true;
    }

    string failure = g_Status;
    bool rolledBack = RestoreRallyAxes(previousPath, previousAnalog);
    g_Status = rolledBack
        ? failure + "; transaction rolled back"
        : failure + "; rollback also failed, restart Trackmania before continuing";
    return false;
}

bool ValidateRallyProfileControls(bool updateStatus = true) {
    bool ok = ValidateRallyAnalogSite();
    string analogFailure = ok ? "" : g_Status;
    bool itemOk = ValidateRallyItemPathPreimage();
    uint observed = ObservedRallyItemSize();
    if (!itemOk) ok = false;
    if (updateStatus) {
        if (ok) {
            g_Status = "Rally/Desert controls pass: exact analog bytes and recognized Rally item FID";
        } else if (analogFailure != "") {
            g_Status = analogFailure;
        } else {
            g_Status = "Rally item FID control failed; observed size " + observed;
        }
    }
    return ok;
}

void RenderRallyProfileControls() {
    UI::Separator();
    UI::Text("Rally profile axes");
    UI::Text("Custom ice data: " +
        (g_RallyReleasePathInstalled ? "release item override" : "installed post-fix item"));
    UI::Text("Analog input: " +
        (g_RallyAnalogLegacyInstalled ? "pre-May 22" : "May 22/current"));
    if (UI::MenuItem("Run Rally/Desert profile self-check", "", false)) {
        ValidateRallyProfileControls(true);
    }
}

void RestoreRallyProfilesOnUnload() {
    bool analogOk = !g_RallyAnalogLegacyInstalled || SetRallyLegacyAnalog(false);
    if (!analogOk) {
        warn("[HistoricalPhysics] Rally analog rollback failed; restart Trackmania");
    }
    if (ObservedRallyItemSize() == RALLY_ITEM_RELEASE_SIZE) {
        warn("[HistoricalPhysics] Rally release item override remains active; run rally_item_overlay restore, then restart Trackmania");
    }
}
