# Rally profile payload provenance

These two `CarRally.Item.Gbx` files are evidence for the confirmed April 2, 2024 custom-ice boundary. The release payload is also the exact source for the transactional pre-launch override; the post-fix payload is retained as the adjacent-data control.

| Payload | Representative | SHA-256 | Behavior axis |
|---|---|---|---|
| `rally-release/CarRally.Item.Gbx` | 2024-02-26 release; byte-identical on 2024-03-19 | `a1d5cdcd21ed4b152ae18b9f94dd8fa4f3eb4375d0035a83c20923a251bccd9a` | `SkinDirNameCustom = Models\RallyCar\` |
| `rally-post-custom-ice/CarRally.Item.Gbx` | 2024-04-30 post-fix snapshot | `7cf6976abe68c8910d2cda6b504aec4c40ec90fc1561e27446b97313c9c80868` | `SkinDirNameCustom = Models\CarRally\` |

The encrypted `Vehicles\Cars\CarRally\B191D2177AEA3017BE17C0D1694BF68515` physics-model payload is byte-identical before and after the boundary (stored size 5,696; SHA-256 `a0a4c1fe0e771ca75053badd1b0b78de0dfe0f98fedcd2cb73d583020700734e`). This is a negative control against attributing the fix to a changed tire or engine model.

The separate May 22 analog-input profile is represented in `../Profile_RallyAnalog.as`. It suppresses the exact six-byte `Xi_GatherLatestInputs` snap-to-target store on build 128130. It does not alter Rally data.
