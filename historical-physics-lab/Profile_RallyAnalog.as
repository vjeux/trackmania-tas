// Shared 2024-05-22 analog smooth-steering fix, isolated from Rally physics.
// Current snaps a converged smoothed input to its exact target; the historical
// profiles suppress that one store and preserve the existing control flow.
const uint64 RALLY_ANALOG_LEGACY_PATCH_RVA = 0x2C360E;
const uint RALLY_ANALOG_LEGACY_PATCH_BYTES = 6;
const string RALLY_ANALOG_LEGACY_PATCH_PREIMAGE = "F3 0F 11 64 8D 74";
const string RALLY_ANALOG_LEGACY_PATCH_REPLACEMENT = "90 90 90 90 90 90";
const string RALLY_ANALOG_LEGACY_CONTEXT_PATTERN =
    "0F 93 C0 85 C0 0F 84 7C 00 00 00 F3 0F 11 64 8D 74 E9 58 01 00 00";
