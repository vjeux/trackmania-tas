// Cursor.as -- hide the editor's cursor before a screenshot.
//
// THE CAR IN THE FRAME (2026-09-09). Drive-through comparison frames of the tiny
// maps showed, in SOME shots and not others, a "grey road slab hanging over the
// track", a "white Tech slab with a red LED strip and the TM logo across the
// grass" — read at pixel level, the rear of the Stadium car: wing, LED tail bar,
// mirrored TM letters on the roof. It was the EDITOR'S CURSOR PREVIEW: when a
// block or item is selected in the inventory, its preview mesh sits at the
// cursor, and the cursor projects from the screen centre onto the grid, i.e.
// near the camera target; the start block's preview carries a car. Whether it
// showed depended on what the previous user of the box had selected and on
// whether the cursor could be placed at the target (an occupied cell hides it —
// which is why the ORIGINAL, whose cells are full of blocks, never showed it and
// the tiny, whose cells hold items, did). Half a night of "hidden records" was
// chased on that car.
//
// The fix: FreeLook edit mode has no cursor. `/freelook` switches to it and
// reports what was selected; `shootctl shootset` calls it after every map load,
// before the first camera. Camera aiming (the probe plugin's cam.txt →
// PluginMapType.Camera* / OrbitalCameraControl) works the same in FreeLook.
string FreeLook() {
    auto ed = cast<CGameCtnEditorFree>(GetApp().Editor);
    if (ed is null) return "{\"editor\":null}";
    string blockName = "";
    if (ed.CurrentBlockInfo !is null) blockName = ed.CurrentBlockInfo.Name;
    string itemName = "";
    if (ed.CurrentItemModel !is null) itemName = ed.CurrentItemModel.IdName;
    string before = "";
    string after = "";
    string err = "";
    if (ed.PluginMapType !is null) {
        before = tostring(ed.PluginMapType.EditMode);
        try {
            ed.PluginMapType.EditMode = CGameEditorPluginMap::EditMode::FreeLook;
        } catch {
            err = getExceptionInfo();
        }
        after = tostring(ed.PluginMapType.EditMode);
    } else {
        err = "no PluginMapType";
    }
    return "{\"before\":\"" + before + "\",\"after\":\"" + after + "\",\"block\":\"" + blockName + "\",\"item\":\"" + itemName + "\",\"error\":\"" + err + "\"}";
}
