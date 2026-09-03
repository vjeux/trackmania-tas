# Snow, Rally, and Desert behavior changes after release

## SnowCar — confirmed changes

Snow released on **2023-11-21**.

1. **2024-01-09: scripted delayed-player functions fixed.** Nadeo’s Winter 2024 changelog says three `SetPlayer_Delayed_` functions previously did not affect SnowCar. This confirms a Snow-specific gameplay/API behavior change, but does not establish a change to its free-driving force law.
2. **2024-02-27: action keys changed from clamp to re-range for analog devices.** AK3, for example, remaps full stick travel to 0–60% rather than clamping values above 60%. This is confirmed control-input behavior, not a changed tire/engine force law. Nadeo described it as an experiment through the Winter season; no later official rollback was found.
3. **2024-02-27: SnowCar hitbox improved.** Nadeo explicitly warned that collision and press-forward records would break. This is a confirmed simulation/collision behavior boundary.
4. **2024-05-22: global smooth-steering input fix.** The Desert update fixed analog devices failing to reach 100% with smooth steering. This is confirmed player-input behavior applicable to affected analog use, but the changelog does not isolate SnowCar or claim changed physics constants.

Snapshot evidence: Snow content is already staged in the 2023-11-15 snapshot and is identical in the measured handler and three tracked packs on 2023-11-24. A major handler/pack change appears 2023-12-21, but it has no independent behavior evidence and must not be labeled another Snow regime. The 2024-02-26 build brackets the confirmed hitbox/action-key changes.

## RallyCar — confirmed changes

Rally released on **2024-02-27**. The re-range action-key behavior shipped with its initial public release, so it is Rally’s release baseline rather than a post-release Rally change.

1. **2024-04-02: RallyCar behavior on custom ice fixed.** This is a confirmed family- and surface-specific behavior change. Nadeo does not specify the exact coefficient or failure mode.
2. **2024-05-22: global smooth-steering input fix.** As above, confirmed control-input behavior, not a family-isolated force-law change.

Snapshot evidence: the 2024-02-26 archive is the release build. The normalized measured handler is identical on 2024-02-26, 2024-03-19, and 2024-04-30. All three tracked packs change between March 19 and April 30, consistent with the official custom-ice fix but insufficient to locate it by hashes alone.

## DesertCar — no confirmed post-release change

Desert released on **2024-05-22**. The same release changelog includes the analog smooth-steering 100% fix, so that is part of the release baseline, not a later Desert change.

No official post-release DesertCar physics, hitbox, steering, or surface-behavior change was found through the 2026 changelog. The July 2024 changelog only mentions improved Desert-car light textures. Later snapshot boundaries—including handler changes by 2024-06-28 and 2024-12-12 and repeated pack changes—are static candidates only. There is no behavior control that ties them to Desert driving.

## Replay-control coverage

There are **no era-matched Snow, Rally, or Desert map+ghost replay controls** in the exhaustive corpus. The complete dynamic server sweep ends on 2022-06-21, before all three cars. The Roevhaal `63.546` oracle is Stadium-only. The archived clients were fingerprinted statically, not behaviorally replayed. Therefore:

- official changelog statements are the behavioral proof listed above;
- snapshot hashes only bracket delivery and must not create extra regimes;
- exact bit-level before/after behavior remains unmeasured for all listed changes.

## Official sources

- Snow release: https://www.trackmania.com/news/7794
- Winter 2024 fixes: https://www.trackmania.com/news/7901
- Action-key experiment: https://www.trackmania.com/news/7971
- Rally release/update and Snow hitbox: https://www.trackmania.com/news/7960
- Rally custom-ice fix: https://www.trackmania.com/news/8013
- Desert release and analog smooth-steering fix: https://www.trackmania.com/news/8097
- Later checked changelogs: https://www.trackmania.com/news/8135, /8256, /8442, /8717, /8952
