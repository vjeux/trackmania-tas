# Gate patch — stop January installing

Three edits. The first is the one that matters; apply it before anything else.

## 1. `Main.as` — refuse the install, the way Fall is refused

```diff
 bool InstallJanuary2022() {
+    if (!PROFILE_JAN2022_BEHAVIOR_CERTIFIED || !PROFILE_JAN2022_STATIC_COMPLETE) {
+        g_Status = "January 2022 is fail-closed: 26 structure accesses use January field"
+                   " offsets against the current layout, including three writes into the"
+                   " live object; behavior is uncertified";
+        return false;
+    }
     if (!g_ExperimentalUnlocked) {
         g_Status = "arm Experimental native profiles first";
         return false;
     }
```

Both constants are emitted `false` by the corrected profile. With the shipped
profile they are absent, so add them there too rather than relying on their
absence.

## 2. `Profiles.as` — the catalog entry must not be selectable

```diff
     profiles.InsertLast(PhysicsProfile(PhysicsProfileId::StadiumJanuary2022,
         "Stadium — January 2022", "representative pre-2022-03-29 profile",
         "Client build 105899, 2022-01-21; dynamic boundary anchor: server build 112349, 2022-03-25.",
         "Executable-side native island including the January initializer, removed helpers, legacy curve wrappers, and historical defaults.",
-        "Static closure is complete and authoritative March boundary controls pass; deterministic client-side semantic certification is still pending.",
-        true, true));
+        "Static closure is NOT complete: an independent audit found 26 structure accesses left at January offsets against the current layout (16 proven, 3 of them writes into the live object). The March boundary control is a server-side result and says nothing about this client payload. Fail-closed pending the January executable and a matched live-map trajectory test.",
+        false, true));
```

`Main.as` already refuses a non-selectable profile at `if (!selected.Selectable)`,
so this closes the catalog path as well as the direct one.

## 3. `profiles.json` — the epoch status is wrong

```diff
       "id": "STADIUM_PRE_2022_03_29",
       "period": "through 2022-03-25",
-      "status": "experimental_selectable",
+      "status": "fail_closed_defective",
```

`STADIUM_PRE_2022_03_29` is the only Stadium epoch other than the installed
current one that is not already fail-closed. Spring and Fall are
`catalog_only`; January should join them until the audit's residual is closed.

## 4. `verify_release.rs` — make the release gate able to fail

The current January block asserts the generator's own counts as literal
strings, so it cannot detect an incomplete payload. Replace the count assertions
with the two gate assertions, mirroring what it already does for Fall:

```diff
-        "PROFILE_JAN2022_FIELD_RELOCATION_COUNT = 161",
-        "PROFILE_JAN2022_CALL_RELOCATION_COUNT = 105",
-        "PROFILE_JAN2022_RIP_RELOCATION_COUNT = 83",
+        "PROFILE_JAN2022_BEHAVIOR_CERTIFIED = false",
+        "PROFILE_JAN2022_STATIC_COMPLETE = false",
...
+    must(&main_as, "if (!PROFILE_JAN2022_BEHAVIOR_CERTIFIED", "direct January install safety gate");
```

## 5. Generator, for the next iteration

`januarygen.rs` must gain what `remap_fall_island.rs` already has:

* fail on `field_failed > 0` instead of printing it to stderr and continuing;
* an exhaustive per-instruction ModRM sweep over every copied region, with an
  explicit `PROVEN_UNCHANGED_MODRM_COUNT`, so a site left unremapped is a
  recorded decision rather than an omission;
* an immediate audit with `AUDITED_IMMEDIATE_COUNT` /
  `PROVEN_UNCHANGED_IMMEDIATE_COUNT`;
* carriage of all of those into the payload, so the release gate can see them.
