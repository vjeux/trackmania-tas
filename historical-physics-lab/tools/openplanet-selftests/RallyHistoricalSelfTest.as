uint g_RallyHistoricalStart = 0;
bool g_RallyHistoricalDone = false;

void Update(float dt) {
    if (g_RallyHistoricalDone) return;
    if (g_RallyHistoricalStart == 0) g_RallyHistoricalStart = Time::Now;
    if (Time::Now - g_RallyHistoricalStart < 8000) return;
    if (!AtSafeSwitchPoint()) return;
    g_ExperimentalUnlocked = true;
    g_SelectedProfile = PhysicsProfileId::RallyRelease2024;
    ApplySelectedProfile();
    if (g_ActiveProfile != PhysicsProfileId::RallyRelease2024 ||
        !g_RallyReleasePathInstalled || !g_RallyAnalogLegacyInstalled) {
        error("[RallyHistoricalSelfTest] FAIL install status=" + g_Status);
        g_RallyHistoricalDone = true;
        return;
    }
    trace("[RallyHistoricalSelfTest] PASS install status=" + g_Status);
    if (!SetRallyLegacyAnalog(false)) {
        error("[RallyHistoricalSelfTest] FAIL analog restore status=" + g_Status);
    } else {
        trace("[RallyHistoricalSelfTest] PASS analog restore");
        trace("[RallyHistoricalSelfTest] ALL PASS");
    }
    g_RallyHistoricalDone = true;
}
