# Snow collision overlay verification

## Payload identity

- Logical override: `GameData\Vehicles\Cars\CarSnow\SnowCar.Shape.Gbx`
- GBX class: `0x0900C000` (`CPlugSurface`)
- Historical size: 1,123 bytes
- Historical SHA-256: `82a0822220468e50f78b372c840fe2c01fee9cc017a712b3f926548668841661`
- First post-Feb size/hash: 1,147 bytes / `ef0ebee29e98faec02c5e563c99688fb664b9b76314eee20c77aef8c9c048d9d`
- Supported current size/hash: 1,151 bytes / `7ea1385e37ecaa3005939bd1d38608f5a36e276da4f81d3efc9079cfa13a68cb`

The pre/post root `Vehicles\Items\Cars\CarSnow.Item.Gbx` files are byte-identical (1,900 bytes, SHA-256 `1f7b1bc03a67d7cfde6917857f81f6e4ef9046385cbe3dc9bccf093a0e65e64c`) and are retained as the negative control.

## Semantic geometry control

`tools/surface_inspect.rs` decodes the `CPlugSurface` compound. Pre-Feb, its first three primitives are spheres with radii `1.195428014`, `0.969449997`, and `1.119449973`. Post-Feb they are ellipsoids with axes `[1.195, 1.1, 1.195]`, `[0.969, 0.8, 1.2]`, and `[1.119, 1.0, 1.119]`. The four radius-`0.47` wheel spheres are identical. This is a direct collision-geometry delta.

## Installer controls

`tools/snow_collision_overlay.rs` was compiled on a devserver and exercised against a fixture containing the exact build-128130 executable (SHA-256 `3fc7d8cda542beda131c44306b123f4004d07d7e22f512b46b762afc29f6edda`).

Controls passed:

1. install preserved an unrelated pre-existing loose file as `.hpl-backup`;
2. installed payload readback matched the historical SHA-256;
3. a second install was idempotent;
4. restore removed the owned payload and restored the prior file byte-for-byte;
5. a one-byte-corrupted installed file was treated as unowned and restore refused to delete it;
6. an unsupported executable hash was rejected before creating the override;
7. after a successful install, replacing `Trackmania.exe` with a different build did not block recovery: restore still removed the owned payload and restored the prior loose file byte-for-byte.

The Openplanet profile gate independently expects a live `SnowCar.Shape.Gbx` FID size of 1,123 for Release/January and 1,151 for February/current. Therefore a missing, stale, or unresolved loose override fails before any code patch.

## Live extraction control

On the supported client launched through Steam, `Fids::Extract` produced:

- `PhyModelSnow.VehiclePhyModel.Gbx`: 635 bytes, SHA-256 `b0a4341913faf8ad71340faa655a7b5d0ff66603a48216c1ae2ecb248687a8a6`;
- `TuningsSnow.VehicleTunings.Gbx`: 10,170 bytes, SHA-256 `db1f5d70f079d263ce290f0ea8f27606fe254a8691279fb1f97e7ab1700e50a8` (hook method);
- `SnowCar.Shape.Gbx`: 1,151 bytes, SHA-256 `7ea1385e37ecaa3005939bd1d38608f5a36e276da4f81d3efc9079cfa13a68cb`;
- `CarSnow.Item.Gbx`: 2,156 bytes, SHA-256 `564190a8d886f802a149e8758d8ec835fe35009e50e6e5f683a0393d5f980c23`.

A live pre-launch loose-override run was not performed in this extraction window. The implementation remains fail-closed: if the game does not resolve the loose shape as the 1,123-byte FID, Release and January cannot patch code.
