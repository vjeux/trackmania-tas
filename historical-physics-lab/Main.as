// Historical Physics Lab: exact-build, menu-only engine selector.
// Developer Mode / School Mode only. Native profiles are never enabled automatically.

const string TARGET_HANDLER_PATTERN =
    "48 8B C4 F3 0F 11 48 10 48 89 48 08 55 56 57 41 54 "
    "48 8D A8 48 FE FF FF 48 81 EC 98 02 00 00 49 8B 78 08 "
    "41 BA 04 00 00 00";
const string TARGET_BUILD_BANNER_PATTERN =
    "64 61 74 65 3D 32 30 32 36 2D 30 31 2D 32 38 5F 31 33 5F 30 30 20 "
    "67 69 74 3D 31 32 38 31 33 30 2D 36 64 64 61 33 37 32 38 65 39 31 20 "
    "47 61 6D 65 56 65 72 73 69 6F 6E 3D 33 2E 33 2E 30";
const uint ENTRY_PATCH_BYTES = 12;
const string TARGET_ENTRY_ORIGINAL = "48 8B C4 F3 0F 11 48 10 48 89 48 08";

// Measured on build 128130 after CarSport catalog preload.
const uint FALL_ITEM_ENTITY_MODEL_OFFSET = 0x288;
const uint FALL_VEHICLE_PHY_MODEL_OFFSET = 0x28;
const uint FALL_CURRENT_TUNINGS_OFFSET = 0x18;
const uint FALL_TUNINGS_DATA_OFFSET = 0x18;
const uint FALL_TUNINGS_COUNT_OFFSET = 0x20;
const uint FALL_TUNINGS_CAPACITY_OFFSET = 0x24;
const uint FALL_TUNINGS_CLASS_OFFSET = 0x28;
const uint FALL_TUNINGS_INDEX_OFFSET = 0x2C;
const uint FALL_TUNING_NAME_OFFSET = 0x18;
const uint FALL_EXPECTED_CURRENT_COUNT = 28;
const uint FALL_TARGET_COUNT = 25;
const uint FALL_EXPECTED_CURRENT_INDEX = 27;
const uint FALL_TARGET_INDEX = 24;
const uint FALL_TUNING_CLASS_ID = 0x090EB000;
array<string> FALL_EXPECTED_TUNING_NAMES = {
    "20fev2013", "06/12/2019_TurboAirControl_Ice", "06/12/2019_TurboAirControl_Ice",
    "WallRepulse", "IceDrift", "IceDriftV1", "IceDriftV2", "AirControlStab",
    "IceDriftV3", "IceDriftV4", "IceDriftV5", "IceDriftV6", "Reactors200605",
    "IceDrift200609", "IceDrift200618", "IceDrift200619", "IceDrift200621",
    "IceDrift200624", "AntiWallHit201021", "Water210115", "Water210208",
    "Water210415", "ExperimentalBobsleighSteer", "NoWiggle\n", "NoWiggleAjusté\n",
    "Wood0", "Wood_20240101_MoreAccelForSlope", "Wood_20240101_MoreAccelForSlope2"
};

array<PhysicsProfile@> g_Profiles;
PhysicsProfileId g_SelectedProfile = PhysicsProfileId::StadiumSummer2023Current;
PhysicsProfileId g_ActiveProfile = PhysicsProfileId::StadiumSummer2023Current;
uint64 g_Handler = 0;
uint64 g_Island = 0;
string g_EntryBackup;
string g_EntryJump;
bool g_Installed = false;
bool g_FallRuntimePatched = false;
CMwNod@ g_FallTunings = null;
uint g_FallOriginalCount = 0;
uint g_FallOriginalIndex = 0;
bool g_BuildSupported = false;
bool g_ExperimentalUnlocked = false;
string g_Status = "initializing";

string HexByte(uint8 value) { return Text::Format("%02X", value); }

string HexU64LE(uint64 value) {
    string encoded;
    for (uint i = 0; i < 8; i++) {
        if (i > 0) encoded += " ";
        encoded += HexByte(uint8((value >> (i * 8)) & 0xFF));
    }
    return encoded;
}

string AbsoluteJump(uint64 target) {
    return "48 B8 " + HexU64LE(target) + " FF E0";
}

PhysicsProfile@ Profile(PhysicsProfileId id) {
    for (uint i = 0; i < g_Profiles.Length; i++) {
        if (g_Profiles[i].Id == id) return g_Profiles[i];
    }
    return null;
}

string ProfileName(PhysicsProfileId id) {
    auto p = Profile(id);
    return p is null ? "unknown" : p.Name;
}

bool ValidateBuild() {
    uint64 banner = Dev::FindPattern(TARGET_BUILD_BANNER_PATTERN);
    g_Handler = Dev::FindPattern(TARGET_HANDLER_PATTERN);
    if (banner == 0 || g_Handler == 0) {
        g_Status = "unsupported Trackmania build; no memory was changed";
        return false;
    }
    if (Dev::Read(g_Handler, 41) != TARGET_HANDLER_PATTERN) {
        g_Handler = 0;
        g_Status = "CarSport signature verification failed; no memory was changed";
        return false;
    }
    g_BuildSupported = true;
    return true;
}

bool ValidateJanuary2022Manifest() {
    if (PROFILE_JAN2022_UNRESOLVED_CALLS != 0 || PROFILE_JAN2022_UNRESOLVED_RIP != 0) {
        g_Status = "January 2022 profile contains unresolved native references";
        return false;
    }
    if (PROFILE_JAN2022_RELOC_OFFSETS.Length != PROFILE_JAN2022_RELOC_TARGET_RVAS.Length ||
        PROFILE_JAN2022_RELOC_OFFSETS.Length != PROFILE_JAN2022_RELOC_TARGET_IS_ISLAND.Length ||
        PROFILE_JAN2022_ABS64_OFFSETS.Length != PROFILE_JAN2022_ABS64_TARGET_RVAS.Length ||
        PROFILE_JAN2022_FIELD_OFFSETS.Length != PROFILE_JAN2022_FIELD_RELOCATION_COUNT ||
        PROFILE_JAN2022_FIELD_SOURCE_VAS.Length != PROFILE_JAN2022_FIELD_RELOCATION_COUNT ||
        PROFILE_JAN2022_FIELD_WIDTHS.Length != PROFILE_JAN2022_FIELD_RELOCATION_COUNT ||
        PROFILE_JAN2022_FIELD_OLD_VALUES.Length != PROFILE_JAN2022_FIELD_RELOCATION_COUNT ||
        PROFILE_JAN2022_FIELD_NEW_VALUES.Length != PROFILE_JAN2022_FIELD_RELOCATION_COUNT ||
        PROFILE_JAN2022_CALL_RELOC_OFFSETS.Length != PROFILE_JAN2022_CALL_RELOCATION_COUNT ||
        PROFILE_JAN2022_CALL_TARGET_ISLAND_OFFSETS.Length != PROFILE_JAN2022_CALL_RELOCATION_COUNT ||
        PROFILE_JAN2022_RIP_RELOC_OFFSETS.Length != PROFILE_JAN2022_RIP_RELOCATION_COUNT ||
        PROFILE_JAN2022_RIP_TARGET_RVAS.Length != PROFILE_JAN2022_RIP_RELOCATION_COUNT ||
        PROFILE_JAN2022_RIP_TARGET_IS_ISLAND.Length != PROFILE_JAN2022_RIP_RELOCATION_COUNT ||
        PROFILE_JAN2022_RELOC_OFFSETS.Length !=
            PROFILE_JAN2022_CALL_RELOCATION_COUNT + PROFILE_JAN2022_RIP_RELOCATION_COUNT ||
        PROFILE_JAN2022_INIT_SOURCE_VAS.Length != PROFILE_JAN2022_INIT_SHADOW_OFFSETS.Length ||
        PROFILE_JAN2022_INIT_SOURCE_VAS.Length != PROFILE_JAN2022_INIT_VALUES.Length ||
        PROFILE_JAN2022_SOURCE_REGION_START_VAS.Length != PROFILE_JAN2022_SOURCE_REGION_END_VAS.Length ||
        PROFILE_JAN2022_SOURCE_REGION_START_VAS.Length != PROFILE_JAN2022_ISLAND_REGION_OFFSETS.Length ||
        PROFILE_JAN2022_SOURCE_REGION_START_VAS.Length != PROFILE_JAN2022_ISLAND_REGION_LENGTHS.Length) {
        g_Status = "January 2022 relocation manifest is inconsistent";
        return false;
    }
    return true;
}

bool ValidateFall2022Manifest() {
    if (PROFILE_FALL2022_UNRESOLVED_CALLS != 0 || PROFILE_FALL2022_UNRESOLVED_RIP != 0) {
        g_Status = "Fall 2022 profile contains unresolved native references";
        return false;
    }
    if (PROFILE_FALL2022_RELOC_OFFSETS.Length != PROFILE_FALL2022_RELOC_TARGET_RVAS.Length ||
        PROFILE_FALL2022_RELOC_OFFSETS.Length != PROFILE_FALL2022_RELOC_TARGET_IS_ISLAND.Length ||
        PROFILE_FALL2022_ABS64_OFFSETS.Length != PROFILE_FALL2022_ABS64_TARGET_RVAS.Length ||
        PROFILE_FALL2022_FIELD_REMAP_COUNT != 44 ||
        PROFILE_FALL2022_ABI_ADAPTER_COUNT != 2 ||
        PROFILE_FALL2022_RELOCATED_CALL_COUNT != 155 ||
        PROFILE_FALL2022_ABS64_THUNK_COUNT != 40) {
        g_Status = "Fall 2022 relocation manifest is inconsistent";
        return false;
    }
    return true;
}

bool AtSafeSwitchPoint() {
    if (GetApp().CurrentPlayground !is null || GetApp().Editor !is null) {
        g_Status = "return to the main menu before switching physics";
        return false;
    }
    return true;
}

void PreloadFallRuntimeGraph() {
    auto catalog = GetApp().GlobalCatalog;
    for (uint i = 0; i < catalog.Chapters.Length; i++) {
        auto chapter = catalog.Chapters[i];
        if (!(chapter.IdName == "Vehicles" || chapter.IdName == "#10003")) continue;
        for (uint j = 0; j < chapter.Articles.Length; j++) {
            auto article = chapter.Articles[j];
            if (article !is null && string(article.Name) == "CarSport") article.Preload();
        }
    }
}

CMwNod@ ResolveFallTunings() {
    auto catalog = GetApp().GlobalCatalog;
    for (uint i = 0; i < catalog.Chapters.Length; i++) {
        auto chapter = catalog.Chapters[i];
        if (!(chapter.IdName == "Vehicles" || chapter.IdName == "#10003")) continue;
        for (uint j = 0; j < chapter.Articles.Length; j++) {
            auto article = chapter.Articles[j];
            if (article is null || string(article.Name) != "CarSport") continue;
            if (article.LoadedNod is null) {
                article.Preload();
                g_Status = "CarSport runtime data is still loading; apply again from the main menu";
                return null;
            }
            auto item = article.LoadedNod;
            auto itemType = Reflection::TypeOf(item);
            if (itemType is null || itemType.Name != "CGameItemModel") {
                g_Status = "unexpected CarSport item-model layout; no memory was changed";
                return null;
            }
            auto entity = Dev::GetOffsetNod(item, FALL_ITEM_ENTITY_MODEL_OFFSET);
            if (entity is null) {
                g_Status = "CarSport entity model is null; no memory was changed";
                return null;
            }
            auto entityType = Reflection::TypeOf(entity);
            if (entityType is null || entityType.Name != "CGameVehicleModel") {
                g_Status = "unexpected CarSport vehicle-model layout; no memory was changed";
                return null;
            }
            auto phy = Dev::GetOffsetNod(entity, FALL_VEHICLE_PHY_MODEL_OFFSET);
            if (phy is null) {
                g_Status = "CarSport physics model is null; no memory was changed";
                return null;
            }
            auto phyType = Reflection::TypeOf(phy);
            if (phyType is null || phyType.Name != "_0x090EA000") {
                g_Status = "unexpected CarSport physics-model layout; no memory was changed";
                return null;
            }
            auto tunings = Dev::GetOffsetNod(phy, FALL_CURRENT_TUNINGS_OFFSET);
            if (tunings is null) {
                g_Status = "CarSport tunings are null; no memory was changed";
                return null;
            }
            auto tuningsType = Reflection::TypeOf(tunings);
            if (tuningsType is null || tuningsType.Name != "_0x090EC000") {
                g_Status = "unexpected CarSport tunings layout; no memory was changed";
                return null;
            }
            return tunings;
        }
    }
    g_Status = "CarSport catalog article was not found; no memory was changed";
    return null;
}

bool ValidateCurrentFallGraph(CMwNod@ tunings) {
    if (tunings is null || FALL_EXPECTED_TUNING_NAMES.Length != FALL_EXPECTED_CURRENT_COUNT) {
        g_Status = "Fall tuning manifest is inconsistent";
        return false;
    }
    uint64 data = Dev::GetOffsetUint64(tunings, FALL_TUNINGS_DATA_OFFSET);
    uint count = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_COUNT_OFFSET);
    uint capacity = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_CAPACITY_OFFSET);
    uint classId = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_CLASS_OFFSET);
    uint index = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_INDEX_OFFSET);
    if (data == 0 || count != FALL_EXPECTED_CURRENT_COUNT ||
        capacity != FALL_EXPECTED_CURRENT_COUNT || classId != FALL_TUNING_CLASS_ID ||
        index != FALL_EXPECTED_CURRENT_INDEX) {
        g_Status = "current CarSport tuning header did not match the measured build-128130 preimage";
        return false;
    }
    for (uint i = 0; i < FALL_EXPECTED_TUNING_NAMES.Length; i++) {
        uint64 tuning = Dev::ReadUInt64(data + uint64(i) * 8);
        if (tuning == 0) {
            g_Status = "current CarSport tuning pointer " + i + " is null";
            return false;
        }
        uint nameId = Dev::ReadUInt32(tuning + FALL_TUNING_NAME_OFFSET);
        string name = MwId(nameId).GetName();
        if (name != FALL_EXPECTED_TUNING_NAMES[i]) {
            g_Status = "current CarSport tuning " + i + " was " + name
                + ", expected " + FALL_EXPECTED_TUNING_NAMES[i] + "; no memory was changed";
            return false;
        }
    }
    return true;
}

bool ApplyFallRuntimeGraph() {
    auto tunings = ResolveFallTunings();
    if (tunings is null || !ValidateCurrentFallGraph(tunings)) return false;
    g_FallOriginalCount = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_COUNT_OFFSET);
    g_FallOriginalIndex = Dev::GetOffsetUint32(tunings, FALL_TUNINGS_INDEX_OFFSET);
    try {
        Dev::SetOffset(tunings, FALL_TUNINGS_INDEX_OFFSET, uint(FALL_TARGET_INDEX));
        Dev::SetOffset(tunings, FALL_TUNINGS_COUNT_OFFSET, uint(FALL_TARGET_COUNT));
        if (Dev::GetOffsetUint32(tunings, FALL_TUNINGS_INDEX_OFFSET) != FALL_TARGET_INDEX ||
            Dev::GetOffsetUint32(tunings, FALL_TUNINGS_COUNT_OFFSET) != FALL_TARGET_COUNT) {
            Dev::SetOffset(tunings, FALL_TUNINGS_COUNT_OFFSET, g_FallOriginalCount);
            Dev::SetOffset(tunings, FALL_TUNINGS_INDEX_OFFSET, g_FallOriginalIndex);
            g_Status = "Fall tuning transaction did not verify and was rolled back";
            return false;
        }
    } catch {
        Dev::SetOffset(tunings, FALL_TUNINGS_COUNT_OFFSET, g_FallOriginalCount);
        Dev::SetOffset(tunings, FALL_TUNINGS_INDEX_OFFSET, g_FallOriginalIndex);
        g_Status = "Fall tuning transaction raised an exception and was rolled back: " + getExceptionInfo();
        return false;
    }
    @g_FallTunings = tunings;
    g_FallRuntimePatched = true;
    return true;
}

bool RestoreFallRuntimeGraph() {
    if (!g_FallRuntimePatched) return true;
    if (g_FallTunings is null ||
        Dev::GetOffsetUint32(g_FallTunings, FALL_TUNINGS_COUNT_OFFSET) != FALL_TARGET_COUNT ||
        Dev::GetOffsetUint32(g_FallTunings, FALL_TUNINGS_INDEX_OFFSET) != FALL_TARGET_INDEX) {
        g_Status = "Fall tuning header was changed by another patch; refusing an unsafe rollback";
        return false;
    }
    try {
        Dev::SetOffset(g_FallTunings, FALL_TUNINGS_COUNT_OFFSET, g_FallOriginalCount);
        Dev::SetOffset(g_FallTunings, FALL_TUNINGS_INDEX_OFFSET, g_FallOriginalIndex);
        if (Dev::GetOffsetUint32(g_FallTunings, FALL_TUNINGS_COUNT_OFFSET) != g_FallOriginalCount ||
            Dev::GetOffsetUint32(g_FallTunings, FALL_TUNINGS_INDEX_OFFSET) != g_FallOriginalIndex) {
            Dev::SetOffset(g_FallTunings, FALL_TUNINGS_INDEX_OFFSET, uint(FALL_TARGET_INDEX));
            Dev::SetOffset(g_FallTunings, FALL_TUNINGS_COUNT_OFFSET, uint(FALL_TARGET_COUNT));
            g_Status = "current tuning header did not restore; Fall values were re-applied";
            return false;
        }
    } catch {
        g_Status = "Fall tuning rollback raised an exception: " + getExceptionInfo();
        return false;
    }
    g_FallRuntimePatched = false;
    @g_FallTunings = null;
    return true;
}

bool RelocateJanuary2022Island() {
    uint64 imageBase = Dev::BaseAddress();
    for (uint i = 0; i < PROFILE_JAN2022_ABS64_OFFSETS.Length; i++) {
        Dev::Write(g_Island + PROFILE_JAN2022_ABS64_OFFSETS[i],
                   imageBase + PROFILE_JAN2022_ABS64_TARGET_RVAS[i]);
    }
    for (uint i = 0; i < PROFILE_JAN2022_RELOC_OFFSETS.Length; i++) {
        uint64 operand = g_Island + PROFILE_JAN2022_RELOC_OFFSETS[i];
        uint64 target = PROFILE_JAN2022_RELOC_TARGET_IS_ISLAND[i]
            ? g_Island + PROFILE_JAN2022_RELOC_TARGET_RVAS[i]
            : imageBase + PROFILE_JAN2022_RELOC_TARGET_RVAS[i];
        int64 delta = int64(target) - int64(operand + 4);
        if (delta < (-2147483647 - 1) || delta > 2147483647) {
            g_Status = "January 2022 rel32 target is out of range at relocation " + i;
            return false;
        }
        Dev::Write(operand, int(delta));
    }
    return true;
}

bool RelocateFall2022Island() {
    uint64 imageBase = Dev::BaseAddress();
    for (uint i = 0; i < PROFILE_FALL2022_ABS64_OFFSETS.Length; i++) {
        Dev::Write(g_Island + PROFILE_FALL2022_ABS64_OFFSETS[i],
                   imageBase + PROFILE_FALL2022_ABS64_TARGET_RVAS[i]);
    }
    for (uint i = 0; i < PROFILE_FALL2022_RELOC_OFFSETS.Length; i++) {
        uint64 operand = g_Island + PROFILE_FALL2022_RELOC_OFFSETS[i];
        uint64 target = PROFILE_FALL2022_RELOC_TARGET_IS_ISLAND[i]
            ? g_Island + PROFILE_FALL2022_RELOC_TARGET_RVAS[i]
            : imageBase + PROFILE_FALL2022_RELOC_TARGET_RVAS[i];
        int64 delta = int64(target) - int64(operand + 4);
        if (delta < (-2147483647 - 1) || delta > 2147483647) {
            g_Status = "Fall 2022 rel32 target is out of range at relocation " + i;
            return false;
        }
        Dev::Write(operand, int(delta));
    }
    return true;
}

bool RemoveNativeProfile() {
    InitializeSnowPatchSites();
    if ((g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveSnowProfilePatches()) return false;
    if (!g_Installed) {
        if (g_FallRuntimePatched && !RestoreFallRuntimeGraph()) return false;
        g_ActiveProfile = PhysicsProfileId::StadiumSummer2023Current;
        return true;
    }
    if (Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != g_EntryJump) {
        g_Status = "CarSport entry was changed by another patch; allocation and Fall graph retained for safety";
        return false;
    }
    bool hadFallGraph = g_FallRuntimePatched;
    if (hadFallGraph && !RestoreFallRuntimeGraph()) return false;
    Dev::Patch(g_Handler, g_EntryBackup);
    if (Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != g_EntryBackup) {
        if (hadFallGraph) ApplyFallRuntimeGraph();
        g_Status = "original CarSport entry did not restore; Fall state was retained where possible";
        return false;
    }
    Dev::Free(g_Island);
    g_Island = 0;
    g_Installed = false;
    g_ActiveProfile = PhysicsProfileId::StadiumSummer2023Current;
    g_Status = "current installed physics and 28-entry tuning view restored";
    trace("[HistoricalPhysics] RESTORED current physics and tuning view");
    return true;
}

bool InstallJanuary2022() {
    if (!PROFILE_JAN2022_STATIC_COMPLETE || !PROFILE_JAN2022_BEHAVIOR_CERTIFIED) {
        g_Status = "January 2022 is fail-closed until its field/ABI audit and matched historical trajectory are complete";
        return false;
    }
    if (!g_ExperimentalUnlocked) {
        g_Status = "arm Experimental native profiles first";
        return false;
    }
    if (!g_BuildSupported || !AtSafeSwitchPoint() || !ValidateJanuary2022Manifest()) return false;
    if (g_Installed && g_ActiveProfile == PhysicsProfileId::StadiumJanuary2022) return true;
    if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveNativeProfile()) return false;
    if (Dev::Read(g_Handler, 41) != TARGET_HANDLER_PATTERN) {
        g_Status = "CarSport entry is not original; refusing to patch over another plugin";
        return false;
    }

    g_Island = Dev::Allocate(PROFILE_JAN2022_ISLAND_SIZE, true);
    if (g_Island == 0) {
        g_Status = "executable allocation failed";
        return false;
    }

    try {
        Dev::Write(g_Island, PROFILE_JAN2022_ISLAND_BYTES);
        if (!RelocateJanuary2022Island()) {
            Dev::Free(g_Island); g_Island = 0;
            return false;
        }
        g_EntryJump = AbsoluteJump(g_Island);
        g_EntryBackup = Dev::Patch(g_Handler, g_EntryJump);
        if (g_EntryBackup != TARGET_ENTRY_ORIGINAL ||
            Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != g_EntryJump) {
            Dev::Patch(g_Handler, g_EntryBackup);
            Dev::Free(g_Island); g_Island = 0;
            g_Status = "entry changed concurrently or patch verification failed";
            return false;
        }
    } catch {
        if (g_Island != 0) Dev::Free(g_Island);
        g_Island = 0;
        g_Status = "installation exception: " + getExceptionInfo();
        return false;
    }

    g_Installed = true;
    g_ActiveProfile = PhysicsProfileId::StadiumJanuary2022;
    g_Status = "January 2022 static-complete preview active; behavior certification pending";
    trace("[HistoricalPhysics] ACTIVE January 2022 handler="
        + Text::FormatPointer(g_Handler) + " island=" + Text::FormatPointer(g_Island));
    return true;
}

bool InstallFall2022() {
    if (!PROFILE_FALL2022_BEHAVIOR_CERTIFIED) {
        g_Status = "Fall 2022 is fail-closed until exact historical behavior is certified";
        return false;
    }
    if (!g_ExperimentalUnlocked) {
        g_Status = "arm Experimental native profiles first";
        return false;
    }
    if (!g_BuildSupported || !AtSafeSwitchPoint() || !ValidateFall2022Manifest()) return false;
    if (g_Installed && g_ActiveProfile == PhysicsProfileId::StadiumFall2022) return true;
    if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveNativeProfile()) return false;
    auto tunings = ResolveFallTunings();
    if (tunings is null || !ValidateCurrentFallGraph(tunings)) return false;
    if (Dev::Read(g_Handler, 41) != TARGET_HANDLER_PATTERN) {
        g_Status = "CarSport entry is not original; refusing to patch over another plugin";
        return false;
    }

    g_Island = Dev::Allocate(PROFILE_FALL2022_ISLAND_SIZE, true);
    if (g_Island == 0) {
        g_Status = "executable allocation failed";
        return false;
    }

    try {
        Dev::Write(g_Island, PROFILE_FALL2022_ISLAND_BYTES);
        if (!RelocateFall2022Island()) {
            Dev::Free(g_Island); g_Island = 0;
            return false;
        }
        g_EntryJump = AbsoluteJump(g_Island);
        g_EntryBackup = Dev::Patch(g_Handler, g_EntryJump);
        if (g_EntryBackup != TARGET_ENTRY_ORIGINAL ||
            Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != g_EntryJump) {
            Dev::Patch(g_Handler, g_EntryBackup);
            Dev::Free(g_Island); g_Island = 0;
            g_Status = "entry changed concurrently or patch verification failed";
            return false;
        }
        if (!ApplyFallRuntimeGraph()) {
            if (Dev::Read(g_Handler, ENTRY_PATCH_BYTES) == g_EntryJump) Dev::Patch(g_Handler, g_EntryBackup);
            Dev::Free(g_Island); g_Island = 0;
            return false;
        }
    } catch {
        if (g_FallRuntimePatched) RestoreFallRuntimeGraph();
        if (g_Handler != 0 && g_EntryJump.Length > 0 &&
            Dev::Read(g_Handler, ENTRY_PATCH_BYTES) == g_EntryJump) {
            Dev::Patch(g_Handler, g_EntryBackup);
        }
        if (g_Island != 0) Dev::Free(g_Island);
        g_Island = 0;
        g_Status = "Fall installation exception with rollback: " + getExceptionInfo();
        return false;
    }

    g_Installed = true;
    g_ActiveProfile = PhysicsProfileId::StadiumFall2022;
    g_Status = "Fall 2022 Stadium physics active; offline use only";
    trace("[HistoricalPhysics] ACTIVE Fall 2022 handler="
        + Text::FormatPointer(g_Handler) + " island=" + Text::FormatPointer(g_Island)
        + " tunings=25/28");
    return true;
}

void SelectProfile(PhysicsProfile@ p) {
    if (p is null) return;
    g_SelectedProfile = p.Id;
    if (!p.Selectable) {
        g_Status = p.Name + " is catalogued but not executable in this package: " + p.Mechanism;
    } else {
        g_Status = "selected " + p.Name + "; apply from the main menu";
    }
}

bool UsesInstalledCurrentPhysics(PhysicsProfileId id) {
    return id == PhysicsProfileId::StadiumSummer2023Current ||
        id == PhysicsProfileId::SnowMay2024Current ||
        id == PhysicsProfileId::RallyMay2024Current ||
        id == PhysicsProfileId::DesertMay2024Current;
}

void ApplySelectedProfile() {
    if (!AtSafeSwitchPoint()) return;
    auto selected = Profile(g_SelectedProfile);
    if (selected is null) {
        g_Status = "unknown profile";
        return;
    }
    if (!selected.Selectable) {
        g_Status = selected.Name + " is fail-closed: " + selected.Mechanism;
        return;
    }

    if (IsRallyProfile(g_SelectedProfile)) {
        if (g_SelectedProfile != PhysicsProfileId::RallyMay2024Current &&
            !g_ExperimentalUnlocked) {
            g_Status = "arm Experimental native profiles before applying historical Rally behavior";
            return;
        }
        if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveNativeProfile()) return;
        if (!ValidateRallyItemPathPreimage()) return;
        bool releasePath = WantsRallyReleasePath(g_SelectedProfile);
        bool legacyAnalog = WantsLegacyRallyAnalog(g_SelectedProfile);
        if (!ApplyRallyAxes(releasePath, legacyAnalog)) return;
        g_ActiveProfile = g_SelectedProfile;
        g_Status = ProfileName(g_SelectedProfile) + " active; " +
            (releasePath ? "release custom-ice data" : "post-fix custom-ice data") +
            ", " + (legacyAnalog ? "pre-May analog input" : "current analog input");
        trace("[HistoricalPhysics] ACTIVE " + ProfileName(g_SelectedProfile));
        return;
    }

    if (UsesInstalledCurrentPhysics(g_SelectedProfile)) {
        if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveNativeProfile()) return;
        if (g_SelectedProfile == PhysicsProfileId::SnowMay2024Current &&
            !ValidateSnowCollisionEpoch(false)) return;
        if (!ApplyRallyAxes(false, false)) return;
        g_ActiveProfile = g_SelectedProfile;
        g_Status = ProfileName(g_SelectedProfile) +
            " uses installed current physics and input; choose its official vehicle family when authoring the map";
        return;
    }
    if (g_SelectedProfile == PhysicsProfileId::StadiumJanuary2022) {
        if (!ApplyRallyAxes(false, false)) return;
        InstallJanuary2022();
        return;
    }
    if (g_SelectedProfile == PhysicsProfileId::StadiumFall2022) {
        g_Status = "Fall 2022 is fail-closed: exhaustive native field-layout remapping and live trajectory certification are incomplete";
        return;
    }
    if (IsSnowProfile(g_SelectedProfile)) {
        if (!ApplyRallyAxes(false, false)) return;
        if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled) && !RemoveNativeProfile()) return;
        InstallSnowProfile(g_SelectedProfile);
        return;
    }
    auto p = Profile(g_SelectedProfile);
    g_Status = (p is null ? "unknown profile" : p.Name + " is not executable yet");
}

void Main() {
    g_Profiles = BuildProfileCatalog();
    if (!ValidateBuild()) return;
    PreloadFallRuntimeGraph();
    g_Status = "ready on build 128130; current physics active";
}

void RenderMenu() {
    if (!UI::BeginMenu("Historical Physics Lab")) return;

    UI::Text("Active: " + ProfileName(g_ActiveProfile));
    UI::Text("Selected: " + ProfileName(g_SelectedProfile));
    UI::Separator();

    if (UI::MenuItem("Arm experimental native profiles", "", g_ExperimentalUnlocked)) {
        g_ExperimentalUnlocked = !g_ExperimentalUnlocked;
        g_Status = g_ExperimentalUnlocked
            ? "experimental native profiles armed for this session"
            : "experimental native profiles disarmed";
    }

    if (UI::BeginMenu("Choose engine profile")) {
        for (uint i = 0; i < g_Profiles.Length; i++) {
            auto p = g_Profiles[i];
            string label = p.Selectable ? p.Name : p.Name + " [catalog only]";
            if (UI::MenuItem(label, "", g_SelectedProfile == p.Id)) SelectProfile(p);
        }
        UI::EndMenu();
    }

    if (UI::MenuItem("Apply selected profile", "", false)) ApplySelectedProfile();
    if ((g_Installed || g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled || g_RallyReleasePathInstalled || g_RallyAnalogLegacyInstalled) &&
        UI::MenuItem("Restore current physics now", "", false)) {
        g_SelectedProfile = PhysicsProfileId::StadiumSummer2023Current;
        ApplySelectedProfile();
    }

    UI::Separator();
    auto selected = Profile(g_SelectedProfile);
    if (selected !is null) {
        UI::TextWrapped(selected.Period);
        UI::TextWrapped(selected.Representative);
        UI::TextWrapped(selected.Mechanism);
        UI::TextWrapped(selected.Evidence);
    }
    UI::Separator();
    UI::TextWrapped(g_Status);
    UI::Text("Target: Trackmania build 128130 (2026-01-28)");
    UI::Text("January 2022 island: " + PROFILE_JAN2022_ISLAND_SIZE + " bytes");
    UI::Text("January relocations: " + PROFILE_JAN2022_FIELD_RELOCATION_COUNT
        + " fields, " + PROFILE_JAN2022_CALL_RELOCATION_COUNT + " calls, "
        + PROFILE_JAN2022_RIP_RELOCATION_COUNT + " RIP references");
    UI::Text("Fall 2022 island: " + PROFILE_FALL2022_ISLAND_SIZE + " bytes");
    UI::Text("Fall tuning view: 25 measured entries (current preimage: 28)");
    UI::Text("Fall relocations: " + PROFILE_FALL2022_RELOC_OFFSETS.Length
        + " rel32, " + PROFILE_FALL2022_ABS64_OFFSETS.Length + " absolute thunks");
    RenderRallyProfileControls();
    RenderVehicleFamilySelector();
    UI::EndMenu();
}

void OnDisabled() {
    RestoreRallyProfilesOnUnload();
    RemoveNativeProfile();
}

void OnDestroyed() {
    RestoreRallyProfilesOnUnload();
    RemoveNativeProfile();
}
