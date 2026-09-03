// Exact-build-128130 SnowCar compatibility patches.
//
// These sites intentionally model separate behavior layers:
//   - input: pre-May smooth-steering and pre-Feb action-key routing;
//   - script API: release-era delayed Snow setters;
//   - collision data: pre-Feb SnowCar.Shape CPlugSurface, patched in memory.
//
// Every write is preflighted, read back, and rolled back as one transaction.

const uint64 SNOW_ACTION_KEY_BRANCH_RVA = 0x2B8C4C;
const uint64 SNOW_SMOOTH_STEERING_STORE_RVA = 0x2C360E;
const uint64 SNOW_DELAYED_ADHERENCE_CALL_RVA = 0x1342927;
const uint64 SNOW_DELAYED_ACCEL_CALL_RVA = 0x1342AB7;
const uint64 SNOW_DELAYED_CONTROL_CALL_RVA = 0x1342C47;

const string SNOW_ACTION_KEY_CURRENT = "74 18";
const string SNOW_ACTION_KEY_PRE_FEB = "90 90";
const string SNOW_SMOOTH_STEERING_CURRENT = "F3 0F 11 64 8D 74";
const string SNOW_SMOOTH_STEERING_PRE_MAY = "90 90 90 90 90 90";
const string SNOW_DELAYED_ADHERENCE_CURRENT = "E8 24 FB FF FF";
const string SNOW_DELAYED_ACCEL_CURRENT = "E8 94 F9 FF FF";
const string SNOW_DELAYED_CONTROL_CURRENT = "E8 04 F8 FF FF";
const string SNOW_DELAYED_RELEASE_NOOP = "31 C0 90 90 90";

// The historical/current sizes are retained as payload provenance; runtime
// ownership is verified from the loaded seven-shape CPlugSurface geometry.
const uint SNOW_COLLISION_PRE_FEB_SIZE = 1123;
const uint SNOW_COLLISION_CURRENT_SIZE = 1151;

class SnowBytePatch {
    string Name;
    uint64 Rva;
    uint Length;
    string CurrentBytes;
    string HistoricalBytes;
    bool Installed = false;

    SnowBytePatch(const string &in name, uint64 rva, uint length,
                  const string &in currentBytes, const string &in historicalBytes) {
        Name = name;
        Rva = rva;
        Length = length;
        CurrentBytes = currentBytes;
        HistoricalBytes = historicalBytes;
    }
}

array<SnowBytePatch@> g_SnowBytePatches;
bool g_SnowCodeInstalled = false;
GmSurfCompound@ g_SnowCollisionCompound = null;
array<vec3> g_SnowCollisionScaleBackup;
bool g_SnowHistoricalCollisionInstalled = false;

void InitializeSnowPatchSites() {
    if (g_SnowBytePatches.Length > 0) return;
    g_SnowBytePatches.InsertLast(SnowBytePatch(
        "pre-May analog smooth steering", SNOW_SMOOTH_STEERING_STORE_RVA, 6,
        SNOW_SMOOTH_STEERING_CURRENT, SNOW_SMOOTH_STEERING_PRE_MAY));
    g_SnowBytePatches.InsertLast(SnowBytePatch(
        "pre-Feb action-key routing", SNOW_ACTION_KEY_BRANCH_RVA, 2,
        SNOW_ACTION_KEY_CURRENT, SNOW_ACTION_KEY_PRE_FEB));
    g_SnowBytePatches.InsertLast(SnowBytePatch(
        "release delayed adherence", SNOW_DELAYED_ADHERENCE_CALL_RVA, 5,
        SNOW_DELAYED_ADHERENCE_CURRENT, SNOW_DELAYED_RELEASE_NOOP));
    g_SnowBytePatches.InsertLast(SnowBytePatch(
        "release delayed acceleration", SNOW_DELAYED_ACCEL_CALL_RVA, 5,
        SNOW_DELAYED_ACCEL_CURRENT, SNOW_DELAYED_RELEASE_NOOP));
    g_SnowBytePatches.InsertLast(SnowBytePatch(
        "release delayed control", SNOW_DELAYED_CONTROL_CALL_RVA, 5,
        SNOW_DELAYED_CONTROL_CURRENT, SNOW_DELAYED_RELEASE_NOOP));
}

bool IsSnowProfile(PhysicsProfileId id) {
    return id == PhysicsProfileId::SnowRelease2023 ||
        id == PhysicsProfileId::SnowJanuary2024 ||
        id == PhysicsProfileId::SnowFebruary2024 ||
        id == PhysicsProfileId::SnowMay2024Current;
}

CSystemFidFile@ FindSnowShapeFid() {
    array<string> candidates = {
        "GameData/Vehicles/Cars/CarSnow/SnowCar.Shape.Gbx",
        "Vehicles/Cars/CarSnow/SnowCar.Shape.Gbx",
        "/Vehicles/Cars/CarSnow/SnowCar.Shape.Gbx"
    };
    for (uint i = 0; i < candidates.Length; i++) {
        CSystemFidFile@ fid = Fids::GetGame(candidates[i]);
        if (fid !is null && fid.ByteSize > 0) return fid;
    }
    return null;
}

float SnowHistoricalRadius(uint index) {
    if (index == 0) return 1.195428014f;
    if (index == 1) return 0.969449997f;
    return 1.119449973f;
}

vec3 SnowCurrentScale(uint index) {
    if (index == 0) return vec3(1.195f, 1.1f, 1.195f);
    if (index == 1) return vec3(0.969f, 0.8f, 1.2f);
    return vec3(1.119f, 1.0f, 1.119f);
}

bool SnowFloatNear(float left, float right) {
    return Math::Abs(left - right) < 0.00001f;
}

bool SnowVecNear(const vec3 &in left, const vec3 &in right) {
    return SnowFloatNear(left.x, right.x) && SnowFloatNear(left.y, right.y)
        && SnowFloatNear(left.z, right.z);
}

GmSurfCompound@ ResolveSnowCollisionCompound() {
    auto fid = FindSnowShapeFid();
    if (fid is null || fid.ByteSize != SNOW_COLLISION_CURRENT_SIZE) {
        g_Status = "supported current Snow shape FID is unavailable";
        return null;
    }
    auto surface = cast<CPlugSurface>(Fids::Preload(fid));
    if (surface is null || surface.m_GmSurf is null) {
        g_Status = "loaded Snow CPlugSurface is unavailable";
        return null;
    }
    auto compound = cast<GmSurfCompound>(surface.m_GmSurf);
    if (compound is null || compound.Surfs.Length != 7 || compound.SurfLocs.Length != 7) {
        g_Status = "Snow collision compound is not the expected seven-shape layout";
        return null;
    }
    @g_SnowCollisionCompound = compound;
    return compound;
}

bool SnowCollisionEpochMatches(bool historical) {
    auto compound = g_SnowCollisionCompound;
    if (compound is null) @compound = ResolveSnowCollisionCompound();
    if (compound is null) return false;
    for (uint i = 0; i < 3; i++) {
        auto child = compound.Surfs[i];
        if (historical) {
            if (child.GmSurfType != EGmSurfType::Sphere) return false;
            auto sphere = cast<GmSurfSphere>(child);
            if (sphere is null || !SnowFloatNear(sphere.Radius, SnowHistoricalRadius(i))) return false;
        } else {
            if (child.GmSurfType != EGmSurfType::Ellipsoid) return false;
            auto ellipsoid = cast<GmSurfEllipsoid>(child);
            if (ellipsoid is null || !SnowVecNear(ellipsoid.Scale, SnowCurrentScale(i))) return false;
        }
    }
    for (uint i = 3; i < 7; i++) {
        auto child = compound.Surfs[i];
        auto sphere = cast<GmSurfSphere>(child);
        if (child.GmSurfType != EGmSurfType::Sphere || sphere is null
            || !SnowFloatNear(sphere.Radius, 0.47f)) return false;
    }
    return true;
}

bool ValidateSnowCollisionEpoch(bool historical) {
    if (SnowCollisionEpochMatches(historical)) return true;
    g_Status = historical
        ? "loaded Snow collision does not match the pre-Feb sphere geometry"
        : "loaded Snow collision does not match the installed ellipsoid geometry";
    return false;
}

bool SetSnowHistoricalCollision(bool historical) {
    if (historical == g_SnowHistoricalCollisionInstalled) {
        return ValidateSnowCollisionEpoch(historical);
    }
    auto compound = g_SnowCollisionCompound;
    if (compound is null) @compound = ResolveSnowCollisionCompound();
    if (compound is null || !ValidateSnowCollisionEpoch(g_SnowHistoricalCollisionInstalled)) return false;

    if (historical) {
        g_SnowCollisionScaleBackup.RemoveRange(0, g_SnowCollisionScaleBackup.Length);
        for (uint i = 0; i < 3; i++) {
            auto ellipsoid = cast<GmSurfEllipsoid>(compound.Surfs[i]);
            g_SnowCollisionScaleBackup.InsertLast(ellipsoid.Scale);
        }
        uint changed = 0;
        try {
            for (uint i = 0; i < 3; i++) {
                auto child = compound.Surfs[i];
                child.GmSurfType = EGmSurfType::Sphere;
                cast<GmSurfSphere>(child).Radius = SnowHistoricalRadius(i);
                changed++;
            }
        } catch {
            for (uint i = 0; i < changed; i++) {
                auto child = compound.Surfs[i];
                child.GmSurfType = EGmSurfType::Ellipsoid;
                cast<GmSurfEllipsoid>(child).Scale = g_SnowCollisionScaleBackup[i];
            }
            g_Status = "Snow collision mutation exception: " + getExceptionInfo();
            return false;
        }
        g_SnowHistoricalCollisionInstalled = true;
        if (!ValidateSnowCollisionEpoch(true)) {
            SetSnowHistoricalCollision(false);
            g_Status = "Snow collision mutation failed readback and was rolled back";
            return false;
        }
        return true;
    }

    if (g_SnowCollisionScaleBackup.Length != 3) {
        g_Status = "Snow collision rollback scales are unavailable";
        return false;
    }
    for (uint i = 0; i < 3; i++) {
        auto child = compound.Surfs[i];
        child.GmSurfType = EGmSurfType::Ellipsoid;
        cast<GmSurfEllipsoid>(child).Scale = g_SnowCollisionScaleBackup[i];
    }
    g_SnowHistoricalCollisionInstalled = false;
    if (!ValidateSnowCollisionEpoch(false)) {
        g_Status = "Snow collision rollback verification failed; restart Trackmania";
        return false;
    }
    g_SnowCollisionScaleBackup.RemoveRange(0, g_SnowCollisionScaleBackup.Length);
    @g_SnowCollisionCompound = null;
    return true;
}

array<bool> DesiredSnowCodePatches(PhysicsProfileId id) {
    array<bool> desired = {false, false, false, false, false};
    if (id == PhysicsProfileId::SnowRelease2023) {
        for (uint i = 0; i < desired.Length; i++) desired[i] = true;
    } else if (id == PhysicsProfileId::SnowJanuary2024) {
        desired[0] = true;
        desired[1] = true;
    } else if (id == PhysicsProfileId::SnowFebruary2024) {
        desired[0] = true;
    }
    return desired;
}

bool SnowProfileNeedsHistoricalCollision(PhysicsProfileId id) {
    return id == PhysicsProfileId::SnowRelease2023 ||
        id == PhysicsProfileId::SnowJanuary2024;
}

bool PreflightSnowCodeTransaction(const array<bool> &in desired) {
    if (desired.Length != g_SnowBytePatches.Length) {
        g_Status = "internal Snow patch-plan length mismatch";
        return false;
    }
    uint64 imageBase = Dev::BaseAddress();
    for (uint i = 0; i < g_SnowBytePatches.Length; i++) {
        auto site = g_SnowBytePatches[i];
        string expected = site.Installed ? site.HistoricalBytes : site.CurrentBytes;
        string observed = Dev::Read(imageBase + site.Rva, site.Length);
        if (observed != expected) {
            g_Status = "Snow patch preimage mismatch at " + site.Name + "; no memory was changed";
            return false;
        }
    }
    return true;
}

bool RollBackSnowCodeChanges(const array<uint> &in changed,
                             const array<string> &in backups,
                             const array<bool> &in previousStates) {
    uint64 imageBase = Dev::BaseAddress();
    bool restored = true;
    for (int index = int(changed.Length) - 1; index >= 0; index--) {
        uint siteIndex = changed[index];
        auto site = g_SnowBytePatches[siteIndex];
        string rollbackBytes = backups[index];
        Dev::Patch(imageBase + site.Rva, rollbackBytes);
        if (Dev::Read(imageBase + site.Rva, site.Length) != rollbackBytes) {
            restored = false;
        } else {
            site.Installed = previousStates[siteIndex];
        }
    }
    return restored;
}

bool ApplySnowCodeTransaction(const array<bool> &in desired) {
    if (!PreflightSnowCodeTransaction(desired)) return false;

    array<bool> previousStates;
    array<uint> changed;
    array<string> backups;
    for (uint i = 0; i < g_SnowBytePatches.Length; i++) {
        previousStates.InsertLast(g_SnowBytePatches[i].Installed);
    }

    uint64 imageBase = Dev::BaseAddress();
    string failure;
    try {
        for (uint i = 0; i < g_SnowBytePatches.Length; i++) {
            auto site = g_SnowBytePatches[i];
            if (site.Installed == desired[i]) continue;
            string expected = site.Installed ? site.HistoricalBytes : site.CurrentBytes;
            string replacement = desired[i] ? site.HistoricalBytes : site.CurrentBytes;
            string backup = Dev::Patch(imageBase + site.Rva, replacement);
            changed.InsertLast(i);
            backups.InsertLast(backup);
            if (backup != expected || Dev::Read(imageBase + site.Rva, site.Length) != replacement) {
                failure = "Snow patch verification failed at " + site.Name;
                break;
            }
            site.Installed = desired[i];
        }
    } catch {
        failure = "Snow patch exception: " + getExceptionInfo();
    }
    if (failure != "") {
        bool restored = RollBackSnowCodeChanges(changed, backups, previousStates);
        g_Status = restored
            ? failure + "; transaction rolled back"
            : failure + "; rollback verification also failed";
        return false;
    }

    g_SnowCodeInstalled = false;
    for (uint i = 0; i < g_SnowBytePatches.Length; i++) {
        if (g_SnowBytePatches[i].Installed) g_SnowCodeInstalled = true;
    }
    return true;
}

bool ValidateSnowTargetSites() {
    InitializeSnowPatchSites();
    array<bool> current = {false, false, false, false, false};
    return PreflightSnowCodeTransaction(current) && ValidateSnowCollisionEpoch(false);
}

bool RemoveSnowCodePatches() {
    array<bool> current = {false, false, false, false, false};
    bool ok = ApplySnowCodeTransaction(current);
    if (ok) g_SnowCodeInstalled = false;
    return ok;
}

bool RemoveSnowProfilePatches() {
    bool codeOk = !g_SnowCodeInstalled || RemoveSnowCodePatches();
    bool collisionOk = !g_SnowHistoricalCollisionInstalled || SetSnowHistoricalCollision(false);
    return codeOk && collisionOk;
}

bool InstallSnowProfile(PhysicsProfileId id) {
    if (!IsSnowProfile(id)) {
        g_Status = "internal error: non-Snow profile sent to Snow installer";
        return false;
    }
    if (id != PhysicsProfileId::SnowMay2024Current) {
        g_Status = "historical Snow profiles are fail-closed until matched old-client trajectory certification";
        return false;
    }
    if (id != PhysicsProfileId::SnowMay2024Current && !g_ExperimentalUnlocked) {
        g_Status = "arm Experimental native profiles first";
        return false;
    }
    if (!g_BuildSupported || !AtSafeSwitchPoint()) return false;
    InitializeSnowPatchSites();
    bool previousCollision = g_SnowHistoricalCollisionInstalled;
    bool historicalCollision = SnowProfileNeedsHistoricalCollision(id);
    if (!SetSnowHistoricalCollision(historicalCollision)) return false;
    if (!ApplySnowCodeTransaction(DesiredSnowCodePatches(id))) {
        string failure = g_Status;
        bool restored = SetSnowHistoricalCollision(previousCollision);
        g_Status = restored ? failure + "; collision transaction rolled back"
            : failure + "; collision rollback also failed, restart Trackmania";
        return false;
    }
    g_ActiveProfile = id;
    g_Status = ProfileName(id) + " active; choose CarSnow in Official vehicle family; offline use only";
    trace("[HistoricalPhysics] ACTIVE " + ProfileName(id));
    return true;
}
