# Ignition (UDS / Virgin Interactive, 1997) — music for the Trackmania port

The game's own soundtrack (the Steam release's `MUSIC/Track02..07.ogg`),
re-encoded to OGG Vorbis q5 like the `mk64-music` branch, for the Trackmania
2020 maps of the seven Ignition tracks (each map's custom music links its file
here by its raw.githubusercontent URL).

| file | game track | used on |
|---|---|---|
| ignition_moosejaw.ogg | Track02 | Moose Jaw Falls |
| ignition_goldrush.ogg | Track03 | Gold Rush |
| ignition_snakeisland.ogg | Track04 | Snake Island |
| ignition_lostruins.ogg | Track05 | Lost Ruins |
| ignition_yodel_cape.ogg | Track06 | Yodel Peaks, Cape Thor (shared in the game) |
| ignition_tokyo.ogg | Track07 | Tokyo Bullet |

`sounds/` — the levels' ambient sounds (`LEVELS/<DIR>/SOUND/<n>_<NAME>.{WAV,PAT}`:
8-bit 11 kHz WAVs and Gravis UltraSound patches, unwrapped and re-encoded to
OGG Vorbis q5 44.1 kHz mono by `ignition sounds`), named
`ign_<level>_<n>_<name>.ogg`, for the maps' MediaTracker Sound blocks (rain,
trains, waves, jungle, wind, city...).
