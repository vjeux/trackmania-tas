uint g_SnowHistoricalStart = 0;
bool g_SnowHistoricalDone = false;

void Update(float dt) {
    if (g_SnowHistoricalDone) return;
    if (g_SnowHistoricalStart == 0) g_SnowHistoricalStart = Time::Now;
    if (Time::Now - g_SnowHistoricalStart < 8000) return;
    if (!AtSafeSwitchPoint()) return;
    g_ExperimentalUnlocked = true;
    g_SelectedProfile = PhysicsProfileId::SnowRelease2023;
    ApplySelectedProfile();
    if (g_ActiveProfile != PhysicsProfileId::SnowRelease2023 || !g_SnowCodeInstalled) {
        error("[SnowHistoricalSelfTest] FAIL install status=" + g_Status);
        g_SnowHistoricalDone = true;
        return;
    }
    trace("[SnowHistoricalSelfTest] PASS install status=" + g_Status);
    if (!RemoveNativeProfile()) {
        error("[SnowHistoricalSelfTest] FAIL code restore status=" + g_Status);
    } else {
        trace("[SnowHistoricalSelfTest] PASS code restore");
        trace("[SnowHistoricalSelfTest] ALL PASS");
    }
    g_SnowHistoricalDone = true;
}
