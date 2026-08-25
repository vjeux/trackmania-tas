# Names — is the title this repo publishes for a map the map's own name?

**Audit of 2026-08-25. Every published map, checked against its own
`.Map.Gbx` header and against trackmania.io. Six titles were wrong; one of
them was invented here. They are corrected, and this file is the retraction
(rule 4), not a deletion.**

It started with a thumbnail. vjeux was shown a map card reading
`[OBJECT OBJECT] BY TAXONOMON` and asked whether this project had work on it.
It did — the repo published that map as **"The Magnet Trial"**, a title
assembled here out of the skin files the map declares
(`magnet-trial-cp-01…16`), because `[object Object]` did not look like a name.
It is the name. Nobody outside this repo has ever called the map anything else,
and *"we've been using wrong name for maps?"* is the right question to ask
next — about all of them, not that one.

## How this was checked

Three sources, in this order of authority. **None of them is a document this
project wrote**, which is the point: a caption agreeing with an index we also
wrote is not confirmation of anything.

1. **The `.Map.Gbx` header** — the name the map declares about itself, read out
   of the `<ident name="…">` attribute of its header XML by
   `tmmaps header --names`. Ground truth.
2. **trackmania.io** `/api/map/<uid>` — `name`, and `authorplayer.name` for the
   author. Fetched by uid, banked raw.
3. **trackmania.exchange** `/api/maps?id=…` — the TMX upload's name, and the
   TMX-id → uid resolution. Kept as its own column; it is the name of an
   *upload*, and it is not always the name of the *map*.

Reproduce it, offline, from the banked responses:

```
tmmaps header --names <every banked .Map.Gbx>  > names.tsv
tmsite names --root . --bank <bank> --headers names.tsv
```

Every request in the fetch was a GET (rule 1). 36 maps, 74 requests, all 200.

## Two decoding bugs the audit had to fix first

Both would have produced confident, wrong answers, and one of them is why
nobody caught 186935 for months.

* **`tmmaps header` read the map name off the wrong XML tag.** It asked
  `<desc>` for a `name` attribute; the name is on `<ident>`. `<desc>` has no
  such attribute, so the field printed as `-` for **every map in the corpus** —
  a value nobody could read, and therefore a value nobody ever compared
  anything to.
* **A name is not a string of letters.** It arrives wearing two layers:
  ManiaPlanet markup (126859's header literally holds
  `$o$i$aa0Kack$05ay Re$09alo$6a0ad$aa0ed $4f0#290`) and XML attribute escaping
  (208024's holds `Miru&apos;s Hell 2`). Comparing raw strings flags every
  decorated map and none of the flags are real. Both layers are now decoded
  once, in `gbx::name`, so the map file and the live service cannot be decoded
  differently.

## The table

| map | directory | published as | header name | trackmania.io | author | verdict |
|---|---|---|---|---|---|---|
| 126859 | `126859-kacky-reloaded-290` | Kacky Reloaded #290 | Kacky Reloaded #290 | Kacky Reloaded #290 | SuperrKuzco | match |
| 134672 | `134672-kekl-sausage-ice` | KEKL- SAUSAGE ICE | KEKL- SAUSAGE ICE | KEKL- SAUSAGE ICE | Travis.TM | match |
| 145875 | `145875-unlucke-get-jiggy-with-it` | unluckE - get jiggy with it | unluckE  - get jiggy with it | unluckE  - get jiggy with it | InfTM | match (spacing differs) |
| 146612 | `146612-spaghetti-nights-2` | Spaghetti Nights 2 | Spaghetti Nights 2 | Spaghetti Nights 2 | AmpelJoe10 | match |
| 153527 | `153527-p-found-pokeuuu` | P-Found - Pokeuuu | P-Found - Pokeuuu | P-Found - Pokeuuu | PokeuuuTM | match |
| 165922 | `165922-idm-ruinin-ur-day-460` | idm ruinin ur day #460 | idm ruinin ur day #460 | idm ruinin ur day #460 | Petvaria. | match |
| 173636 | `173636-tap-water-01` | Tap water 01 | Tap water 01 | Tap water 01 | Reddnox | match |
| 173691 | `173691-spring-2023-15-underwater` | Spring 2023 - 15 (Underwater) | Spring 2023 - 15 (Underwater) | Spring 2023 - 15 (Underwater) | Reddnox | match |
| 186935 | `186935-magnet-trial` | [object Object] | [object Object] | [object Object] | Taxonomon | match |
| 191465 | `191465-training-10-long` | Training - 10 Long | Training - 10 Long | Training - 10 Long | in-.- | match |
| 197047 | `197047-welcome-to-wiggles` | Welcome☺to wiggles | Welcome☺to wiggles | Welcome☺to wiggles | CatBagasm | match |
| 203072 | `203072-yeet-fall-2024-04` | YEET Fall 2024 - 04 | YEET Fall 2024 - 04 | YEET Fall 2024 - 04 | QuentinTM15 | match |
| 203330 | `203330-get-in-the-hole-impossible` | Get in the Hole | Get in the Hole | Get in the Hole | EvenOliveTM.exe | match |
| 208024 | `208024-mirus-hell-2` | Miru's Hell 2 | Miru's Hell 2 | Miru's Hell 2 | byMiru | match |
| 210218 | `210218-fall-2024-25-pure-wet-icy-wood` | Fall 2024 - 25 (Pure Wet Icy Wood) | Fall 2024 - 25 (Pure Wet Icy Wood) | Fall 2024 - 25 (Pure Wet Icy Wood) | R4igekon | match |
| 227654 | `227654-the-blev-special` | The Blev Special | The Blev Special | The Blev Special | Blev.. | match |
| 227969 | `227969-great-wtf-of-what-165` | Great wtf of what #165 | Great wtf of what #165 | Great wtf of what #165 | FrankTheHamster | match |
| 228607 | `228607-torment-1-up` | Fall 2024 - 08 Torment (1-UP)(ft' Emelius) | Fall 2024 - 08 Torment (1-UP)(ft' Emelius) | Fall 2024 - 08 Torment (1-UP)(ft' Emelius) | Bernkastel_. | match |
| 228811 | `228811-torment-1-down` | Fall 2024 - 08 Torment (1-DOWN) | Fall 2024 - 08 Torment (1-DOWN) | Fall 2024 - 08 Torment (1-DOWN) | Bernkastel_. | match |
| 238835 | `238835-turtle-trial-angustus` | [Turtle Trial] Angustus | [Turtle Trial] Angustus | [Turtle Trial] Angustus | Bald_tm | match |
| 249521 | `249521-impossible-at-for-ssano` | impossible at for ssano | impossible at for ssano | impossible at for ssano | in-.- | match |
| 252289 | `252289-surely-my-least-cooked-at` | surely my least cooked at | surely my least cooked at | surely my least cooked at | in-.- | match |
| 267460 | `267460-impossible-mini-trial-2` | Impossible Mini Trial 2 | Impossible Mini Trial 2 | Impossible Mini Trial 2 | Mattlightning | match |
| 267859 | `267859-bald-turtle-35` | bald turtle #35 | bald turtle #35 | bald turtle #35 | Bald_tm | match |
| 270051 | `270051-fall-2025-16-cp1-end` | Fall 2025 - 16 CP1 End | Fall 2025 - 16 CP1 End | Fall 2025 - 16 CP1 End | in-.- | match |
| 270053 | `270053-fall-2025-18-cp1-end` | Fall 2025 - 18 CP1 End | Fall 2025 - 18 CP1 End | Fall 2025 - 18 CP1 End | in-.- | match |
| 274191 | `274191-u10s-32-yeet-max-up` | U10S_32 By Everios96 [Yeet] MAX-UP | U10S_32 By Everios96 [Yeet] MAX-UP | U10S_32 By Everios96 [Yeet] MAX-UP | TwinsNaA | match |
| 276874 | `276874-untitled-01` | untitled 01 | untitled 01 | untitled 01 | DugonGOD | match |
| 276877 | `276877-untitled-02` | untitled 02 | untitled 02 | untitled 02 | DugonGOD | match |
| 279197 | `279197-fall-2025-01-reverse-cp1-end` | Fall 2025 - 01 Reverse CP1 End | Fall 2025 - 01 Reverse CP1 End | Fall 2025 - 01 Reverse CP1 End | in-.- | match |
| 279209 | `279209-fall-2025-13-reverse-cp1-end` | Fall 2025 - 13 Reverse CP1 End | Fall 2025 - 13 Reverse CP1 End | Fall 2025 - 13 Reverse CP1 End | in-.- | match |
| 279218 | `279218-fall-2025-22-reverse-cp1-end` | Fall 2025 - 22 Reverse CP1 End | Fall 2025 - 22 Reverse CP1 End | Fall 2025 - 22 Reverse CP1 End | in-.- | match |
| 284238 | `284238-you-love-water` | YOU LOVE WATER | YOU LOVE WATER | YOU LOVE WATER | Eating_My_Wings | match |
| 285268 | `285268-pain-ft-mango-teuflum` | Pain ft Mango & Teuflum | Pain ft Mango & Teuflum | Pain ft Mango & Teuflum | Slidelock | match |
| 285885 | `285885-finish-is-on-the-roof` | finish is on the roof to your right | finish is on the roof to your right | finish is on the roof to your right | lasyopp | match |
| 286279 | `286279-turtle-trial-leto` | [Turtle Trial] Leto | [Turtle Trial] Leto | [Turtle Trial] Leto | Bald_tm | match |

*"Published as" is the root README's index entry, as it now reads. The verdict
column is the audit's, against the header.*

## What was wrong, and what it is now

| map | published as | the map's own name | what happened |
|---|---|---|---|
| 186935 | The Magnet Trial | `[object Object]` | **invented here.** Built from the map's skin dependencies because the real name looked like a bug. It is a bug — somebody's editor stringified a JavaScript object into the title field — but it is the map's name, and the author is Taxonomon. |
| 197047 | Welcome to wiggles | `Welcome☺to wiggles` | the author put U+263A between the words, not a space. Header and trackmania.io agree, byte for byte. |
| 203330 | Get in the Hole ( Impossible ) | `Get in the Hole` | we published the **TMX upload's** title. The map is "Get in the Hole"; " ( Impossible )" is the uploader's decoration on the exchange page. |
| 228607 | Torment (1-UP) | `Fall 2024 - 08 Torment (1-UP)(ft' Emelius)` | truncated. The campaign prefix and Emelius's credit were dropped. |
| 228811 | Torment (1-DOWN) | `Fall 2024 - 08 Torment (1-DOWN)` | truncated, same way. |
| 274191 | U10S_32 [Yeet] MAX-UP | `U10S_32 By Everios96 [Yeet] MAX-UP` | truncated — and what was dropped is another author's credit. |

Eleven further places said something a fourth way: a caption reading
`Fall 2025 - 16 (CP1 end)` for a map called `Fall 2025 - 16 CP1 End`,
`Great WTF of what #165` for `Great wtf of what #165`, a page titled
`You love water` under an index entry reading `YOU LOVE WATER`. Those are now
the header's spelling too. **Every one of the 36 published names is now the
name in the map's own header**, and `tmsite names` is the scan that says so.

**Directory names were not touched.** `186935-magnet-trial/` is a URL, it may
be linked from outside this repo, and a rename would break those links for a
cosmetic gain. A directory slug is not a claim about what a map is called; the
page inside it is.

## One name this repo still prints differently, on purpose

145875's real name has **two** spaces before the dash —
`unluckE  - get jiggy with it` — in the header and on trackmania.io alike.
Markdown collapses a run of spaces, so the rendered title has one. The map's
page says so; nothing else is changed.

## What the audit turned up that is not a naming problem

* **126859's banked map is a different upload from the one the boards track.**
  Our file declares uid `Z4p7Gy3gjXINzu8pgm_WzYYjtmg`; TMX and trackmania.io
  both list `NTU3ZGRlMzEtYzNiOC00YzJmLTk` for map id 126859. Same name, same
  author time (24.062), different container. Nothing on the page depends on
  which copy it is, but a re-simulation should be told to use the board's.
* **Six map directories bank maps that are not their map** — 146612 holds seven
  Spaghetti maps, 153527 five Pokeuuu maps, 210218 six Pure-Wet-Icy-Wood
  siblings, 284238 two cold-start siblings. Every one of them is under a
  `key_siblings/`, `route_siblings/` or `cold_siblings/` directory, which is
  what those directories are for. Listed here only so a future scan that joins
  on the directory name instead of the uid knows the trap is there:
  `sort -u` by directory picks `Pathfinding - Legacy` for 153527.
* **173691's map is only banked as working copies.** All six banked files with
  uid `D0KdisOjKSxSIAXawtwlBqLz9Kb` are ours — gates neutralised, spawns moved.
  They agree with each other and with trackmania.io on the name, so the name is
  not in doubt; a pristine download is not on the persistent store.
