bool g_OfficialVehicleSelfTestDone = false;
uint g_OfficialVehicleSelfTestStart = 0;

void Update(float dt) {
    if (g_OfficialVehicleSelfTestDone) return;
    if (g_OfficialVehicleSelfTestStart == 0) g_OfficialVehicleSelfTestStart = Time::Now;
    if (Time::Now - g_OfficialVehicleSelfTestStart < 3000) return;
    ScanVehicleFamilies(true);
    trace("[OfficialVehicleSelfTest] COUNT=" + g_VehicleNames.Length);
    for (uint i = 0; i < g_VehicleNames.Length; i++) {
        trace("[OfficialVehicleSelfTest] FAMILY=" + g_VehicleNames[i]);
        if (g_VehicleNames[i].ToLower().Contains("snow")) g_SelectedVehicle = int(i);
    }
    auto editor = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (editor is null || g_SelectedVehicle < 0) return;
    bool ok = ApplyVehicleFamily();
    uint title = MapTitleIdOffset();
    uint got = title >= 0x18 ? Dev::GetOffsetUint32(editor.Challenge, title - 0x18) : 0;
    trace("[OfficialVehicleSelfTest] APPLY=" + (ok ? "PASS" : "FAIL")
        + " expected=" + g_VehicleIds[g_SelectedVehicle] + " got=" + got);
    g_OfficialVehicleSelfTestDone = true;
}
