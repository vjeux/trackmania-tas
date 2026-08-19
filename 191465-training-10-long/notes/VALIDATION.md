# Cold-process validation transcript — map 191465 Training - 10 Long

Date: 2026-08-19T00:31:16Z  host: 77493.od.fbinfra.net
Oracle: /tmp/tmoracle/server/TrackmaniaServer (GameVersion 3.3.0, git 128182-0de74ece09e)

Each run below is a FRESH TrackmaniaServer process (tmtas validate spawns one
per worker and exits). The human WR ghost is carried in every batch as the
known-answer control: it must return 13081.

## sha256
170f32d322c22882beffba9e65122961ff645c1ddb8a644f985b168cd517c123  tapes/TAS_13074_analog.Ghost.Gbx
1476ab63dcf38d83b59f200f2db0b738318ec10f259ec3d9b8ddbade4605d7f0  tapes/TAS_13080_firstpass.Ghost.Gbx
78fa358431eb3cef47ef3533ced5ee3008f8edd6b11032fa1d1ef88e936c7360  tapes/WIP_keyboard.Ghost.Gbx
d50e4b961c77d09952aa68fb0be0c5c07c18d394f2dcaa34437d2c812ce42605  tapes/WIP_pad5.Ghost.Gbx
c80da94f0d0af3565cfa9797e69622dc71b79bfc3ebbbdf9c49d20cca3838fb1  human_WR_13081.Ghost.Gbx
418dc5d0ba139df2f9bd4e17dac63444d69e04e5c2a74cbb77f683248b13f4d7  t10long.Map.Gbx

## cold validation pass 1  (00:31:19Z)
```
file                                       sim_time      cps
TAS_13074_analog.Ghost.Gbx                    13074        -
TAS_13080_firstpass.Ghost.Gbx                 13080        -
WIP_keyboard.Ghost.Gbx                        13075        -
WIP_pad5.Ghost.Gbx                            13074        -
human_WR_13081.Ghost.Gbx                      13081        -
```

## cold validation pass 2  (00:31:29Z)
```
file                                       sim_time      cps
TAS_13074_analog.Ghost.Gbx                    13074        -
TAS_13080_firstpass.Ghost.Gbx                 13080        -
WIP_keyboard.Ghost.Gbx                        13075        -
WIP_pad5.Ghost.Gbx                            13074        -
human_WR_13081.Ghost.Gbx                      13081        -
```

## cold validation pass 3  (00:31:39Z)
```
file                                       sim_time      cps
TAS_13074_analog.Ghost.Gbx                    13074        -
TAS_13080_firstpass.Ghost.Gbx                 13080        -
WIP_keyboard.Ghost.Gbx                        13075        -
WIP_pad5.Ghost.Gbx                            13074        -
human_WR_13081.Ghost.Gbx                      13081        -
```
