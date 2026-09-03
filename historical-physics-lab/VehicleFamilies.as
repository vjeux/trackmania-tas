// Vehicle families are orthogonal to Stadium physics epochs. This selector uses
// the same GlobalCatalog source and map-ident fields as Editor++, and only edits
// a map already open in the editor. The loose GameData resources must exist.

array<uint> g_VehicleIds;
array<uint> g_VehicleAuthors;
array<uint> g_VehicleCollections;
array<string> g_VehicleNames;
int g_SelectedVehicle = -1;
bool g_VehiclesScanned = false;

bool IsKnownVehicleFamily(const string &in name) {
    string n = name.ToLower();
    return n.Contains("sport") || n.Contains("stadium") || n.Contains("snow") ||
        n.Contains("rally") || n.Contains("desert");
}

void ScanVehicleFamilies(bool force = false) {
    if (g_VehiclesScanned && !force) return;
    g_VehicleIds.RemoveRange(0, g_VehicleIds.Length);
    g_VehicleAuthors.RemoveRange(0, g_VehicleAuthors.Length);
    g_VehicleCollections.RemoveRange(0, g_VehicleCollections.Length);
    g_VehicleNames.RemoveRange(0, g_VehicleNames.Length);
    g_SelectedVehicle = -1;

    auto catalog = GetApp().GlobalCatalog;
    for (uint i = 0; i < catalog.Chapters.Length; i++) {
        auto chapter = catalog.Chapters[i];
        if (!(chapter.IdName == "Vehicles" || chapter.IdName == "#10003")) continue;
        for (uint j = 0; j < chapter.Articles.Length; j++) {
            auto article = chapter.Articles[j];
            if (article is null || !IsKnownVehicleFamily(article.Name)) continue;
            g_VehicleIds.InsertLast(article.Id.Value);
            g_VehicleAuthors.InsertLast(article.IdentAuthor.Value);
            g_VehicleCollections.InsertLast(chapter.Id.Value);
            g_VehicleNames.InsertLast(article.Name);
            trace("[HistoricalPhysics] VEHICLE FOUND " + article.Name
                + " id=" + article.Id.Value + " collection=" + chapter.Id.Value);
        }
    }
    g_VehiclesScanned = true;
    g_Status = "discovered " + g_VehicleNames.Length + " installed vehicle-family article(s)";
}

uint MapTitleIdOffset() {
    auto type = Reflection::GetType("CGameCtnChallenge");
    if (type is null) return 0;
    return type.GetMember("TitleId").Offset;
}

bool ApplyVehicleFamily() {
    auto editor = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (editor is null || editor.Challenge is null) {
        g_Status = "open a map in the editor before changing its vehicle family";
        return false;
    }
    if (g_SelectedVehicle < 0 || uint(g_SelectedVehicle) >= g_VehicleIds.Length) {
        g_Status = "select an installed vehicle family first";
        return false;
    }
    uint title = MapTitleIdOffset();
    if (title < 0x18) {
        g_Status = "CGameCtnChallenge layout is unavailable; map was not changed";
        return false;
    }
    // Relative to TitleId, matching the map-ident layout used by Editor++:
    // player model -0x18, collection -0x14, author -0x10.
    Dev::SetOffset(editor.Challenge, title - 0x18, g_VehicleIds[g_SelectedVehicle]);
    Dev::SetOffset(editor.Challenge, title - 0x14, g_VehicleCollections[g_SelectedVehicle]);
    Dev::SetOffset(editor.Challenge, title - 0x10, g_VehicleAuthors[g_SelectedVehicle]);
    g_Status = "map vehicle set to " + g_VehicleNames[g_SelectedVehicle]
        + "; save as a new map and ensure compatible car gates exist";
    trace("[HistoricalPhysics] MAP VEHICLE " + g_VehicleNames[g_SelectedVehicle]);
    return true;
}

void RenderVehicleFamilySelector() {
    UI::Separator();
    UI::Text("Vehicle family (map authoring)");
    UI::TextWrapped("Official current vehicles only.");
    if (UI::MenuItem("Scan installed vehicle families", "", false)) ScanVehicleFamilies(true);
    if (!g_VehiclesScanned) ScanVehicleFamilies(false);

    if (g_VehicleNames.Length == 0) {
        UI::TextWrapped("No vehicle articles found. Install the official/restored GameData resources first.");
        return;
    }

    string preview = g_SelectedVehicle >= 0 ? g_VehicleNames[g_SelectedVehicle] : "Choose installed family";
    if (UI::BeginCombo("Vehicle family", preview)) {
        for (uint i = 0; i < g_VehicleNames.Length; i++) {
            if (UI::Selectable(g_VehicleNames[i], g_SelectedVehicle == int(i))) g_SelectedVehicle = int(i);
        }
        UI::EndCombo();
    }
    if (UI::MenuItem("Apply family to open map", "", false)) ApplyVehicleFamily();
    UI::TextWrapped("Official vehicles only: Stadium, Snow, Rally, and Desert.");
}
