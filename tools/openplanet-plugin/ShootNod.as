// ShootNod.as -- everything we know about reaching the shoot dialog from
// script, including the two routes that DO NOT work, so nobody spends the
// afternoon on them again.
//
// The dialog on screen ("SHOOT THE VIDEO TO A FILE": Shoot Name, Quality
// Preset, Width, Height, FPS, File format, OK / CANCEL) is a
// CGameDialogShootParams, and that nod carries the OnOk we want to call.
//
// ROUTE 1 -- a typed member. DEAD. Nothing in the game's entire class dump
//   declares a member of type CGameDialogShootParams@. Checked by grepping
//   OpenplanetNext.json for "CGameDialogShootParams@": zero hits. So no walk
//   over declared members can reach it, at any depth.
//
// ROUTE 2 -- the control tree's data binding. DEAD, but worth knowing about:
//   CControlBase carries Nod (the CMwNod a control is bound to) and OnAction
//   (its own activation), and CGameDialogs::Dialogs is a CGameMenu whose Frames
//   array holds EVERY dialog frame -- not just CurrentFrame, which reads null
//   while the shoot dialog is on screen. That is a real mechanism and it is
//   implemented below. It finds nothing here: the nine basic frames
//   (AskYesNo, AskString, Message, MessageAvatar, DialogSaveAs, Throbber,
//   WaitMessage, DialogPopUp, AskYesNoCancel) hold one quad each and no bound
//   nods, and the map editor's UISuperRoot has no children while the
//   MediaTracker is open.
//
// ROUTE 3 -- a manialink UI layer. DEAD. Every reachable CGameManiaApp was
//   enumerated (the menu's CGameManiaAppTitle with 77 layers, and the map
//   editor's CGameEditorPluginMap, which DERIVES from CGameManiaApp, off the
//   switcher module stack). Grepping all of their pages for "Shoot Name",
//   "Quality Preset", "Preset", "Webm", "VideoFps" finds the replay menu's
//   Shoot verb and nothing else. The dialog is not a manialink page.
//
// ROUTE 4 -- an in-process memory scan. DEAD, AND IT KILLS THE GAME.
//   This was tried properly, not blindly: candidates were filtered to aligned
//   pointers whose target begins with a vtable inside the game module
//   (Dev::BaseAddress()..BaseAddressEnd()), read through Dev::SafeReadUInt64,
//   with the walk bounded by Reflection::MwClassInfo::Size. Two findings:
//     * Dev::SafeReadUInt64 does NOT return 0 on unmapped memory -- it THROWS
//       "Unable to read memory", which aborts the request but not the game.
//     * Making the scan resumable across those faults got further and then
//       HARD-CRASHED the process (2026-08-22, ~800 nods in from GetApp()).
//   DO NOT REINTRODUCE A MEMORY SCAN FOR NODS. Two independent attempts, two
//   dead games. The vtable pre-check is not sufficient.

CGameMenu@ DialogMenu() {
    auto dlg = GetApp().BasicDialogs;
    if (dlg is null) return null;
    return dlg.Dialogs;
}

// EVERY CONTROL ROOT THE GAME HAS RIGHT NOW. Opening the in-game MediaTracker
// PUSHES a CGameEditorMediaTracker module over the CGameCtnEditorFree that
// still holds the interface, so the roots come off the whole switcher stack,
// not just GetApp().Editor.
void CollectRoots(array<CControlBase@> &out roots, string &out sb) {
    auto app = GetApp();
    auto menu = DialogMenu();
    if (menu !is null) {
        for (uint i = 0; i < menu.Frames.Length; i++)
            if (menu.Frames[i] !is null) roots.InsertLast(menu.Frames[i]);
        sb += "basicdialogs frames: " + menu.Frames.Length + "\n";
    }
    auto sw = app.Switcher;
    if (sw !is null) {
        sb += "modules: " + sw.ModuleStack.Length + "\n";
        for (uint i = 0; i < sw.ModuleStack.Length; i++) {
            auto m = sw.ModuleStack[i];
            if (m is null) { sb += "  [" + i + "] null\n"; continue; }
            auto ti = Reflection::TypeOf(m);
            sb += "  [" + i + "] " + ((ti is null) ? "?" : ti.Name);
            auto fr = cast<CGameCtnEditorFree>(m);
            if (fr !is null && fr.EditorInterface !is null && fr.EditorInterface.InterfaceRoot !is null) {
                roots.InsertLast(fr.EditorInterface.InterfaceRoot);
                sb += "  +InterfaceRoot";
            }
            sb += "\n";
        }
    }
    auto pg = app.CurrentPlayground;
    if (pg !is null && pg.Interface !is null && pg.Interface.InterfaceRoot !is null) {
        roots.InsertLast(pg.Interface.InterfaceRoot);
        sb += "playground InterfaceRoot\n";
    }
}

CMwNod@ FindBoundNod(CControlBase@ c, const string &in wanted, int depth, int maxDepth) {
    if (c is null || depth > maxDepth) return null;
    auto n = c.Nod;
    if (n !is null) {
        auto ti = Reflection::TypeOf(n);
        if (ti !is null && ti.Name == wanted) return n;
    }
    auto cont = cast<CControlContainer>(c);
    if (cont is null) return null;
    for (uint i = 0; i < cont.Childs.Length; i++) {
        auto r = FindBoundNod(cont.Childs[i], wanted, depth + 1, maxDepth);
        if (r !is null) return r;
    }
    return null;
}

CMwNod@ FindBoundNodAnywhere(const string &in wanted) {
    array<CControlBase@> roots;
    string sb = "";
    CollectRoots(roots, sb);
    for (uint i = 0; i < roots.Length; i++) {
        auto r = FindBoundNod(roots[i], wanted, 0, 18);
        if (r !is null) return r;
    }
    return null;
}

// The dump behind all of the above: every control under every root, with the
// runtime type of the nod it is bound to. needle "*" shows everything, "" shows
// only controls that carry a nod.
void WalkBound(CControlBase@ c, int depth, const string &in needle, string &out sb, int maxDepth) {
    if (c is null || depth > maxDepth) return;
    auto n = c.Nod;
    string tn = "-";
    if (n !is null) {
        auto ti = Reflection::TypeOf(n);
        tn = (ti is null) ? "?" : ti.Name;
    }
    bool hit = (needle == "*")
            || (n !is null && (needle == "" || tn.IndexOf(needle) >= 0))
            || (needle != "" && needle != "*" && c.IdName.IndexOf(needle) >= 0);
    if (hit) {
        string pad = "";
        for (int i = 0; i < depth; i++) pad += "  ";
        auto cti = Reflection::TypeOf(c);
        sb += pad + c.IdName + " [" + ((cti is null) ? "?" : cti.Name) + "]"
            + " vis=" + (c.IsHiddenExternal ? "0" : "1")
            + " -> " + tn + "\n";
    }
    auto cont = cast<CControlContainer>(c);
    if (cont is null) return;
    for (uint i = 0; i < cont.Childs.Length; i++)
        WalkBound(cont.Childs[i], depth + 1, needle, sb, maxDepth);
}

string DialogNods(const string &in needle) {
    array<CControlBase@> roots;
    string sb = "";
    CollectRoots(roots, sb);
    for (uint i = 0; i < roots.Length; i++) {
        auto r = roots[i];
        string body = "";
        WalkBound(r, 0, needle, body, 18);
        sb += "== root[" + i + "] " + r.IdName + "\n" + body;
    }
    return sb;
}

// The shoot dialog, if any route ever reaches it. Route 2 is the only one left
// standing and it does not find it today; it is kept because it costs nothing
// and it is how the game would hand the dialog over if a patch ever binds it.
CGameDialogShootParams@ ShootParams() {
    return ShootDialogNod();
}

CGameDialogShootParams@ ShootParamsLegacy() {
    return cast<CGameDialogShootParams>(FindBoundNodAnywhere("CGameDialogShootParams"));
}

// ---------------------------------------------------------------------------
// IS A MODAL DIALOG UP? A NUMBER, from the game.
//
// The shoot dialog is not under BasicDialogs, so /ctx reports dialog:null while
// it is plainly on screen -- and the old driver "waited" for the string
// "FrameDialogSaveAs", which is a DIFFERENT dialog left over from the ghost
// import, so the wait passed instantly and the accept went in before the dialog
// existed. That is the bug behind "the click landed on the Import Ghosts dialog
// behind". CGameSwitcher::FocusDialogCount is the game's own count of modal
// dialogs holding focus, so 0 -> 1 is a real signal and 1 -> 0 is the accept.
string FocusDialogs() {
    auto sw = GetApp().Switcher;
    if (sw is null) return "{\"err\":\"no Switcher\"}";
    return "{\"focusdialogs\":" + sw.FocusDialogCount + "}";
}

// ---------------------------------------------------------------------------
// THERE ARE FOUR DIALOG MENUS, NOT ONE.
//
// Everything above searched GetApp().BasicDialogs.Dialogs -- and that is only
// the BASIC dialog menu. CGameCtnMenus (the MenuManager) carries FOUR
// CGameMenu instances: Menus, InGameDialogs, Dialogs and SystemDialogs, which
// line up with CGameMenu::EMenuOrder (Menu, InGameMenu, GameDialog,
// SystemDialog, BasicDialog). The shoot dialog is a GAME dialog, so it was
// never going to be in the basic one.
//
// CGameMenu also carries CurrentFocusedControl -- so once the right menu is in
// hand, the focused control is readable, which is the thing the keystroke
// approach is flying blind about.
array<CGameMenu@> AllMenus(array<string> &out names) {
    array<CGameMenu@> res;
    auto dlg = GetApp().BasicDialogs;
    if (dlg !is null && dlg.Dialogs !is null) { res.InsertLast(dlg.Dialogs); names.InsertLast("basic"); }
    auto mp = MPl();
    if (mp !is null) {
        auto mm = mp.MenuManager;
        if (mm !is null) {
            if (mm.Menus !is null)          { res.InsertLast(mm.Menus);          names.InsertLast("menus"); }
            if (mm.InGameDialogs !is null)  { res.InsertLast(mm.InGameDialogs);  names.InsertLast("ingame"); }
            if (mm.Dialogs !is null)        { res.InsertLast(mm.Dialogs);        names.InsertLast("game"); }
            if (mm.SystemDialogs !is null)  { res.InsertLast(mm.SystemDialogs);  names.InsertLast("system"); }
        }
    }
    return res;
}

string MenuReport() {
    array<string> names;
    auto menus = AllMenus(names);
    string sb = "";
    for (uint i = 0; i < menus.Length; i++) {
        auto m = menus[i];
        sb += names[i] + ": frames=" + m.Frames.Length
            + " order=" + int(m.MenuOrder)
            + " current=" + ((m.CurrentFrame is null) ? "null" : m.CurrentFrame.IdName)
            + " focus=" + ((m.CurrentFocusedControl is null) ? "null" : m.CurrentFocusedControl.IdName);
        if (m.CurrentFocusedControl !is null) {
            auto ti = Reflection::TypeOf(m.CurrentFocusedControl);
            sb += " [" + ((ti is null) ? "?" : ti.Name) + "]";
            auto n = m.CurrentFocusedControl.Nod;
            if (n !is null) {
                auto nti = Reflection::TypeOf(n);
                sb += " -> " + ((nti is null) ? "?" : nti.Name);
            }
        }
        sb += "\n";
    }
    return sb;
}

// Every control under every menu frame, with the nod it is bound to.
string MenuTree(const string &in needle) {
    array<string> names;
    auto menus = AllMenus(names);
    string sb = "";
    for (uint i = 0; i < menus.Length; i++) {
        auto m = menus[i];
        for (uint f = 0; f < m.Frames.Length; f++) {
            auto fr = m.Frames[f];
            if (fr is null) continue;
            string body = "";
            WalkBound(fr, 0, needle, body, 18);
            if (body == "") continue;
            sb += "== " + names[i] + "[" + f + "] " + fr.IdName + "\n" + body;
        }
    }
    if (sb == "") sb = "nothing matching \"" + needle + "\"\n";
    return sb;
}

// ---------------------------------------------------------------------------
// THE SHOOT DIALOG, FOUND. 2026-08-22.
//
//   game: frames=43 current=FrameDialogShootVideo
//         focus=EnumFileFormat [CControlEnum] -> CGameDialogShootParams
//
// It is a frame of the GAME dialog menu (CGameCtnMenus::Dialogs, MenuOrder 5),
// and its controls are bound to the CGameDialogShootParams. Every earlier
// search failed for the same reason: they looked in BasicDialogs, which is a
// DIFFERENT CGameMenu (MenuOrder 11).
//
// This also explains the keystroke damage. The dialog does NOT open with OK
// focused -- it opens on EnumFileFormat, so an Enter sent at it cycles the file
// format, which is exactly how the output silently became AVI.
CGameDialogShootParams@ ShootDialogNod() {
    array<string> names;
    auto menus = AllMenus(names);
    for (uint i = 0; i < menus.Length; i++) {
        auto m = menus[i];
        // the focused control is the cheapest route in
        if (m.CurrentFocusedControl !is null) {
            auto sp = cast<CGameDialogShootParams>(m.CurrentFocusedControl.Nod);
            if (sp !is null) return sp;
        }
        // otherwise walk the frame the menu says is current
        if (m.CurrentFrame !is null) {
            auto n = FindBoundNod(m.CurrentFrame, "CGameDialogShootParams", 0, 18);
            if (n !is null) return cast<CGameDialogShootParams>(n);
        }
    }
    return null;
}
