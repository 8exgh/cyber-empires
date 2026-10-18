# E. Battle preparation / ACTION.PRG hand-off / tactical practice modes (CYBOR.PRG)

Scope: everything in CYBOR.PRG that composes the 80x78 battlefield, places units, runs the two
pre-battle screens, writes the hand-off files for ACTION.PRG, reads the results back, plus the
Showdown and Survivor practice modes. Offsets/tables were read from the raw CYBOR.PRG data segment
(file offset = 0x3000 + 8947*16 + sym.off; helper: `re/tbl.py SYM [cols] [b]`), listings from
`re/fnc.py` / `re/calls.py`.

Conventions: tile matrix `MATRIX[80*78]` bytes, index `y*80+x`; tile pixel size 32; all "UI pixel"
coordinates are on the 320x200 screen. `country(c)` = `_MECHRECORD + c*236`; `owner` = `+172`,
`slots[k]` = 7 shorts at `+192..+204` (k=0..6), `fortlevel` = `+226`, `cyborg[i]` = 5 shorts at
`+234+10*i` (`type, status%, home country, ..`), `citydamage%` = `+384`, `terrain` = `+390`.
Cyborg types 0..8 = TITAN BEHEMOTH HERCULES ACHILLES CROSSBOLT MARS DRAGON CYCLOPS MERCURY; 10 = empty.
`_HL[9] = {750,600,390,270,150,150,90,90,30}` = max armour (hit points) per type.

Note on country "build slots": `slots[k]` (k=0..6) is the list of structures in the country. Values
seen by this code: 0 empty, 1 capital/government, 2 fortification (level in `+226`), 3 uni-post
factory, 4 dual-post factory, 5 quadro-post factory, 6 (quadro-post variant, same 10x8 picture as 5,
see open questions), 7 factory enhancement. The capital is always slot 0 (building requires a
government first), which `_CHECK_PIECES` relies on.

---------------------------------------------------------------------------------------------------
## 0. Where a real-time battle starts and ends (hand-off protocol)

`_SET_ACTION_PHASE` (turn flow; other agent) does, per confirmed attack `ALIST[i] = {attacker cty,
defender cty, flag}`:

```
LOAD_ATTACK(att, def)  -> ACTIONLIST[0..9]  = types of att's cyborgs whose destination (+238) == def
                           ACTIONLIST[40+i] = hit points = status% * HL[type] / 100   (_HITS_LEFT)
                           (the cyborg slot in the country record is emptied: type=10, status=100)
LOAD_DEFEND(def)       -> ACTIONLIST[10..19] / [50..59] likewise for all cyborgs in def
SORT_ACTIONLIST(0,10); SORT_ACTIONLIST(10,20)   // compact live cyborgs to the front; empty slot hits = -1
ANAME = player name of owner(att); DNAME = owner(def)          (names: _MECHRECORD+14+32*p, 19 chars)
if SELECTION == 2 (game option "simulate all battles")  -> _BATTLE_SIMULATION (no ACTION.PRG)
SET_PLAYER_TYPE();  if CHOICE == 3 (both computer)      -> _BATTLE_SIMULATION
FADE_OUT; ACTION_SETUP();  if SIMULATE                   -> _BATTLE_SIMULATION
INSTRAT = 0; SET_TERRAIN_LOAD();  save locals into _PZ+44..+72, SELECTION->_PZ+88, DELAYTIME->_PZ+96
WRITE_FILE2("pass2.dat", &_ALIST, 0x483)     // 1155 bytes: _ALIST(600) _CLIST(144) _TLIST(22) _BDLEVEL _CYDAMAGE
                                             // _PIECES(156) _SOUNDS(10) _FADEFLAG _CYBSTRING(60) _PSTRUCT(14)
                                             // _LTERRAIN _ACTIONTEMP(40) _OLDCITYDAM _ATTACKCTY _DEFENDCTY
                                             // _ACTIONLIST(80) _ATTCONFIRMED _DEFCONFIRMED(7) = 0xCE57..0xD2DA
LOADSTRAT = 0; _START()                      // writes matrix.dat, acts.DAT, pinfo.iat, actstuf.iat, EXITS
--- CYBER.COM/runme.bat runs ACTION.PRG, then CYBOR.PRG again ---
CYBOR _MAIN: LOAD_FILE2("pinfo.iat",&_PZ,0x71); PASSFLAG = _PZ[0]; _PZ[0]=0; rewrite pinfo.iat;
             EGA=_PZ+86; if PASSFLAG: LOAD_FILE2("actstuf.iat",&_GAMESTUFF,0x1E6)
             GAMESTUFF+461 = _PZ+90 (control: 0 mouse / joystick), GAMESTUFF+467 = byte _PZ+92 (sound type 0/1/2)
_DO_MAIN_BODY: PASSFLAG==4 -> LOAD_DATA_FILE(2) ("acts.dat" -> _MECHRECORD) and resume strategy;
               PASSFLAG 1/2 -> tactical mode resume (see §3)
_SET_ACTION_PHASE (PASSFLAG path): LOAD_FILE2("pass2.dat"); _START2(); restore locals from _PZ; ...
```

`_PZ` (113 bytes, == PINFO.IAT): +0 short mode (0 none, 1 Showdown, 2 Survivor, 4 strategy battle;
ACTION.PRG zeroes it while running and restores it on exit), +2 char ANAME[19], +21 char DNAME[19],
+40 T_NUM, +42 CHOICE, +44..+62 ten SET_ACTION_PHASE locals, +64/+66/+68 CPCOUNT1/2/3, +70 FADEFLAG,
+72 LTERRAIN, +74 LOADSTRAT, +76 KILLTONS, +78 UPBOUND, +80 LOWBOUND, +82 GAMECOUNT, +84 CLASS,
+86 EGA (written by INTRO.PRG), +88 SELECTION, +90 control mode (INTRO), +92 sound type (INTRO),
+94 ? (INTRO), +96 byte DELAYTIME, +97..+112 eight shorts joystick calibration (INTRO `_DOCAL`).
ACTION.PRG only reads/writes the first 0x5C=92 bytes of PINFO.IAT (into its `_P`).

---------------------------------------------------------------------------------------------------
## 1. Composing the 80x78 battlefield

### 1.1 Terrain base map: `_READ_MATRIX`
```
prefix = {"ARCT_","CONI_","DEST_","VOLC_","CITY_"}[T_NUM]      // T_NUM = country.terrain - 1  (LANGS+14297.., 6 bytes each)
name   = prefix + itoa(DEFENDCTY) + ".MAP"                      // e.g. CITY_10.MAP
GIFLOAD(name, SCREEN3)                                          // GIF87a decoded into a 320x200 buffer
for x in 0..79: for y in 0..77: MATRIX[y*80+x] = pixel(x,y)     // palette index == tile index
GIFLOAD("door3.gif", SCREEN3)                                   // restore scratch screen
```
`_FNAMES` ("arctic1.MAP", "carnif1.MAP", "DESERT1.MAP", "val1.MAP", "city4.MAP") is dead data (no code
references it). `_READ_MATRIX2`/`_WRITE_MATRIX` just `LOAD_FILE2/WRITE_FILE2("matrix.dat", &MATRIX, 0x1860)`.

### 1.2 Attack direction and the structure shift
`ADIR = rand(0,3)` (set in `_ACTION_SETUP`). 0 = attacker enters from the top edge, 1 = right, 2 = bottom,
3 = left. All structures are shifted away from the attacker's edge by `_BO[ADIR] = (0,19), (-17,0),
(0,-18), (15,0)` (x,y in tiles). On a CITY map (terrain 5) the shift is always `(-1,+1)` instead.

### 1.3 Structures: `_INSERT_STRUCTURES` (exact)
Called once per battle (human defender: from `_SET_DEFENDER_SCREEN`; AI defender: from `_ACTION_SETUP`).

Tables (all shorts, but only the low byte is stored into MATRIX):
* `_BUILDPOS[8]` = (y,x) tile anchors per slot k (NOTE: first byte is the ROW):
  k0 (35,36) capital, k1 (26,25), k2 (26,36), k3 (26,47), k4 (43,25), k5 (43,36), k6 (43,47), [7] = (24,25) fort anchor.
* `_CAPBUILD` = 5 owner patterns x 7 rows x 11 cols (tiles 60..93 structure set, 134..139 wall corners/edges).
  Capital core tile (the targetable piece): owner p -> tile 75+p, at pattern (x=5,y=3) (owner 4: x=4,y=3).
* `_FACP[4]` = {cols, stride, rows, base}: v=3: 7,42,6,0 ; v=4: 8,64,8,210 ; v=5 and v=6: 10,80,8,530.
  `_FAC` = 930 shorts: 5 owner patterns of each size consecutively (base + owner*stride).
  Factory core tiles are 71..74; count per pattern: uni 1, dual 2, quadro 4.
* `_FAC3[5]` = {abs index into _FAC, tile}: (578,105) (658,105) (727,106) (811,107) (864,106) — for v==5 only,
  the one core at that index (owner p's 10x8 pattern) is overwritten by tile 105..107 (so a quadro-post of
  type 5 shows 3 core buildings + 1 pad; type 6 shows 4 cores).
* `_ENHANCE` = 5 owner patterns x 7x7; core tile 70 at (3,3).
* `_FORT` = 3 levels x 22 tiles: [0] horizontal wall, [1] vertical wall, [2..21] the 20 `_FORTPOS` pieces.
  L1: 144,145, 149,146,148,147, 142,143,143,142,142,143,143,142, 141,140,141,140,141,140,141,140
  L2: 135,134, 137,136,138,139, 130,131,131,130,130,131,131,130, 133,132,133,132,133,132,133,132
  L3: 121,120, 123,122,124,125, 126,127,127,126,126,127,127,126, 128,129,128,129,128,129,128,129
* `_FORTPOS[20]` = linear offsets y*80+x (unshifted): (23,24) (58,24) (23,52) (58,52) | (37,24) (35,22) (43,24)
  (45,22) | (37,52) (35,54) (43,52) (45,54) | (58,35) (60,33) (60,42) (58,40) | (23,35) (21,33) (21,42) (23,40).
* `_TURCORDS[4][12]` (x,y) turret tile positions per ADIR (unshifted):
  ADIR0: (32,23)(39,21)(42,21)(48,23)(23,21)(60,21)(20,21)(63,21)(17,21)(66,21)(14,21)(69,21)
  ADIR1: (59,30)(61,37)(61,39)(59,45)(61,25)(61,52)(61,22)(61,55)(61,19)(61,58)(61,16)(61,61)
  ADIR2: (32,53)(39,55)(42,55)(48,53)(23,55)(60,55)(20,55)(63,55)(17,55)(66,55)(14,55)(69,55)
  ADIR3: (22,30)(20,37)(20,39)(22,45)(20,25)(20,52)(20,22)(20,55)(20,19)(20,58)(20,16)(20,61)
* `_TURT[4][12]` turret unit type per fort level (0xFF = none): L0 all none; L1: 9,9,9,9,12,12,12,12,-,-,-,-;
  L2: 10,10,10,10,12x6,-,-; L3: 11,11,11,11,12x8.  (9 LAC, 10 HAC, 11 NC, 12 LRM — ACTION `_STATMECH` has 13
  unit types 0..12.)

```
void INSERT_STRUCTURES() {
  xo = BO[ADIR].x; yo = BO[ADIR].y; notcity = 1;
  if (country(DEFENDCTY).terrain == 5) { notcity = 0; xo = -1; yo = +1; }
  TURRETT = 0; np = 0;                       // np = next free _PIECES entry (PIECES cleared earlier)
  own = country(DEFENDCTY).owner;
  for (k = 0; k < 7; k++) {
    PSTRUCT[k] = 0; v = slots[k]; y0 = BUILDPOS[k].y + yo; x0 = BUILDPOS[k].x + xo; i = 0;
    if (v == 1) {                                                    // capital 11x7
      for (y = y0; y < y0+7; y++) for (x = 0; x < 11; x++, i++) {
        t = CAPBUILD[own*77 + i];
        if (notcity || t < 120) MATRIX[y*80 + x0 + x] = t;        // city maps keep their own walls
        if (75 <= t && t <= 79) PIECES[np++] = {(x0+x)*32, y*32, 1};
      }
    } else if (3 <= v && v <= 6) {                                   // factory
      f = v-3; base = FACP[f].base + own*FACP[f].stride;
      for (y = y0; y < y0+FACP[f].rows; y++) for (x = 0; x < FACP[f].cols; x++, i++) {
        t = FAC[base+i]; MATRIX[y*80+x0+x] = t;
        if (v == 5 && FAC3[own].index == base+i) MATRIX[y*80+x0+x] = FAC3[own].tile;
        if (71 <= t && t <= 74) { PIECES[np++] = {(x0+x)*32, y*32, 2}; PSTRUCT[k] = v; }   // tested on FAC[], not MATRIX
      }
    } else if (v == 7) {                                             // enhancement 7x7
      for (y = y0; y < y0+7; y++) for (x = 0; x < 7; x++, i++) {
        t = ENHANCE[own*49+i]; MATRIX[y*80+x0+x] = t;
        if (t == 70) { PIECES[np++] = {(x0+x)*32, y*32, 2}; PSTRUCT[k] = 7; }
      }
    } else if (v == 2) {                                             // fortification
      L = country.fortlevel; off = (L-1)*22; TURRETT = L;
      // horizontal walls: rows 24 and 52, x = 24..57 except gate gap 37..43; gate bastion rows 22 and 54, x = 35..45
      for (x = 24; x < 58; x++) {
        if (x < 37 || x > 43) { MATRIX[(yo+24)*80 + x+xo] = FORT[off]; MATRIX[(yo+52)*80 + x+xo] = FORT[off]; }
        if (x > 34 && x < 46) { MATRIX[(yo+22)*80 + x+xo] = FORT[off]; MATRIX[(yo+54)*80 + x+xo] = FORT[off]; }
      }
      // vertical walls: cols 23 and 58, y = 25..51 except gap 35..40; bastion cols 21 and 60, y = 33..42
      for (y = 25; y < 52; y++) {
        if (y < 35 || y > 40) { MATRIX[(y+yo)*80 + xo+58] = FORT[off+1]; MATRIX[(y+yo)*80 + xo+23] = FORT[off+1]; }
        if (y > 32 && y < 43) { MATRIX[(y+yo)*80 + xo+60] = FORT[off+1]; MATRIX[(y+yo)*80 + xo+21] = FORT[off+1]; }
      }
      for (j = 0; j < 20; j++) MATRIX[FORTPOS[j] + yo*80 + xo] = FORT[off+2+j];     // corners + gate pieces
      for (j = 0; j < 12; j++) {                                      // 12 turret markers (all levels)
        tx = TURCORDS[ADIR][j].x + xo; ty = TURCORDS[ADIR][j].y + yo;
        TRYCYBT[2j] = tx; TRYCYBT[2j+1] = ty;
        BACKGROUND[BKINDEX++] = {MATRIX[ty*80+tx], ty*80+tx};        // remembered for restoration
        MATRIX[ty*80+tx] = 158;                                       // temporary "turret here" marker
        TRYCYBT[2j] *= 32; TRYCYBT[2j+1] *= 32;                      // -> pixels
      }
    }
  }
}
```
`_PIECES[26]` = {pixel x, pixel y, kind} (kind 1 capital, 2 factory/enhancement) are the attacker AI's
structure targets and are passed to ACTION.PRG; `_PSTRUCT[7]` remembers which slot each group of
pieces belongs to (fort: 0 — forts/walls are never "pieces").

### 1.4 City damage pre-applied to the map: `_SET_TERRAIN_LOAD` (after ACTION_SETUP, before _START)
```
TDIF = 1; CITY = 0;
if (QUICKL) { if (terrain(def) == LTERRAIN) TDIF = 0; else if (LTERRAIN == 5 || terrain(def) == 5) CITY = 1; }
LTERRAIN = terrain(def);            // (QUICKL/TDIF/CITY are CYBOR-side flags for the simulation/summary tile reload)
if (LTERRAIN == 5) {
  COUNT_CTY_PIECES(&CPCOUNT1);      // count MATRIX tiles 32..59 (intact city buildings) in x 3..76, y 3..74
  CHECK_CTY_DAMAGE(CPCOUNT1);       // dmg = country.citydamage% ; if dmg: DAMAGE_CITY_PIECES(n=(int)(dmg*0.01*count), dmg)
  COUNT_CTY_PIECES(&CPCOUNT3);
}
DAMAGE_CITY_PIECES(n, pct): while (n > 0) for x in 3..76: for y in 3..74:
     t = MATRIX[y*80+x];
     if (32<=t<=59 && n>0 && rand(0,99) < pct) { MATRIX[..] = WALLCITY[t]; n--; }       // building -> rubble (3..31)
     else if (3<=t<=31 && n>0 && rand(0,99) < pct) MATRIX[..] = WALLCITY[t];           // rubble -> 120..145 variant (no n--)
```
`_WALLCITY[180]` is the "damaged version of tile t" lookup: t 3..31 -> 120..145, t 32..59 -> 3..31,
t 61..89 -> 94..119 (103 at 89), t 168..179 -> 146,147,148,149,135..142.

### 1.5 Attacker positions: `_SET_POSITIONS(first, last, useDir, dir)`
`_APOS[4]` = (39,3) (76,39) (39,74) (3,39) tile; `_AOFF[4]` = (5,0) (0,5) (5,0) (0,5).
```
off = AOFF[ADIR]; out = 0;  if (useDir) { off = AOFF[dir]; if (dir == 2) out = 20; }
base = APOS[ADIR+dir];  k = 0;                         // strategy: dir = 0, ADIR random; tactical: ADIR = 0, dir 0 or 2
for (i = first; i < last; i++) {
  if (k++ == 0)                  p = base;
  else if (off.x > 0 || off.y > 0) { p = base + off; off = -off; }
  else                           { p = base + off; off = -(off - AOFF[ADIR+dir]); }
  TRYCYB[out] = p.x*32; TRYCYB[out+1] = p.y*32; out += 2;
}
```
i.e. along the edge: base, +1, -1, +2, -2, +3, -3, +4, -4, +5 steps of 5 tiles (ADIR 0: x = 39,44,34,49,29,
54,24,59,19,64 on row 3). Done for all 10 slots regardless of occupancy (`ACTION_SETUP: SET_POSITIONS(0,10,0,0)`).
`TRYCYB[40]`: [0..19] attacker (x,y) pairs, [20..39] defender pairs; `_CLEAR_TRYCYB` presets all 40 to 128.

### 1.6 Defender positions
Defender arrangement list `_DEFARR[11]` (`_SET_DEFENDER_ARRANGEMENT`): indexes 10..19 of the occupied
defender ACTIONLIST slots in order, terminated by -1. `_DEFCON` = ACTIONLIST index-10 of the cyborg the
defending human controls; `_ATTCON` likewise for the attacker (AI: last occupied slot).

UI-space positions (`_AP[4][10]`, pixel coords on the schematic minimap, per ADIR, per defender slot):
ADIR0: (63,81)(92,81)(57,81)(98,81)(51,81)(104,81)(45,81)(110,81)(39,81)(116,81)
ADIR1: (117,128)(117,97)(117,134)(117,91)(117,141)(117,83)(117,148)(117,76)(117,155)(117,69)
ADIR2: (63,144)(92,144)(57,144)(98,144)(51,144)(104,144)(45,144)(110,144)(39,144)(116,144)
ADIR3: (41,128)(41,97)(41,134)(41,91)(41,141)(41,83)(41,148)(41,76)(41,155)(41,69)
`_BACKUP[4]` = (0,15) (-15,0) (0,-15) (15,0) (UI pixels = 7-8 tiles behind the wall).

AI defender (`_ACTION_SETUP`, CHOICE 1 or 3): for occupied slot i (10..19):
`TRYCYB[20+2(i-10)] = AP[ADIR][i-10] (+ BACKUP[ADIR] if TURRETT != 0 and type not in {TITAN, BEHEMOTH, CROSSBOLT})`
— heavy units stand in front of the fort, light ones behind the wall. Human defender: `_AUTO_PLOT`
uses `AP[ADIR][j]` for arrangement order j (no BACKUP), or the player clicks positions (§1.8).

Conversion UI -> tiles and collision resolution: `_SET_DEF_POS` (runs for all 10 defender pairs, even empty):
```
xo,yo = BO[ADIR]; special = 0;   if (T_NUM == 4) { xo = yo = 0; special = 118; }
for (s = 20; s < 40; s += 2) {
  TRYCYB[s]   = (TRYCYB[s]   - 17) / 2 + xo + 10;        // C division (trunc toward 0)
  TRYCYB[s+1] = (TRYCYB[s+1] - 53) / 2 + yo + 9;         // minimap: 2 px per tile, origin tile (10,9) at px (17,53)
  clear = CHECK_CYB_CLEAR(s);  t = MATRIX[y*80+x];  k = 0; r = 1;
  while (!clear || !passable(t)) {                         // spiral: 8 directions _SW, radius r
    x += SW[k].x*r; y += SW[k].y*r; clear = CHECK_CYB_CLEAR(s); t = MATRIX[y*80+x];
    if (!passable(t) || x < 3 || x > 76 || y < 3 || (y > 74 && clear)) { x -= SW[k].x*r; y -= SW[k].y*r; clear = 0; }
    t = MATRIX[y*80+x]; if (++k > 7) { r++; k = 0; }
  }
  BACKGROUND[BKINDEX++] = {t, y*80+x}; MATRIX[y*80+x] = 200;      // occupied marker
  TRYCYB[s] = x*32; TRYCYB[s+1] = y*32;
}
for (b = 0; b < BKINDEX; b++) MATRIX[BACKGROUND[b].off] = BACKGROUND[b].tile;   // remove 200 and 158 markers
```
`_SW[8]` = (0,-2) (2,-2) (2,0) (2,2) (0,2) (-2,2) (-2,0) (-2,-2).
`passable(t)`: `LP[T_NUM].lo < t < LP[T_NUM].hi || t == 0 || t == special`, with `_LP` = arctic (0,59),
conifer (20,59), desert (0,59), volcanic (0,52), city (120,158).
`_CHECK_CYB_CLEAR(&clear, s)`: clear = 1; for every earlier defender pair e in 20..s-2 (already pixels, /32):
if |x - ex| <= 1 and |y - ey| <= 1 -> clear = 0. (The `y > 74 && clear` precedence looks like a source bug;
treat y > 74 as out of bounds.) Because markers are restored, MATRIX.DAT never contains tiles 158/200;
turrets reach ACTION.PRG only as units (records 20..31).

### 1.7 `_ACTION_SETUP` (pseudo-C)
```
GIFLOAD("door3.gif", SCREEN2); SORT_ACTIONLIST(0,10); SORT_ACTIONLIST(10,20);
ATTPLAY = owner(ATTACKCTY); DEFPLAY = owner(DEFENDCTY); ATTSIM = DEFSIM = SIMULATE = 0; BKINDEX = 0;
CLEAR_TRYCYB(); CLEAR_PIECES(); ADIR = rand(0,3); T_NUM = terrain(DEFENDCTY) - 1;
if (CHOICE == 0 || CHOICE == 1) {                 // human attacker
   PREPARE_FOR_BATTLE(ATTPLAY);                   // splash: frame, flag of player (SET_FLAGS(p,0x91,0x30)), name, "HIT ENTER TO PREPARE FOR BATTLE", fade in/delay/out
   AGGLEVEL = 227; SET_ATTACKER_SCREEN(); SET_AGGRESSION(1); ATTGOAL = ATTMISSION;
} else {                                          // AI attacker
   AGGLEVEL = 227; ATTGOAL = 0; DECIDE_ON_GOAL(); SET_AGGRESSION(1);
   ATTCON = largest i < 10 with ACTIONLIST[i] != 10;
}
SET_POSITIONS(0, 10, 0, 0);
if (CHOICE == 0 || CHOICE == 2) { PREPARE_FOR_BATTLE(DEFPLAY); SET_DEFENDER_SCREEN(); }   // human defender
else {                                            // AI defender
   T_NUM = terrain(DEFENDCTY) - 1; READ_MATRIX(); INSERT_STRUCTURES();
   CLEAR_DEFENDER_ARRANGEMENT(); SET_DEFENDER_ARRANGEMENT(); DEFCON = DEFARR[0] - 10; M_ON();
   for (i = 10, o = 20; i < 20; i++) if (ACTIONLIST[i] != 10) {
      p = AP[ADIR][i-10];
      if (TURRETT && ACTIONLIST[i] != 0 && ACTIONLIST[i] != 1 && ACTIONLIST[i] != 4) p += BACKUP[ADIR];
      TRYCYB[o++] = p.x; TRYCYB[o++] = p.y;
   }
}
SET_DEF_POS(); SET_SIMVAR();
```
`_SET_PLAYER_TYPE`: `CHOICE = (LROWY[owner(att)] == 0x60 ? 2 : 0) | (LROWY[owner(def)] == 0x60 ? 1 : 0)`
(`_LROWY[p]` is the player-type row of the setup screen: 0x48 inactive, 0x60 computer, else human).
So CHOICE 0 = both human, 1 = defender AI, 2 = attacker AI, 3 = both AI (simulated, never ACTION.PRG).
`_SET_SIMVAR`: `SIMULATE = CHOICE==0 ? (ATTSIM && DEFSIM) : (ATTSIM || DEFSIM)`.
`_SET_AGGRESSION(_)`: `ATTAGG = _AG[i].level` for the i with `_AG[i].px == AGGLEVEL`; `_AG` = (175,0) (201,1)
(227,2) (253,3) (279,4) — AGGLEVEL is the slider's pixel x, default 227 => level 2.
`_DECIDE_ON_GOAL` (AI attacker): h = number of attacker TITAN/BEHEMOTH/CROSSBOLT; if (T_NUM==4 && h>=2)
ATTAGG = 253 (immediately overwritten by SET_AGGRESSION -> 2, i.e. no effect); else if (h > 2) ATTGOAL = 2
(CAPITAL); else ATTGOAL stays 0 (GUERRILLA).

### 1.8 Pre-battle screens (what the human can choose)
Common: `_SET_GENERAL_INFO` prints ANAME at (45,0xA8) colour 0 and DNAME at (45+78,0xA8) colour 1, "COUNTRY:" +
country name at (34,0x22)/(51,0x22), "TERRAIN:" + terrain name (`_GET_TERRAIN`: LANGS+14892+20*(t-1), also sets
COLOR1/COLOR2 from `_COLORS[(t-1)*2..]` = 184/178, 187/178, 184/178, 184/181, 187/178) at y 0x28.
`_SET_FLAGS(player, x, y)` blits the player's flag `_FP[player]` = (177,30)-(206,59), (209,30)-(238,59),
(241,30)-(270,59), (273,30)-(302,59), (177,61)-(206,90) from SCREEN1 to (x,y). Flags at (0xB0,8) attacker and
(0xFE,8) defender. `_GET_COORDINATES` waits for a mouse press+release and stores XCORD/YCORD.
`_LIST_CYBORGS(isAttacker)`: title "ATTACKING CYBORGS" at (0x67,0x22) / "DEFENDING CYBORGS" at (0x69,0xB8),
"STATUS" column +0x4C; rows 7 px apart starting y 0x34+0xB (att) / 0x36+0xB (def); per occupied slot prints
name (`_GET_UNIT_NAME`: LANGS+14327+15*type) and damage % (`_GET_DAMAGE(type,hits)`: 100 if hits > HL-2 else
float formula -> `_CYDAMAGE`); first entry highlighted (colour 0) and CHOSENCYB=0, attacker: ATTCON = that slot.
Records row y in `_CYBORGBOX[10]` and slot index in `_ATTARR[10]`.
`_PLOT_CONTROL_CYB(type, isAttacker)` blits the portrait `_CC[type]` (6 shorts: src x0,y0,x1,y1, dst dx,dy)
into the frame at (0x12,0x3D) attacker / (0xA3,0x3A) defender.

**Attacker screen `_SET_ATTACKER_SCREEN`** (menu by `_BUILD_ATTACK_MENU`: 9 frames `_ASF`, buttons `_ASB`,
"TARGET PRIORITY" at (167,101)). Loop on clicks:
* x < 0x84 && y > 0x6C: `_CHECK_ATT_CYBORG_LIST` — click a row (x 0x39..0x69, y in CYBORGBOX[i]..+5) to choose the
  controlled cyborg: `ATTCON = ATTARR[i]`, CHOSENCYB = i, redraw names (chosen in colour 0, others 1).
* mission buttons (y 0x3C..0x5A): x 0x48..0x67 `_SET_MONKEY` -> ATTMISSION = 0 "GUERRILLA ATTACK: CYBORG UNITS
  SEEK AND FIRE UPON ANY ENEMY STRUCTURES OR CYBORG UNITS"; x 0x88..0xA7 `_SET_BUILDING` -> 1 "STRUCTURE ATTACK:
  CYBORG UNITS ARE TO SEEK AND FIRE UPON ANY ENEMY STRUCTURES"; x 0xC8..0xE7 `_SET_CAPITAL` -> 2 "CAPITAL ATTACK:
  ... SEEK AND DESTROY ENEMY CAPITAL BUILDING"; x 0x108..0x127 `_SET_CONQUEST` -> 3 "CONQUER COUNTRY: ... FIRE UPON
  ALL ENEMY UNITS UNTIL DESTROYED". Each: `_CLEAR_OLD_SELECTION` (restore button rect `_AM[ATTMISSION]`
  = (124,154,153,183)->(73,61), (155,..)->(137,61), (186,..)->(201,61), (217,..)->(265,61) from SCREEN2),
  blit pressed button, `_CLEAR_MISSION_MESSAGE` (SCREEN2 (0xA8,0x91)-(0x122,0xA2) -> (0xA8,0x93)), print text.
  Default mission is GUERRILLA (0) — `_SET_MONKEY` is called when the screen opens.
* aggression: x 0xCC..0xDF, y 0x80..0x8A `_DECREASE_ATTACKER_AGGRESS` (if AGGLEVEL > 175: AGGLEVEL -= 26);
  x 0xE6..0xF9 `_INCREASE_ATTACKER_AGGRESS` (if AGGLEVEL < 279: += 26); slider knob drawn at x=AGGLEVEL, y 0x73;
  `_CHANGE_AGGRESS_MESSAGE` (text at (0xA8,0xAA)/(0xA8,0xB0)): 175 "UNCOMMITTED ATTACK / MAY NOT FOLLOW ORDERS",
  201 "LOOSELY COMMITTED ATTACK: / FOLLOW ORDERS LOOSELY", 227 "MODERATELY COMMITTED ATTACK: / FOLLOW ORDERS
  MODERATELY CLOSE", 253 "STRONGLY COMMITTED ATTACK: / FOLLOW ORDERS VERY CLOSE", 279 "EXTREMELY COMMITTED
  ATTACK: / FOLLOW ORDERS EXTREMELY CLOSE". Encoded as ATTAGG = 0..4 (GAMESTUFF+457); ACTION.PRG maps it through
  `_PRIOR[5] = {1,1,1,2,3}`.
* x 0x57..0x93, y 0xA..0x1C "SIMULATE": `_SWITCH_TO_SIMULATION(1)` -> ATTSIM = 1, exit.
* x 0xD..0x4C, y 0xA..0x1C "DONE": exit (fade out, XCORD=YCORD=0).

**Defender screen `_SET_DEFENDER_SCREEN`** (`_BUILD_DEFEND_MENU`: 5 frames; minimap frame (15,51)-(136,172),
list frame (163,99)-(291,184), portrait frame (163,58)-(199,88); "AUTO" button blit at (218,65); SIMULATE button
only drawn if ATTSIM || CHOICE != 0). It calls READ_MATRIX, INSERT_STRUCTURES, `_DRAW_MAP2` (schematic
minimap: 8x9 blocks of 16x14 px terrain texture from (17,53), attack-direction arrows, structure icons),
SET_DEFENDER_ARRANGEMENT, LIST_CYBORGS(0). Then:
* click inside minimap (x 18..137, y 54..173) while not done: must also be inside `_PL[dir]` (dir = ADIR, 4 for
  city) = (x0,x1,y0,y1): (17,138,53,147) (38,138,53,174) (17,138,78,174) (17,121,53,174) (17,138,53,174) — i.e.
  the defender's own side (attacker edge excluded); otherwise error sound. Places the next cyborg in DEFARR order
  at UI (XCORD-1, YCORD-1) into TRYCYB[20+2j], draws a 3x3 marker (SCREEN1 (0xB2,COLOR1)-(0xB4,COLOR1+2)),
  highlights the next list entry; when all placed: done, `_SET_DEFEND_CON(lastIdx,0,0)` marks the controlled
  cyborg's marker in COLOR2.
* "AUTO" (x 0xDA..0x119, y 0x41..0x53), only before any manual placement: `_AUTO_PLOT`: positions = AP[ADIR][j]
  for arrangement j (with sound + 10-tick delay per unit), then done.
* cyborg list (x 0xA4..0x126, y 0x64..0xBB) after done: `_CHECK_DEF_CYBORG_LIST` chooses the controlled cyborg
  (CHOSENCYB), swapping marker colours via `_SET_DEFEND_CON(newIdx, oldIdx, 1)`.
* "SIMULATE" (x 0x57..0x93, y 0xA..0x1C) if ATTSIM || CHOICE != 0: `_SWITCH_TO_SIMULATION(0)` -> DEFSIM = 1, exit.
* "DONE" (x 0xD..0x4C, y 0xA..0x1C) after done: `DEFCON = DEFARR[CHOSENCYB] - 10`, fade out, return.

---------------------------------------------------------------------------------------------------
## 2. Files passed to ACTION.PRG and back

### 2.1 `_GAMESTUFF` (487 bytes; 0x1E6 = 486 written as ACTSTUF.IAT) — written by `_START`, read by `_START2`
```
+0    MechRec[36]  8 bytes each:
        +0 short x      pixel position (TRYCYB*32 + 16, i.e. tile*32+16 = tile centre)
        +2 short y
        +4 byte  type   0..8 cyborg, 9..12 turret (LAC,HAC,NC,LRM), 0xFF none
        +5 byte  armour% 0..100 (out: hits*100/HL[type]; turrets 100); on return 101 = destroyed, 0 = never existed
        +6 byte  facing 0..7 (0 N, 2 E, 4 S, 6 W): attackers (ADIR*2+4)%8, defenders & turrets ADIR*2
        +7 byte  side   0 attacker (records 0..9), 1 defender (10..19) and turrets (20..31); 32..35 unused (type 0xFF)
      record i<20: type = ACTIONLIST[i] (0xFF if 10), armour% = ACTIONLIST[40+i]*100/HL[type]; pos from TRYCYB[2i]
      record 20+j (j<12): only if TRYCYBT[2j] != 0: pos TRYCYBT+16, type _TURT[fortlevel][j], armour 100, side 1
+288  short  TTYPE (T_NUM 0..4; selects ARCTIC4/CONIFER/DESERT3/VOLCANIC/CITY1F + STRUCT4 + WALL4/CITY2F tilesets)
+290  byte   attacker is computer (0xFF) / human (0)      = -(CHOICE & 2)/2
+291  byte   defender is computer (0xFF) / human (0)      = -(CHOICE & 1)
+292  byte   RESULT (written by ACTION): 0 nobody left, 1 defender wins, 2 attacker survived
+293  byte   ARET  attacker retreated (some attacker mech ended with status -4)
+294  byte   DRET  defender retreated
+295  Piece[26] 6 bytes: short px, short py, short kind (1 capital, 2 factory/enhancement, 0 none/destroyed)
+451  short ATTPLAY (attacker player id -> ACTION _PC1 colours)   +453 short DEFPLAY (_PC2)
+455  short EGA flag
+457  short ATTAGG 0..4          +459 short ATTGOAL 0 GUERRILLA 1 STRUCTURE 2 CAPITAL 3 CONQUER
+461  short control mode (PINFO+90)      +463 short ATTCON (attacker's controlled record)  +465 short DEFCON (+10 in ACTION)
+467  byte  sound type (PINFO+92)
+468  short[8] joystick calibration (PINFO+97..); ACTION reads +472/+474 as JOYAX/JOYAY, +480/+482 JOYBX/JOYBY,
      +468/+470/+476/+478 in _CONTROL
+484  short HUMANNUMBER (number of human players)      +486 pad
```
`_START` also: WRITE_MATRIX (matrix.dat 6240 bytes); `_PZ[0] = LOADSTRAT ? TACT_SELECTION : 4`; if !LOADSTRAT
`_SAVE_DATA_FILE2` (acts.DAT = byte-swapped `_MECHRECORD`, 17652 bytes); `_PZ+40 = T_NUM, +78 UPBOUND, +80 LOWBOUND,
+82 GAMECOUNT, +84 CLASS, +42 CHOICE, +2/+21 ANAME/DNAME, +74 LOADSTRAT, +76 KILLTONS`; write pinfo.iat (0x71) and
actstuf.iat (0x1E6); `_EXIT2`.

ACTION.PRG side (`_MAIN`/`_LOADCRUD`/`_UNLOADCRUD`): loads actstuf.iat, pinfo.iat (92 bytes, zeroes word 0 while
running), keyboard.iat, tilesets by TTYPE, matrix.dat (`_INITMAP`); its mech record (78 bytes) gets x,y, type(+0x21),
facing(+0x23), side(+0x2B), status(+0x27: -1 none, -2 destroyed, -4 retreated), hits(+0x4B) = armour%*maxarmour/100.
On exit: `_WRITE_FILE("matrix.dat")` (post-battle map), `_UNLOADCRUD` (records back, RESULT/ARET/DRET), writes
actstuf.iat and pinfo.iat (word 0 restored).

### 2.2 Reading the results: `_START2` and the result handlers
```
READ_MATRIX2();                                       // post-battle matrix (used for city piece counting / summary)
for i in 0..19: ACTIONLIST[i] = ACTIONTEMP[i] = (signed char) GAMESTUFF[i].type;
                ACTIONLIST[40+i] = GAMESTUFF[i].armour > 100 ? -1 : armour * HL[type] / 100;   // -1 = destroyed
PASSFLAG = 0; VICTOR = GAMESTUFF+292; ARET = +293; DRET = +294;
KILLTONS, UPBOUND, LOWBOUND, GAMECOUNT, CLASS, T_NUM, CHOICE, HUMANNUMBER, ANAME, DNAME restored from _PZ/_GAMESTUFF;
LOADSTRAT = 0; palette reset; PIECES[0..25] = GAMESTUFF+295.. (kind 0 where ACTION destroyed the piece)
```
`_SET_ACTION_PHASE` then: if LTERRAIN==5: CPCOUNT2 = count(32..59); CPCOUNT2 = CPCOUNT3 - 3*(CPCOUNT3-CPCOUNT2);
OLDCITYDAM = citydamage; if CPCOUNT1 != CPCOUNT2: citydamage = (float formula over CPCOUNT1/CPCOUNT2) capped at 100.
QUICKL = 1; `_CHECK_PIECES`; `_BATTLE_SUMMARY`; INSTRAT = LOADSTRAT = 1. Outcome: VICTOR 2 -> (DRET: DEFENDER_RETREAT)
PLACE_VICTOR(1), owner(def) = owner(att), country.+392 = owner(att), SOUNDS[def]--, SOUNDS[att]++; VICTOR 1 ->
(ARET: ATTACKER_RETREAT) country.+392 = owner(att), SOUNDS[att]--, PLACE_VICTOR(0); else both retreat handlers +
RESET_DESTIN (other agent).
`_LOAD_BAT`, `_SAVE_MAP`, `_I_MAP` are empty stubs. `_RESET_STRATEGY`: LOADSTRAT=0, INSTRAT=1, `_LOADMAP2` (reload
framebt3.gif strategy frame if needed, init doors, button10.gif), FADE_IN.

`_CHECK_PIECES` (inputs to `_ADJUST_DATA_STRUCT`): BDLEVEL = 0. If PIECES[0].kind == 0 and slot0 == 1 -> capital
destroyed: `_CLEAR_STRUCT(DEFENDCTY)`, BDLEVEL = 30 and return. Else for slots k = 1..6 with PSTRUCT[k] != 0: n =
(PSTRUCT[k] == 7) ? 1 : PSTRUCT[k]-2 pieces starting at running index p (starts at 1); count destroyed (kind 0) ->
di, BDLEVEL += di; if di > 0 call `_ADJUST_DATA_STRUCT()` (no args; it reads DEFENDCTY, slots at +192.., +388 and
the globals BDLEVEL/PSTRUCT); p += n.  (Piece count assumed per factory: 3->1, 4->2, 5->3, 6->4 — see open questions.)

### 2.3 `_LOAD_DISK` / `_AOPEN` / `_GETEXT` (floppy-disk handling only)
`_AOPEN(name, mode)`: try `open(name, 4)`; on failure pick the disk by extension (`_GETEXT` = pointer after '.'):
"ex*"/"i*" -> 0 "Main", "ei*" -> 2 "EGA", "g*"/"d*" -> 1 "VGA", "m*" -> 3 "Map" (names 8 bytes each at DGROUP 0x2A2),
`_LOAD_DISK(disk)` shows "Insert %s Disk / And press any key" (text mode box), loops until the file opens, then
reopens with `mode`. The data files used by this scope: *.MAP (Map disk), door3.gif, button10.gif, framebt3.gif,
topw1b..topw4b.gif (`_TLOADMAP`: strategy map halves into SCREEN3 via `_MAKEMAP`), matrix.dat, actstuf.iat,
pinfo.iat, pass2.dat, acts.DAT, MECHS.DAT (new game), SCORES.DAT, GAME%d.DAT.

---------------------------------------------------------------------------------------------------
## 3. Practice modes: Showdown and Survivor

Entry: main menu SELECTION == 3 -> `_LOAD_SCORES`, CONTINUE = 1, QUICKL = 0, then loop `{ _DO_ACTION(); _GET_TACTICAL_SELECTION(); }`
while CONTINUE; `_SAVE_SCORES` on exit. `_DO_ACTION` (UI): LOADSTRAT = 1, loads button10.gif, `_BUILD_TACT_MENU`
("SELECT A BATTLE PRACTICE SCENARIO", "SHOWDOWN" button frame (148,48)-(299,83), "SURVIVOR" (148,94)-(299,190),
score box (33,142)-(120,162) with "LEVEL / YOUR SCORE / CYBORG"), prints the 9 cyborg names and the high-score table
(for class i: name `_PNAME[i]` 15 chars at (0xCF, 0x8A+6i), score `_PSCORE[i]` 8 chars at (0xFA,..), level `_PLEVEL[i]`
8 chars at (0x11E,..)) plus the current GAMECOUNT-1, KILLTONS and CLASS name. Skipped when PASSFLAG > 0.
`_GET_TACTICAL_SELECTION`: PASSFLAG 1 -> TACT_SELECTION 1, PASSFLAG 2 -> 2 (resume after ACTION.PRG); otherwise click:
x 0x1D..0x44 & y 0x30..0x56 -> 1 Showdown, y 0x5E..0x84 -> 2 Survivor; x 0xF2..0x12B & y 5..0x12 "exit" -> 4
(CONTINUE = 0, TACTICAL = 0); two toggle buttons at y 0xB5..0xBF: x 0x2C..0x4E mouse / x 0x56..0x78 joystick
(JOYSTICKCONTROL, `_NEW_MOUSE`). Then `_DO_SHOWDOWN` / `_DO_SURVIVOR` (= `_CHOOSE_CYBORG`), FADE_OUT.
SCORES.DAT = `_PSCORE[9][8]` (72) + `_PNAME[9][15]` (135) + `_PLEVEL[9][8]` (72) as ASCII strings (279 bytes).

### 3.1 Showdown (`_DO_SHOWDOWN`) — WAR HAMMER (attacker, player 0 = ... see below) vs DESTROYER
```
if (!PASSFLAG) {
  SET_TACT_VARIABLES();                 // ADIR=0 DEFCON=0 TACTICAL=1 TURRETT=0 ATTAGG=2 ATTGOAL=0 ATTPLAY=rand(1,4) DEFPLAY=0 ATTCON=0; TDIF/CITY flags
  GET_T_PLAYERS();                      // CHOICE cycles 0 "HUMAN VS. HUMAN", 1 "HUMAN VS. COMPUTER", 2 "COMPUTER VS. HUMAN", 3 "DEMO"
  LTERRAIN = T_NUM;
  if (CHOICE == 3) {                    // demo: random terrain and money
     T_NUM = rand(0,4); DEFENDCTY = _SDCTRY[T_NUM] = {1,61,29,34,2}[T_NUM]; READ_MATRIX();
     BUYMONEY = rand(5,100) * rand(10,100) * rand(10,100);
  } else { CHOOSE_TERRAIN(); MONEY_FOR_TACT(); }
  COUNTRY1 = 0; country(0).+388 (bays) = 10; PLAYERNUM = 1;                        // country record 0 is the shop
  if (CHOICE == 1 || CHOICE == 3) { INIT_BAYS(); COMPUTER_BUY(); RESOLVE_MECHS2(1); }   // player 1 = defender "DESTROYER"
  else { TACT = 1; SHOW_CYB_MENU(); TACT = 0; }                                         // human buys (build menu, other agent)
  DNAME = "DESTROYER"; PLAYERNUM = 2;
  if (CHOICE > 1) { INIT_BAYS(); COMPUTER_BUY(); RESOLVE_MECHS2(2); } else { TACT = 1; SHOW_CYB_MENU(); TACT = 0; }
  ANAME = "WAR HAMMER";
  SORT_ACTIONLIST(0,10); SORT_ACTIONLIST(10,20);
  SET_POSITIONS(0,10,1,0);              // attackers along the top edge  (APOS[0]=(39,3), AOFF (5,0))
  SET_POSITIONS(10,20,1,2);             // defenders along the bottom edge (APOS[2]=(39,74))
  if (QUICKL) { if (LTERRAIN == T_NUM) TDIF = 0; else if (LTERRAIN == 4 || T_NUM == 4) CITY = 1; }
  ATTACKCTY = (DEFENDCTY+1) % 72; owner(ATTACKCTY) = 0; owner(DEFENDCTY) = 1;
  CLEAR_PIECES(); _START();             // no INSERT_STRUCTURES: bare terrain map, no turrets, no pieces
}
_START2(); SELECTION = 3; PASSFLAG = 0; QUICKL = 1; FADE_OUT; (NEW_MOUSE if joystick)
ATTACKCTY = (DEFENDCTY+1)%72; owner(ATTACKCTY)=0; owner(DEFENDCTY)=1; player0.name = ANAME; player1.name = DNAME;
BATTLE_SUMMARY(); TACTICAL = 0; LOADSTRAT = 1;
```
Rules: CHOICE 1 => attacker human/defender computer; 2 => attacker computer; 3 => demo (both AI, still run in
ACTION.PRG since SET_PLAYER_TYPE is not used here). Money (`_MONEY_FOR_TACT` -> `_GET_MONEY2`): "ENTER THE AMOUNT MONEY
TO ALLOCATE FOR EACH PLAYER", BUYMONEY starts at 2500; buttons (x 0x73..0x8A minus / 0xB4..0xCB plus): y 0x6E..0x7C
+-500, y 0x5E..0x6C +-10000, y 0x4E..0x5C +-100000; clamped to [2500, 1000000]; OK at (0xBF..0xD0, 0x1A..0x2D).
Both players get the same BUYMONEY. Terrain (`_CHOOSE_TERRAIN` -> `_GET_T`): "TERRAIN SELECTION", T_NUM cycles 0..4
with the -/+ buttons (y 0x6E..0x7C), OK -> SET_TACT_VARIABLES, DEFENDCTY = _SDCTRY[T_NUM] (so the maps used are
ARCT_1, CONI_61, DEST_29, VOLC_34, CITY_2), READ_MATRIX. Computer purchase: `_COMPUTER_BUY`: TEMPVAULT = BUYMONEY;
n = rand(0,9); n times `_TRYBUY(rand(0,8))`, then `_TRYBUY(0)` until 10 bays tried; `_TRYBUY(t)`: first type >= t
(t..8, TITAN first) whose `_UNITCOST` fits in TEMPVAULT is `_INSERT_BAY`ed into `_TEMPBAY[10]` (init 10 = empty by
`_INIT_BAYS`) and paid. `_RESOLVE_MECHS2(p)`: ACTIONLIST[(p==1?10:0)+i] = TEMPBAY[i] % 100, hits = `_ARMOR(type)` =
HL[type]. The human shop is the normal build menu with TACT = 1 (BUYMONEY as vault).
After the battle: `_BATTLE_SUMMARY` only (no score), back to the practice menu.

### 3.2 Survivor (`_DO_SURVIVOR` = `_CHOOSE_CYBORG`)
```
if (!PASSFLAG) {
  "CYBORG SELECTION / SELECT CYBORG FOR SURVIVAL / CYBORG:"; GET_MECH();     // CLASS cycles 0..8 with -/+ (y 0x6E..0x7C), portrait _CC[CLASS] in frame (0x40,0x8C), name at (0x87,0x64); OK
  GAMECOUNT = 1; VICTOR = 1; UPBOUND = 8; LOWBOUND = 8; KILLTONS = 0;
  SET_POSITIONS(0,10,1,0); SET_POSITIONS(10,20,1,2);
} else goto resume;
while (VICTOR == 1) {                                   // player (defender) survived -> next level
  SET_TACT_VARIABLES();                                 // Survivor: DEFENDCTY = rand(1,72), T_NUM = terrain(DEFENDCTY)-1, LTERRAIN = old T_NUM
  READ_MATRIX(); LOAD_SURVIVOR_BATTLE();
  SET_POSITIONS(0,10,1,0); SET_POSITIONS(10,20,1,2);    // enemies top edge, player bottom edge (TRYCYB[20..21])
  CHOICE = 2;                                           // attacker computer, defender human
  ANAME = player0.name; CLEAR_PIECES(); _START();       // -> ACTION.PRG
resume:
  _START2(); player0.name = ANAME; PASSFLAG = 0; TACT_SELECTION = 2; LOADSTRAT = 1; QUICKL = 1; FADE_OUT;
  for i in 0..9: if (ACTIONLIST[40+i] == -1) KILLTONS += HL[ACTIONTEMP[i]];     // tonnage of destroyed enemies
  ACTIONLIST[60] = HL[CLASS];                           // player's cyborg fully repaired
  GAMECOUNT++;
}
// player lost (VICTOR != 1)
if (atoi(PSCORE[CLASS]) < KILLTONS) {                   // new high score for this cyborg class
  READ_HUMAN_NAME(0, 0x54, 1) -> player0.name; PLEVEL[CLASS] = itoa(GAMECOUNT-1); PSCORE[CLASS] = itoa(KILLTONS);
  PNAME[CLASS] = player0.name; SAVE_SCORES();
}
```
`_LOAD_SURVIVOR_BATTLE` (level = GAMECOUNT): if GAMECOUNT > 10 and UPBOUND > 0: UPBOUND--; else if GAMECOUNT < 10 and
LOWBOUND > 0: LOWBOUND--. Clear ACTIONLIST (type 10, hits 0). Defender slot 10 = CLASS with hits HL[CLASS].
n = min(GAMECOUNT, 10) enemies: type = rand(LOWBOUND, UPBOUND) (a 4 CROSSBOLT is replaced by 5 MARS), hits = HL[type].
So level 1 = one MERCURY, level 2 = two from {CYCLOPS..MERCURY}, ... level 9 = nine from {TITAN..MERCURY} (LOWBOUND 0),
level 10 = ten of any type, level 11+ = ten from {0..UPBOUND} with UPBOUND shrinking toward TITAN only.
Score = total armour of enemies destroyed across all levels; one high score per chosen class (name, score, level).

---------------------------------------------------------------------------------------------------
## 4. One-line summaries of the pure UI functions and menus

* `_BUILD_PREPARE_BATTLE`: frame (100,35)-(217,136), flag frame (142,45)-(175,78), name frame (132,84)-(185,90),
  "HIT ENTER TO / PREPARE FOR / BATTLE" at (108,98/110/122).
* `_BUILD_DEFEND_MENU`: full frame (0..317,0..197 fill colour 50), 5 `_ASF` frames, 5 `_ASB` buttons, minimap frame
  (15,51)-(136,172), list frame (163,99)-(291,184), portrait frame (163,58)-(199,88), AUTO button (218,65),
  DONE button blit (85,10)+(105,15) only when a SIMULATE choice is relevant.
* `_BUILD_ATTACK_MENU`: 9 `_ASF` frames, `_ASB` buttons 0..21 (mission buttons 18..21 at (73/137/201/265, 61)),
  "TARGET PRIORITY" (167,101).  `_ASF[9]` = (x0,y0,x1,y1,colour,style): (15,31,98,43,112,5) (163,43,216,48,128,4)
  (241,43,294,48,128,4) (173,5,206,38,96,3) (251,5,284,38,96,3) (19,100,151,185,112,5) (18,61,54,91,96,3)
  (172,112,277,120,112,5) (159,142,297,185,112,5).
* `_BUILD_TACT_MENU`, `_BUILD_MENU2` (load-game menu, `_M2`/`_M2B` tables), `_BUILD_CYB_MENU` (build/shop menu:
  WGT/COST/B-TIME boxes, 3+ bay frames), `_BUILD_STATS_MENU` (statistics/options: TOTAL ASSETS, TOTAL CYBORGS,
  MAP DISPLAY FOG OF WAR/SHOW ALL, PLANET TYPE POOR/AVERAGE/RICH, INACTIVE players), `_BUILD_BATTLE_STATS`
  ("BATTLE SUMMARY", ATTACKER/DEFENDER CASUALTIES boxes (24,85)-(70,149) and (247,85)-(293,149)): layout only.
* `_RESET_CYBORG_BOXES` (CYBORGBOX[10] = -1), `_CLEAR_DEFENDER_ARRANGEMENT` (DEFARR[11] = -1),
  `_CLEAR_ATTACKER_ARRANGEMENT` (ATTARR[10] = -1), `_CLEAR_AGGRESS_MESSAGE` (SCREEN2 (0xA8,0xA4)-(0x122,0xB0) ->
  (0xA8,0xAA)), `_SWITCH_TO_SIMULATION(side)` (ATTSIM/DEFSIM = 1 + button blit (0x129,0xBF)-(0x13F,0xC7) ->
  (0x6A,0xF)), `_LOADMAP` (button10.gif/door3.gif reload), `_TLOADMAP` (world map screens).

---------------------------------------------------------------------------------------------------
## 5. Open questions
1. Slot value 6 vs 5 (both drawn with the 10x8 quadro-post picture; type 5 has one core replaced by `_FAC3`
   tile 105..107). Probably 5 = quadro-post, 6 = quadro-post + upgrade, but the economy agent should confirm.
   Related bug: `_INSERT_STRUCTURES` records 4 pieces for a type-5 factory (test is on `_FAC[]`, before the FAC3
   overwrite) while `_CHECK_PIECES` consumes only 3 (PSTRUCT-2), so piece->slot attribution for later slots is off
   by one after a type-5 factory. Decide whether to replicate.
2. `_SET_DEF_POS` bounds test `(y > 74 && clear)` — almost certainly a precedence bug; recommended: treat y>74 as
   out of bounds.
3. `_DECIDE_ON_GOAL` writes ATTAGG = 253 for a city map with >= 2 heavy attackers, but `_SET_AGGRESSION(1)` runs
   right after and resets ATTAGG to 2; probably meant AGGLEVEL = 253 (level 3). Original behaviour: no effect.
4. The exact ACTION-side meaning of ATTAGG via `_PRIOR` = {1,1,1,2,3}, of GAMESTUFF+461 control values, and of the
   joystick calibration shorts belong to the ACTION.PRG agent.
5. `_DRAW_MAP2`'s schematic minimap (structure icon positions, `_PLOT_*ARROWS`) was not reverse-engineered in detail;
   only the click->tile mapping (`tile = ((px-1)-17)/2 + 10 + BO.x`, `((py-1)-53)/2 + 9 + BO.y`) matters for logic.
6. Who sets `_LROWY[p]` to 0x60 (computer) vs the human value — `_CHANGE_BUTTONS` (game setup; other agent).
7. `_FNAMES` ("arctic1.MAP", "city4.MAP", ...) is unreferenced; MAP file names come from LANGS+14297.. prefixes.
8. `_SET_TERRAIN_LOAD`'s QUICKL/TDIF/CITY flags are consumed by the simulation/summary tile loaders (other agent).
