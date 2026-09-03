uint g_IntegratedTestStart = 0;
int g_IntegratedTestPhase = 0;
bool g_IntegratedTestDone = false;

void TestPass(const string &in name) {
    trace("[IntegratedSelfTest] PASS " + name + " status=" + g_Status);
}

void TestFail(const string &in name) {
    error("[IntegratedSelfTest] FAIL " + name + " status=" + g_Status);
    g_IntegratedTestDone = true;
}

void Update(float dt) {
    if (g_IntegratedTestDone) return;
    if (g_IntegratedTestStart == 0) g_IntegratedTestStart = Time::Now;
    if (Time::Now - g_IntegratedTestStart < uint(8000 + g_IntegratedTestPhase * 2500)) return;
    if (!AtSafeSwitchPoint()) return;

    g_ExperimentalUnlocked = true;
    if (g_IntegratedTestPhase == 0) {
        array<string> rallyFidPaths = {
            "GameData/Vehicles/Items/Cars/CarRally.Item.Gbx",
            "Vehicles/Items/Cars/CarRally.Item.Gbx",
            "/Vehicles/Items/Cars/CarRally.Item.Gbx"
        };
        for (uint i = 0; i < rallyFidPaths.Length; i++) {
            auto fid = Fids::GetGame(rallyFidPaths[i]);
            if (fid !is null) trace("[IntegratedSelfTest] RALLY_FID path=" + rallyFidPaths[i]
                + " size=" + fid.ByteSize + " sizeEd=" + fid.ByteSizeEd
                + " full=" + Fids::GetFullPath(fid));
        }
        bool snowOk = ValidateSnowTargetSites();
        string snowStatus = g_Status;
        bool rallyOk = ValidateRallyProfileControls(true);
        string rallyStatus = g_Status;
        trace("[IntegratedSelfTest] DIAG current snow=" + snowOk + " status=" + snowStatus
            + " rally=" + rallyOk + " status=" + rallyStatus);
        if (!snowOk || !rallyOk) {
            TestFail("current preimages"); return;
        }
        TestPass("current preimages");
    } else if (g_IntegratedTestPhase == 1) {
        auto jan = Profile(PhysicsProfileId::StadiumJanuary2022);
        auto fall = Profile(PhysicsProfileId::StadiumFall2022);
        auto snow = Profile(PhysicsProfileId::SnowFebruary2024);
        auto rally = Profile(PhysicsProfileId::RallyApril2024);
        string originalEntry = Dev::Read(g_Handler, ENTRY_PATCH_BYTES);
        if (jan is null || fall is null || snow is null || rally is null ||
            jan.Selectable || fall.Selectable || snow.Selectable || rally.Selectable) {
            TestFail("historical catalog gates"); return;
        }
        if (InstallJanuary2022() || InstallFall2022() ||
            InstallSnowProfile(PhysicsProfileId::SnowFebruary2024)) {
            TestFail("direct historical install gates"); return;
        }
        g_SelectedProfile = PhysicsProfileId::RallyApril2024;
        ApplySelectedProfile();
        if (Dev::Read(g_Handler, ENTRY_PATCH_BYTES) != originalEntry || g_Installed ||
            g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled ||
            g_RallyReleasePathInstalled || g_RallyAnalogLegacyInstalled ||
            g_ActiveProfile != PhysicsProfileId::StadiumSummer2023Current) {
            TestFail("historical fail-closed state"); return;
        }
        TestPass("all historical profiles fail closed");
    } else if (g_IntegratedTestPhase == 2) {
        g_SelectedProfile = PhysicsProfileId::SnowMay2024Current;
        ApplySelectedProfile();
        if (g_SnowCodeInstalled || g_SnowHistoricalCollisionInstalled ||
            g_ActiveProfile != PhysicsProfileId::SnowMay2024Current) {
            TestFail("Snow current"); return;
        }
        g_SelectedProfile = PhysicsProfileId::RallyMay2024Current;
        ApplySelectedProfile();
        if (g_RallyReleasePathInstalled || g_RallyAnalogLegacyInstalled ||
            g_ActiveProfile != PhysicsProfileId::RallyMay2024Current) {
            TestFail("Rally current"); return;
        }
        g_SelectedProfile = PhysicsProfileId::StadiumSummer2023Current;
        ApplySelectedProfile();
        if (g_ActiveProfile != PhysicsProfileId::StadiumSummer2023Current) {
            TestFail("Stadium current"); return;
        }
        TestPass("installed-current profiles");
        trace("[IntegratedSelfTest] ALL PASS");
        g_IntegratedTestDone = true;
        return;
    }
    g_IntegratedTestPhase++;
}
