# ACTION.PRG — simulation core (reverse-engineered)

Source: action.asm (Turbo C, large model). All constants below were read from the disassembly /
re_action_tables.txt. Helper dumps of every function discussed are in re/adump/*.txt (made with re/afn.sh).
Conventions: `short` = int16, `byte` = int8 unless stated; positions are pixels on an 80x78-tile map of 32x32 tiles
(map 2560 x 2496 px); tile (tx,ty) = (x/32, y/32); map byte index = ty*80 + tx.
ACTION contains NO random numbers (no _RAND_NUM) — the battle is fully deterministic given inputs.

---------------------------------------------------------------------------------------------------
## 0. Overall structure, timing, end of battle, files (_MAIN)

```
_MAIN:
  LOAD_FILE2("actstuf.iat", &GAMESTUFF, 486)         // handoff struct from CYBOR.PRG
  LOAD_FILE2("pinfo.iat",   &P, 92); saved = P[0]; P[0] = 0; WRITE_FILE2("pinfo.iat", &P, 92)
                                                     // P[0] zeroed while the battle runs (crash marker), restored at exit
  EGA = GAMESTUFF.s[455]; SCRBOOT(EGA); SETFADE(0)
  LOADCRUD()                                         // GAMESTUFF -> MECHS[], globals (see §1.3)
  SET_MFDB()                                         // allocates VSCREEN, MAPLOC (80*78 tile bytes), HITLOC (80*78 tile HP bytes) ...
  palette setup (TEMPPALLET), MBOOT()                // MBOOT hooks INT 8 and INT 9, reprograms PIT ch0 divisor 0x100
  LOAD_FILE2("keyboard.iat", &J1, 52)                // 2 x 26-byte key sets
  GIFLOAD mechset8.gif -> MECHSET (320x200), newmars.gif -> NEWMARS, newmars2.gif -> NEWMARS2
  GIFLOAD FNAMES[TTYPE].struct -> SHAPES2 (tiles 60..119), FNAMES[TTYPE].wall -> SHAPES3 (120..179),
          FNAMES[TTYPE].terrain -> SHAPES1 (0..59)   // FNAMES rows (60 bytes, 4 x 15-char names) per terrain:
                                                     // 0 arctic1.MAP ARCTIC4.gif STRUCT4.gif WALL4.gif
                                                     // 1 carnif1.MAP CONIFER.gif STRUCT4.gif WALL4.gif
                                                     // 2 DESERT1.MAP ARCTIC4.gif STRUCT4.gif WALL4.gif
                                                     // 3 val1.MAP    volcanic.gif STRUCT4.gif WALL4.gif
                                                     // 4 city4.MAP   city1f.gif  STRUCT4.gif city2f.gif
  MAKESIZES()                                        // per-type/dir sprite bounding boxes (§3.4)
  GIFLOAD batframe.gif (HUD), palettes (COLOUR_PALETTE, TERRAIN_COLOURS[TTYPE], PLAYER_COLOURS[PC1]/[PC2])
  INITMAP()                                          // LOAD_FILE("matrix.dat", MAPLOC); HITLOC[i] = WALL_POWER[MAPLOC[i]] for i<6240
  INITMECHS()                                        // BOOTMECH(0..31); SWITCHMECH(CURMECH[p]) for p=0,1
  SETPLAYER(0);SETMAP();SETPLAYER(1);SETMAP(); DRAW_ALL_MECHS; DRAW_BULLETS; PLOTHIT x2; PLOTHEAT; DOALLRADAR; DISPLAYMAP x2
  sound init (player.bin + adfx.mus (SOUNDT==1, AdLib) / rolfx.rl (SOUNDT==2, Roland); SOUNDT==0 PC speaker)
  draw player flags, wait for key (GETCH), COPY_SCREENS2
  done = 0; LTIME = CLOCK()
  while (!done) {
     SETPLAYER(0); SETMAP(); SETPLAYER(1); SETMAP();     // scroll each player's 6x6-tile viewport to its current mech
     done = DRAW_ALL_MECHS();                            // draws mechs AND detects end of battle (returns -1)
     DRAW_BULLETS(); PLOTHIT(CURMECH[0],0); PLOTHIT(CURMECH[1],1); PLOTHEAT();
     SETPLAYER(0); DISPLAYMAP(); SETPLAYER(1); DISPLAYMAP();
     CHECKHOLDS(0); CHECKHOLDS(1);                       // §4 (hold flag per side)
     CONTROL();                                          // waits for >=1 clock tick, runs DOCLICK once per elapsed tick, reads input
     if (EGA==1) REFRESH(); TOGGLE_SCREENS2(); DOALLRADAR();
  }
  SETFADE(0); WRITEPAL
  WRITE_FILE("matrix.dat", MAPLOC)                       // battle-damaged tile map written back (6240 bytes)
  UNLOADCRUD()                                           // MECHS -> GAMESTUFF (results, §1.3)
  WRITE_FILE2("actstuf.iat", &GAMESTUFF, 486)
  P[0] = saved; WRITE_FILE2("pinfo.iat", &P, 92)
  EXIT2()
```
There is NO reference to pass2.dat in ACTION.PRG; results go back only via actstuf.iat (and matrix.dat / pinfo.iat).

### Timing
* MBOOT programs PIT channel 0 with divisor 0x0100 → 1193182/256 = 4660.9 interrupts/s. The INT 8 handler
  (seg 0x55d:0x56E) does PC-speaker sample output every interrupt, sound-queue work every 32nd, and every
  256th interrupt increments the word `CLOCK()` returns (cs:0xF) → **18.2 Hz tick** (standard BIOS rate), then
  handles mouse/joystick.
* `_CONTROL`: `while (CLOCK()==LTIME); n = CLOCK()-LTIME; for i in 1..n: DOCLICK(); LTIME = CLOCK();`
  → **the simulation step (DOCLICK) runs exactly once per 18.2 Hz tick**, catching up if rendering was slower.
  Rendering runs at most once per tick. Feet animation uses CLOCK() directly; everything else uses per-mech counters.
* `_DELAY2(n)` waits n/3 ticks (unused in the loop). `_CLOCK2` (INT 1Ah) is unused.

### End of battle
`_DRAW_ALL_MECHS` returns -1 (→ loop exit) when the *current* mech of a player (CURMECH[p]) has
status -4 (retreated off map) or has just finished exploding (status -2 → -1), and `BUMPMECH(p)` finds no other
mech `k` in 0..19 with status >= 0 and player == p. Turrets (indices 20..31) never count; a side whose
last cyborg dies/retreats loses immediately even if turrets survive. Nothing else ends the battle (no timer,
no capital-destroyed condition inside ACTION; structure destruction only clears GAMESTUFF structure flags).

---------------------------------------------------------------------------------------------------
## 1. Record layouts

### 1.1 Mech record — `_MECHS`, 32 records x 78 (0x4E) bytes = 2496
Indices: 0..9 player 0 (attacker) cyborgs, 10..19 player 1 (defender) cyborgs, 20..31 fortification turrets
(code uses m/10 as the player for retreat/hold/retreat-direction; the player field is also stored).

| off  | type   | name (mine)      | meaning |
|------|--------|------------------|---------|
| 0x00 | short  | x                | sprite centre x, pixels (map coords) |
| 0x02 | short  | y                | sprite centre y |
| 0x04 | byte   | nbullets         | own bullets alive (inc in FIREWEAPON per shot, dec in ADDNULL when a bullet is freed). Cannot fire if >= 16 |
| 0x05 | byte   | avoid_dir        | obstacle-avoidance heading 0..7 (CHECKOBSTICAL/DOMECH) |
| 0x06 | byte   | (unused)         | |
| 0x07 | byte   | avoid_turn       | +1 / -1 : side chosen to walk around an obstacle |
| 0x08 | byte   | crowd            | count of friendly mechs blocking probe positions (TESTPOSITION); avoidance aborted when > 32 |
| 0x09 | short[4] | missile[4]     | bullet pointers of the LRM salvo in flight (slot i) |
| 0x11 | byte[4]  | missile_shot[4]| shot number (muzzle table row) of each in-flight LRM |
| 0x15 | 4 bytes| (unused)         | |
| 0x19 | short  | target           | AI target: 0..31 = mech index; -(k+1) = structure k of GAMESTUFF structure list; -40 (0xFFD8) = none |
| 0x1B | short  | tdist            | set by GETD: Chebyshev-ish distance (|dx| or |dy| part beyond ±7) to target; SELECTWEAPON compares weapon range against it |
| 0x1D | short  | nmissiles        | number of LRMs currently in flight & under guidance (0..shots) |
| 0x1F | short  | reload           | ticks until the mech may fire again |
| 0x21 | short  | type             | 0..8 cyborg (TITAN..MERCURY), 9..12 turret types (see STATMECH) |
| 0x23 | short  | dir              | facing 0..7 (0=N, clockwise, see _DIR) |
| 0x25 | short  | want_dir         | desired direction this tick (copy of JOY[m]) |
| 0x27 | short  | status           | 1 moving, 0 stopped/blocked/shutdown, -1 dead (removed), -2 exploding, -4 retreated off map |
| 0x29 | short  | lockout          | 0 normal; -3 = heat shutdown (set when heat>100, cleared at a cooling tick when heat<75). While -3: no move, no fire, no missile steering |
| 0x2B | short  | player           | 0 attacker / 1 defender |
| 0x2D | short  | (unused)         | |
| 0x2F | short  | heat             | 0..150 |
| 0x31 | short  | anim             | animation counter: ++ every tick; reset to 0 on fire and on explosion start |
| 0x33 | short  | in_water         | 1 if the tile under the centre is tile 53 (0x35) (water / lava), else 0 |
| 0x35 | short  | (unused)         | |
| 0x37 | short  | weapon           | current weapon slot 0..3 |
| 0x39 | short  | (unused)         | |
| 0x3B | short  | hold             | 1 = hold position (behave like a turret: turn & shoot, never advance). Set per side by CHECKHOLDS every frame (see §4) |
| 0x3D | byte   | avoid_cnt        | obstacle avoidance step counter (CHECKOBSTICAL ++ ; DOMECH --). -1 set by TESTSHIT on map border |
| 0x3E | byte   | bump             | -1 none; 0..31 = mech we collided with (TESTC); other values = -(tile index) of a wall tile we decided to shoot (TESTSHIT) |
| 0x3F | short  | goal_x           | goal position used while bump != -1 (other mech's position, or wall tile centre) |
| 0x41 | short  | goal_y           | |
| 0x43 | short[4] | ammo[4]        | rounds per weapon slot; -1 = unlimited |
| 0x4B | short  | hits             | remaining armour points (0..STATMECH.armor) |
| 0x4D | byte   | redraw           | countdown; when it hits 0 and no missiles in flight the viewport is re-centred (DRAWMAP) — set to explosion length by CLEARMISSLE |

### 1.2 Bullet record — `_BULLETS`, 800 x 15 bytes = 12000; singly linked list
`FPTR` = head of the active list, `NULLPTR` = head of the free list, `NULLRECORD` (all-zero record at 0x5CEC) = list terminator.
`LPTR` = number of records ever handed out (grows until 800; "bullet overload" fatal if exceeded).

| off | type  | name   | meaning |
|-----|-------|--------|---------|
| 0   | short | next   | pointer to next bullet (or &NULLRECORD) |
| 2   | short | x      | pixel x |
| 4   | short | y      | pixel y |
| 6   | int8  | vx     | velocity px/tick (= FIRESTUF[dir].dx * speed / 10) |
| 7   | int8  | vy     | |
| 8   | int8  | dir    | 0..7 |
| 9   | int8  | wtype  | weapon type 0..7 |
| 10  | int8  | owner  | mech index |
| 11  | short | age    | ticks since fired while >= 0; negative = exploding, counts up to -1 then deleted |
| 13  | short | range  | remaining travel budget in px (|vx|+|vy| subtracted per tick); expires when it goes negative |

`ADDBULLET(x,y,vx,vy,dir,wtype,range,owner)` pops from the free list (else takes a fresh record), pushes it at the
head (FPTR), age = 0. `DELETEBULLET(prev)` unlinks the bullet following `prev` (prev == &NULLRECORD means the head)
and `ADDNULL` pushes it on the free list and decrements the owner's `nbullets`.

### 1.3 `_GAMESTUFF` = ACTSTUF.IAT (486 bytes) as read/written by ACTION
(Stored little-endian in memory; ACTION does no byte swapping on it.)

| off | type | meaning |
|-----|------|---------|
| 0..255 | 32 x 8 bytes | mech slots i = 0..31: `short x; short y; int8 type (-1 = empty); int8 health (0..100 %); int8 dir (0..7); int8 player`. LOADCRUD: `hits = STATMECH[type].armor * health / 100` (32-bit math), status 0, or if type == -1: status -1, hits 1. UNLOADCRUD writes back x,y,type,player,dir and health: for status -1/-2: `hits==0 ? 101 : 0` (101 = destroyed in battle, 0 = was an empty slot); otherwise `hits*100/armor` (retreated mechs keep their health). |
| 256..287 | 32 bytes | not touched by ACTION |
| 288 | short | TTYPE terrain 0 arctic, 1 conifer, 2 desert, 3 volcanic, 4 city (written back unchanged) |
| 290 | int8 | PSTAT[0]: player 0 control, -1 = computer, else human (value copied into CFL of controlled mechs) |
| 291 | int8 | PSTAT[1] |
| 292 | int8 | RESULT (written by UNLOADCRUD): 2 = attacker has a mech with status >= 0, 1 = only defender has, 0 = neither |
| 293 | int8 | ARET: 1 if any attacker mech has status -4 (retreated) |
| 294 | int8 | DRET: same for defender |
| 295..450 | 26 x 6 bytes | structure list k = 0..25: `short x; short y; short alive`. x,y in pixels of the tile's top-left (code adds 16 for the centre). Alive flag cleared by DOCLICK when the tile at (x/32,y/32) is destroyed by a bullet (PLAYCRIT sound). Used as AI targets (target = -(k+1)); entry 0 is weighted as the primary goal (GOALTABLE column 1) |
| 451 | short | PC1 colour index for player 0 (PLAYER_COLOURS row) |
| 453 | short | PC2 colour index for player 1 |
| 455 | short | EGA flag (0 VGA / 1 EGA) |
| 457 | short | aggression index 0..4 → ATTAGG = PRIOR[idx] = {1,1,1,2,3} |
| 459 | short | ATTGOAL 0..3 (row of GOALTABLE) |
| 461 | short | joysticks: 0 none, 1 one, 2 two |
| 463 | short | initial CURMECH[0] (0..9) |
| 465 | short | initial CURMECH[1] (0..9; +10 added) |
| 467 | int8 | SOUNDT 0 PC speaker, 1 AdLib, 2 Roland, <0 none |
| 468,470 | short | joystick A centre x,y;  472,474: half-ranges (JOYAX, JOYAY) |
| 476,478 | short | joystick B centre;      480,482: half-ranges |
| 484 | short | HUMPLAYER (1 → with no joystick both players read key set 1) |

Also: `RDIR[0] = (MECHS[0].dir+4) % 8`, `RDIR[1] = (MECHS[10].dir+4) % 8` (retreat directions = opposite of the
leaders' initial facing). matrix.dat (80x78 tile bytes) is loaded by INITMAP and written back at the end.

---------------------------------------------------------------------------------------------------
## 2. Static tables

### 2.1 `_WEAPONS` — 8 x 11 shorts (22 bytes)
Weapon type numbering (derived from STATMECH weapon lists ↔ muzzle-table names, and from DOCLICK special cases 6/7):

| w | name | range px | speed px/tick | damage | heat/shot (firer) | heat on target | HUD icon frame | reload ticks | PC-spk snd | explosion anim | Roland snd | AdLib snd |
|---|------|------|----|----|---|----|------|---|---|---|----|----|
| 0 | HLAS heavy laser    | 700 | 16 | 3  | 1 | 3  | 1243 | 3 | 2 | 1 (HLASER_EXPLOSION, 7f) | 6  | 12 |
| 1 | PPC                 | 700 | 16 | 20 | 4 | 10 | 1244 | 7 | 1 | 2 (EXPLOSION, 13f)       | 12 | 4  |
| 2 | LLAS light laser    | 250 | 16 | 1  | 1 | 1  | 1253 | 2 | 3 | 0 (DITTY, 5f)            | 5  | 10 |
| 3 | HFIRE flamer        | 80  | 8  | 0  | 1 | 8  | 1248 | 1 | 8 | 2                        | 2  | 3  |
| 4 | LAUTO light autocannon | 350 | 6 | 4 | 0 | 0  | 1249 | 2 | 4 | 2                        | 9  | 1  |
| 5 | HAUTO heavy autocannon | 350 | 6 | 8 | 0 | 0  | 1247 | 2 | 5 | 2                        | 12 | 18 |
| 6 | SMIS short-range missile | 400 | 10 | 16 | 1 | 2 | 1245 | 4 | 6 | 2                      | 7  | 3  |
| 7 | LMIS long-range missile  | 3200 | 15 | 10 | 2 | 2 | 1246 | 4 | 7 | 2                      | 7  | 3  |

Field index → offset: [0] range (+0), [1] speed (+2), [2] damage (+4), [3] fire heat (+6), [4] impact heat (+8),
[5] HUD icon (+10, DRAW_SPRITE2 frame, 16x16 MECHSET cell frame-0x400), [6] reload (+12), [7] PC-speaker sound id (+14),
[8] explosion animation = ANIMATION[25 + value] (+16), [9] Roland sound (+18), [10] AdLib sound (+20).
Sound index actually used: `WEAPONS[w].s[9 + (2 - SOUNDT)]` → AdLib (SOUNDT 1) uses +20, Roland (2) uses +18.
The flamer does 0 armour damage: it is a pure heat weapon (+8 heat per hit). `_WEAPON_RANGE` {150,150,50,2,15,25,50,0}
is never referenced by code.

### 2.2 `_STATMECH` — 13 x 18 shorts (36 bytes), rows 0..8 cyborgs, 9..12 turrets
Field index → meaning: [0] feet animation (13 = MED_FEET, 14 = LIGHT_FEET, -1 none), [1] armour, [2] speed px/tick,
[3..6] weapon type per slot (-1 empty), [7..10] ammo per slot (-1 unlimited), [11..14] shots per volley per slot,
[15] cooling divisor, [16] unused by ACTION (build/class value?), byte[34] sprite size (32/16 px; muzzle base offset),
byte[35] collision size (collision radius input). `_TRANS_TABLES[type*4+slot]` = pointer to muzzle offset table.

| type | name | feet | armour | speed | weapons (slot0..3) | ammo | shots/volley | cool div | size | coll | muzzle tables |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | TITAN     | 13 | 750 | 2 | PPC, HAUTO, LMIS, -     | -1,200,20,0  | 2,2,2,0 | 50 | 32 | 22 | TITAN_PPC, TITAN_HAUTO, TITAN_LMIS |
| 1 | BEHEMOTH  | 13 | 600 | 2 | HLAS, SMIS, LMIS, LAUTO | -1,60,20,200 | 4,2,2,2 | 75 | 32 | 18 | BEL_HLAS, BEL_SMIS, BEL_LMIS, BEL_LAUTO |
| 2 | HERCULES  | 13 | 390 | 2 | PPC, HLAS, SMIS, -      | -1,-1,40,0   | 2,2,2,0 | 78 | 32 | 20 | HERC_PPC, HERC_HLAS, HERC_SMIS |
| 3 | ACHILLES  | 13 | 270 | 4 | HLAS, SMIS(6), -, -     | -1,30,0,0    | 4,2,0,0 | 90 | 32 | 20 | ACHIL_HLAS, "ACHIL_LAUTO" (slot 1 type is 6 = SMIS per table) |
| 4 | CROSSBOLT | 13 | 150 | 2 | LLAS, LMIS, -, -        | -1,80,0,0    | 2,4,0,0 | 75 | 32 | 18 | CROSS_LLAS, CROSS_LMIS |
| 5 | MARS      | 13 | 150 | 4 | PPC, LAUTO, -, -        | -1,100,0,0   | 1,2,0,0 | 75 | 32 | 18 | MARS_PPC, MARS_LAUTO |
| 6 | DRAGON    | 14 | 90  | 4 | LLAS, HFIRE, -, -       | -1,40,0,0    | 2,1,0,0 | 30 | 16 | 14 | DRAG_LLAS, DRAG_HFIRE |
| 7 | CYCLOPS   | 14 | 90  | 5 | LLAS, LAUTO, -, -       | -1,25,0,0    | 2,1,0,0 | 30 | 16 | 14 | CYC_LLAS, CYC_LAUTO |
| 8 | MERCURY   | 14 | 30  | 6 | LLAS, -, -, -           | -1,0,0,0     | 3,0,0,0 | 10 | 16 | 14 | MERC_LLAS |
| 9 | LAC turret (light fort)  | -1 | 60 | 0 | LAUTO | -1  | 1 | 10 | 32 | 16 | MTURRET |
| 10 | HAC turret (medium fort)| -1 | 60 | 0 | HAUTO | 250 | 1 | 10 | 32 | 16 | MTURRET |
| 11 | NC turret (heavy fort)  | -1 | 90 | 0 | PPC   | -1  | 1 | 75 | 32 | 16 | HTURRET |
| 12 | LRM turret              | -1 | 30 | 0 | LMIS  | -1  | 1 | 10 | 32 | 16 | LTURRET |

Turrets have speed 0: they rotate (NDIR) but never translate. Type 12 turrets fire guided LRMs (DOTUR/DOMISSLE).

Muzzle tables: `shorts[shot][dir 0..7][dx,dy]` (32 bytes per shot). Spawn position of shot i in direction d:
`x = mech.x - size/2 + 8 + tbl[i][d].dx`, `y = mech.y - size/2 + 8 + tbl[i][d].dy` (size = byte[34]).

### 2.3 Direction tables (8 directions, 0 = north, clockwise)
```
dir:        0      1      2     3      4      5       6       7
_DIR     (0,-1) (1,-1) (1,0) (1,1)  (0,1)  (-1,1) (-1,0)  (-1,-1)   unit step, movement = DIR*speed
_FIRESTUF (0,-10)(7,-7)(10,0)(7,7)  (0,10) (-7,7) (-10,0) (-7,-7)   bullet velocity = FIRESTUF*speed/10
_D1 (bytes) (0,-1)(1,-1)(1,0)(1,1)(0,1)(-1,1)(-1,0)(-1,-1)        sign test in GETTD
_D2 (bytes) (0,1) (1,1) (1,0)(1,1)(0,1)(1,1) (1,0) (1,1)          |D1|; corridor test in GETTD
_DIRTAB[3][3] = {{7,0,1},{6,-1,2},{5,4,3}}  indexed [sy+1][sx+1] : (sign dx, sign dy) -> dir
_DIR_CORRECT[8] = {1,-1,-1,1,1,-1,-1,1}       default avoidance turn per facing
_GOALTABLE[4][4] = {{1,1,1,1},{1,2,2,2},{1,2,1,1},{2,1,1,1}}  row ATTGOAL; cols: enemy mechs, structure 0, other structures, turrets
_PRIORITIES[9][4] = {1,0,2,-1},{2,0,3,1},{2,0,1,-1},{1,0,-1,-1},{1,0,-1,-1},{1,0,-1,-1},{1,0,-1,-1},{1,0,-1,-1},{0,-1,-1,-1}
                    weapon-slot preference order per cyborg type (SELECTWEAPON)
_PRIOR[5] = {1,1,1,2,3}  aggression multiplier
```

### 2.4 Tiles
* `_WALL_POWER[180]` tile hit points: tiles 0..59 → 20, 60..119 → 50, 120..129 → 250, 130..139 → 150, 140..149 → 50,
  150..159 → 250, 160..169 → 150, 170..179 → 30.
* `_COL[5][6]` blocking tile ranges per terrain (3 inclusive [lo,hi] pairs; a tile blocks mechs and bullets if inside any):
  arctic {120,120, 61,93, 120,169}; conifer {3,20, 61,93, 119,169}; desert {120,120, 61,93, 120,169};
  volcanic {120,120, 61,93, 120,169}; city {3,59, 61,93, 168,179}. Tile 2 = map border (always blocks, handled separately).
* `_WALLP[5]` → replacement table per terrain: arctic/desert/volcanic → `_WALL_LISTARC`, conifer → `_WALL_LISTCON`,
  city → `_WALLCITY`. Each is `byte newtile[180]` (destroyed tile → what it becomes; 0 = bare ground). Notably
  `_WALL_LISTCON[3..20]` = trees become stumps (21..24 / 1), `_WALL_LISTARC[61..93]` = structures 94..119 (ruins),
  120..149 → 150..179 (damaged wall variants). The new tile's HP is `WALL_POWER[newtile]`, so a wall can be
  destroyed in stages (250 → 150 → 30 → 0-HP ground).
* Tile 53 (0x35) = water (lava in volcanic): doubles cooling (adds 15 heat/cooling tick in volcanic), hides feet sprites.

### 2.5 Animation / sprite tables (`_ANIMATION`, 30 x {short *frames; short n; short 0; short k})
| idx | frames | n | k | use |
|---|---|---|---|---|
| 0..8 | TITAN_BODY, BEL_BODY, HERC_BODY, ACHIL_BODY, CROSS_BODY, MARS_BODY, DRAG_BODY, CYC_BODY, MERC_BODY | 9,2,9,2,2,8,2,2,2 | 2,0,2,0,0,2,0,0,0 | body of type i |
| 9,10,11 | HTURRET_BODY | 4 | 2 | turret body types 9..11 |
| 12 | LTURRET_BODY | 2 | 0 | type 12 |
| 13 | MED_FEET | 11 | 0 | feet (big cyborgs) |
| 14 | LIGHT_FEET | 7 | 0 | feet (small) |
| 15..22 | HLASER, PPC, LIGHT_LASER, EXPLOSION, LIGHT_AUTO, HEAVY_AUTO, MISSILE, LONG_MISSILE | 2,2,2,13,2,2,8,9 | 0,0,0,0,0,0,4,4 | projectile of weapon idx-15 |
| 23 | EXPLOSION | 13 | 0 | small-mech death |
| 24 | MECH_EXPLOSION | 17 | 0 | big-mech death |
| 25,26,27 | DITTY, HLASER_EXPLOSION, EXPLOSION | 5,7,13 | 0 | projectile impact (WEAPONS[8]+25) |

Frame rows are `frames[dir*n + i]`; a 0 or -1 entry means "repeat previous entry" (-1 also terminates).
Frame number → sheet cell (DRAW_SPRITE): **0x400 <= f < 0x800: MECHSET8.GIF 16x16 cell c=f-0x400, sx=(c%20)*16,
sy=(c/20)*16, drawn at (x-8,y-8)**; **1 <= f < 0x400: NEWMARS.GIF 32x32 cell c=f-1, sx=(c%10)*32, sy=(c/10)*32, at
(x-16,y-16)**; **f >= 0x800: NEWMARS2.GIF 32x32 cell c=f-0x800**. Colour 0 is transparent. Examples: TITAN body
dir d = NEWMARS2 cells 4+3d.. (frames 2052..); HERCULES = NEWMARS cells 0..23; DRAGON = MECHSET cells 0..7, CYCLOPS 8..15,
MED_FEET frames 1164.. = MECHSET cells 140.., projectiles 1040..1239 = MECHSET cells 16..215, HUD icons 1243..1253 = cells 219..229.

`DRAW_ANIMATION(x,y,anim,dir,step,alt,counter)`:
```
if anim == -1 return; A = ANIMATION[anim]; row = A.frames + dir*A.n
if !alt: period = (A.k ? A.k : A.n) - 1; i = (counter % period) * step
else:    period = A.n - A.k - 1;          i = A.k + (counter % period) * step
f = row[i]; if f == 0 || f == -1: f = row[i-1]
DRAW_SPRITE(x - viewport.x*32, y - viewport.y*32, f) for each player viewport containing tile (x/32,y/32); mark touched tiles
```
Uses (DRAW_ALL_MECHS / DRAW_BULLETS): feet `(x,y,STATMECH[t].s[0],dir, step=status(0/1), 0, CLOCK())` only if !in_water;
body `(x,y,t,dir,0,0,anim)` normally, or `(x,y,t,dir,1,1,anim)` (recoil frames k..) while reload>0 and weapon slot 0;
dying: types<6 `(x,y,24,0,1,0,anim)` for 17 ticks, else `(x,y,23,0,1,0,anim)` for 13 ticks;
bullets: age<0 `(x,y,25+WEAPONS[w].s[8],0,1,0,n+age)`; 0<=age<4 `(x,y,15+w,dir,1,1,age)` (launch frames); else `(x,y,15+w,dir,1,0,age)`.

### 2.6 `_MSIZE[13][8][4]` (computed by MAKESIZES at start)
For each type t and dir d: draw frame `ANIMATION[t].frames[d*n]` centred in an s x s box (s = 16 or 32, half = s/2),
then: left = (first column in 0..half-1 with any non-zero pixel) - half - 1; top likewise over rows;
right = (last column in s-1..1 with a pixel) - half + 1; bottom likewise. Result: centre-relative bounding box
`{left, top, right, bottom}` used for bullet hit tests (§3.6). For Rust: precompute from the PNG sheets.

---------------------------------------------------------------------------------------------------
## 3. Simulation step — `_DOCLICK` (once per 18.2 Hz tick) — exact order
Inputs per mech from the previous frame: `JOY[m]` (-1 or desired dir 0..7), `KEY[m]` (1 = fire; 2..5 are human
commands consumed by CONTROL). Human-controlled mechs (CFL[m] != -1) get JOY/KEY from CONTROL; all others from
GETCOMP (called inside DOCLICK, see §4).

```
DOCLICK():
  ITIME++
  for m in 0..31: M = MECHS[m]
    if M.redraw > 0: if --M.redraw == 0 && M.nmissiles == 0: SETPLAYER(M.player); DRAWMAP(M.x-72, M.y-69)   // re-centre view on mech
    tile = MAP[(M.y/32)*80 + M.x/32]
    if M.status == -1: goto tail
    GETCOMP(m)                                              // AI → JOY[m], KEY[m] unless human-controlled (§4)
    T = STATMECH[M.type]
    // ---- cooling: every 10 ticks, staggered by index
    if ITIME % 10 == m % 10 && M.status != -2:
        M.heat -= M.hits / T.cooldiv + 1                     // unsigned division
        if M.in_water: if TTYPE == 3: ADDHEAT(m, 15) else M.heat -= M.hits / T.cooldiv + 1
        if M.heat < 0: M.heat = 0
        if M.heat < 75 && M.lockout == -3: M.lockout = 0
    M.anim++
    // ---- bullets hitting this mech
    S = MSIZE[M.type][M.dir]; x0 = M.x+S.left-8; y0 = M.y+S.top-8; x1 = M.x+S.right+8; y1 = M.y+S.bottom+8
    for B in bullet list:
        if B.age < 0: continue                              // already exploding
        if !(x0 < B.x < x1 && y0 < B.y < y1): continue      // strict
        if B.owner == m: continue
        if M.status == -2: continue
        if !DCHECK(B.x, B.y, x0+8, y0+8, x1-8, y1-8, B.dir): continue
        dmg = WEAPONS[B.wtype].damage
        if (unsigned)dmg > (unsigned)M.hits:                // destroyed
            M.status = -2; PLAYBOOM(); M.anim = 0; M.hits = 0
        else:
            M.hits -= dmg; M.target = SELECTT(m); M.avoid_cnt = 0; ADDHEAT(m, WEAPONS[B.wtype].impact_heat)
        B.age = -ANIMATION[25 + WEAPONS[B.wtype].expl].n    // start explosion (5/7/13 ticks)
        B.x += B.vx/2; B.y += B.vy/2
        if B.wtype == 7: CLEARMISSLE(B, 1)
    // ---- reload / movement
    if --M.reload < 0: M.reload = 0
    if M.status < 0: goto tail                              // -2 exploding, -4 retreated (never -1 here)
    if M.lockout == -3: goto tail                           // heat shutdown: no move/steer/fire
    if M.nmissiles:                                         // human "fire again" detonates the LRM salvo
        if (m == CURMECH[0] && FIRE[0]==1 && LFIRE[0]==0) || (m == CURMECH[1] && FIRE[1]==1 && LFIRE[1]==0):
            for i < M.nmissiles: M.missile[i].age = -ANIMATION[25+WEAPONS[M.missile[i].wtype].expl].n
            M.nmissiles = 0
    if JOY[m] == -1: M.status = 0; goto fire               // (skips the in_water / border update below)
    else if M.nmissiles:                                    // steering the LRM salvo instead of the mech
        L = M.missile[0]
        if L.age > 10:
            L.dir = NDIR(L.dir, JOY[m]); sp = WEAPONS[7].speed (15)
            L.vx = FIRESTUF[L.dir].dx*sp/10; L.vy = FIRESTUF[L.dir].dy*sp/10
            tbl = TRANS_TABLES[M.type*4 + M.weapon]; ox,oy = tbl[M.missile_shot[0]][L.dir]
            for i in 1..nmissiles-1: F = M.missile[i]; F.x = L.x - ox + tbl[M.missile_shot[i]][L.dir].dx
                                     F.y = L.y - oy + tbl[..].dy; F.vx=L.vx; F.vy=L.vy; F.dir=L.dir
    else:                                                   // normal movement
        sp = T.speed
        M.want_dir = JOY[m]; M.dir = NDIR(M.dir, JOY[m]); M.status = 1      // turn at most 45° per tick
        if M.want_dir == M.dir:                             // only translate when facing the wanted direction
            M.x += DIR[JOY].dx*sp; M.y += DIR[JOY].dy*sp     // diagonal = sp in both axes (1.41x faster)
            if LCHECK(M.x, M.y, m): M.x -= ..; M.y -= ..; M.status = 0      // terrain collision → undo
        for k in 0..31 while M.status == 1:
            if MECHS[k].status != -1 && != -4 && k != m && TESTC(m, k) == -1: undo move; M.status = 0
    M.in_water = (tile == 53)
    if tile == 2 && RETREAT[m/10]: M.status = -4            // reached the border while retreating → leaves battle
  fire:
    // ---- firing
    if KEY[m] == 1 && M.reload == 0 && M.nbullets < 16:
        FIREWEAPON(m); M.reload = WEAPONS[T.weapon[M.weapon]].reload; M.anim = 0
  tail:
    if M.lockout == -3 && M.status > 0: M.status = 0
  // ---- bullets
  prev = &NULLRECORD
  for B in bullet list:
    if B.age >= 0:
        vx,vy = B.vx,B.vy
        if (B.wtype == 7 && B.age < 10) || (B.wtype == 6 && B.age < 4): B.x += vx/8; B.y += vy/8   // slow launch
        else: B.x += vx; B.y += vy
        B.range -= |vx| + |vy|
    tx,ty = B.x/32, B.y/32; tile = MAP[ty*80+tx]
    B.age++
    if tile == 2 || B.age == -1:                            // off map, or explosion finished
        if B.wtype == 7 && tile == 2: CLEARMISSLE(B, 0)
        DELETEBULLET(prev); continue (prev unchanged)
    if (unsigned)B.range > 0xFA00 && B.age >= 0:           // range went negative → detonate in place
        B.age = -ANIMATION[25+expl].n; if B.wtype == 7: CLEARMISSLE(B, 1)
    if B.age <= 0: prev = B; continue
    if tile in COL[TTYPE] ranges:                           // hit a blocking tile
        idx = ty*80+tx; hp = HITLOC[idx]; dmg = WEAPONS[B.wtype].damage
        if hp <= dmg:
            for k in 0..25: if GS.structure[k].alive && GS.structure[k].x/32 == tx && .y/32 == ty: alive = 0; PLAYCRIT()
            MAP[idx] = WALLP[TTYPE][MAP[idx]]; HITLOC[idx] = WALL_POWER[MAP[idx]]
            mark tile touched in both viewports (redraw)
        else: HITLOC[idx] = hp - dmg
        B.age = -ANIMATION[25+expl].n; if B.wtype == 7: CLEARMISSLE(B, 1)
    prev = B
```

### 3.1 Movement summary
* Speed (px/tick @18.2 Hz): Titan/Behemoth/Hercules/Crossbolt 2, Achilles/Mars/Dragon 4, Cyclops 5, Mercury 6; turrets 0.
* Turning: one 45° step per tick toward JOY (NDIR; a 180° request turns clockwise). No translation on a tick in which
  the mech is not yet facing the wanted direction (except the tick it arrives: NDIR happens first, then the compare).
* Terrain collision (LCHECK): the 4 corners `(x ± h, y ± h)`, `h = STATMECH.coll/2 - 4` (Titan 7, Behemoth 5, Hercules 6,
  small mechs 3), each tested with TESTSHIT: blocked if tile == 2 (border) or tile within COL ranges.
* Mech-mech collision (TESTC): boxes; collide if `|xa-xb| < r && |ya-yb| < r`, `r = (coll_a + coll_b + 10)/2`
  (Titan vs Titan 27, small vs small 19). On collision the move is undone and status = 0; enemies record each other as
  `bump` target (goal = other's position) so the AI shoots point blank.
* Border: tile 2 blocks; if the side is retreating the mech instead gets status -4 (removed from play, drawn no more,
  counts as retreated in UNLOADCRUD).

### 3.2 Heat model (`_ADDHEAT(m, h)`)
```
M.heat += h
if M.heat > 100: M.lockout = -3; M.status = 0          // shutdown until heat < 75 (checked only at cooling ticks)
if M.heat > 150: M.heat = 150; M.status = -2; M.hits = 0; M.anim = 0; PLAYBOOM()   // heat death → explosion
```
Sources: firing `WEAPONS.fire_heat` per shot (whole volley: Titan PPC 2x4 = 8), being hit `impact_heat`
(PPC 10, flamer 8, HLAS 3, missiles 2, LLAS 1, autocannons 0), lava +15 per cooling tick (volcanic only).
Cooling every 10 ticks: `hits / cooldiv + 1` (Titan full health 16, Mercury 4; cooling degrades with damage),
doubled in water (tile 53) outside volcanic maps. AI stops shooting walls above 75 heat; HUD draws heat bar.

### 3.3 Damage model
No hit locations, no criticals, no armour zones: `hits -= damage` (unsigned compare: `damage > hits` → destroyed).
Destroyed: status -2, hits 0, PLAYBOOM, death animation (17 ticks types 0..5, 13 ticks others) then status -1.
A bullet never hits its owner but CAN hit friendly mechs and turrets (no player check). Exploding (-2) mechs are
not hit; retreated (-4) mechs can still be hit (they sit on the border tile) but are not drawn.
Being hit re-targets the AI (`SELECTT`) and aborts obstacle avoidance. `PLAYCRIT` ("critical" sound) is only
played when a structure from the GAMESTUFF list is destroyed.

### 3.4 Hit test geometry (`_DCHECK(bx,by,x0,y0,x1,y1,dir)`)
Box = sprite bounding box MSIZE (pixel-exact per direction). Outer test with ±8 px margin, then by bullet direction:
dir 0/4: `x0 <= bx <= x1`; dir 2/6: `y0 <= by <= y1`; dir 1/5: `x0+y0 <= bx+by <= x1+y1`; dir 3/7: `x0-y1 <= bx-by <= x1-y0`.
(i.e. the bullet's track, projected perpendicular to its motion, crosses the box.)

### 3.5 Projectiles (`_FIREWEAPON(m)`)
```
if M.nmissiles: return                               // can't fire while guiding LRMs
slot = M.weapon (0..4 accepted); w = T.weapon[slot]; half = T.size/2
bx = M.x - half + 8; by = M.y - half + 8; d = M.dir; tbl = TRANS_TABLES[type*4+slot]
fs = |FIRESTUF[d].dx| + |FIRESTUF[d].dy|  (10 straight, 14 diagonal; 0→14)
for i in 0 .. T.shots[slot]-1:
    if M.ammo[slot] == 0: continue
    M.nbullets++
    if w == 7: M.nmissiles = i+1; LFIRE[M.player] = 1
    if M.ammo[slot] != -1: M.ammo[slot]--
    ADDHEAT(m, WEAPONS[w].fire_heat); play sound WEAPONS[w].snd
    ADDBULLET(bx + tbl[i][d].dx, by + tbl[i][d].dy,
              FIRESTUF[d].dx*WEAPONS[w].speed/10, FIRESTUF[d].dy*WEAPONS[w].speed/10,
              d, w, WEAPONS[w].range*fs/10, m)
    M.missile[i] = new bullet; M.missile_shot[i] = i
```
So a volley spawns all shots in the same tick, each at its own muzzle offset. Velocity = speed px/tick along the axis
(0.7*speed per axis on diagonals); range budget = range (straight) or 1.4*range (diagonal) and the per-tick deduction is
|vx|+|vy|, so flight time ≈ range/speed ticks for every direction (HLAS/PPC 44 ticks ≈ 700 px, LLAS 16 ticks, flamer 10,
autocannons 58, SMIS 40, LMIS 213 ticks). SMIS move at 1/8 speed for their first 4 ticks, LMIS for 10 ticks.
On expiry a bullet detonates where it is (explosion animation, no damage); bullets flying onto border tile 2 vanish silently.
LRM salvos: the firer "guides" them — JOY steers the lead missile one 45° step/tick (after age 10); the others are kept in
formation using the muzzle-table offsets; the firer cannot move or fire until the salvo is gone; a fresh fire press
detonates the salvo. `_CLEARMISSLE(B, flag)`: removes B from its owner's missile[] list (shifts down), nmissiles--, and
when it reaches 0 and the owner is a player's current mech: flag==1 → `owner.redraw = -B.age` (re-centre the view
after the explosion), flag==0 → DRAWMAP(owner) immediately.

### 3.6 Destructible terrain
Blocking tiles (COL ranges) absorb bullets. Tile HP map `HITLOC` initialised from WALL_POWER. When HP <= damage the
tile is replaced by WALLP[TTYPE][tile] and gets WALL_POWER[new]; structures in the GAMESTUFF list on that tile are
flagged dead. Mechs never damage tiles by walking. The result map is saved to matrix.dat.

---------------------------------------------------------------------------------------------------
## 4. Control interfaces (other agent covers details; summarised for the sim)
* `_CONTROL`: for p in 0,1: `cur = CURMECH[p]; JOY[cur] = JOYSTICK(J1[keyset])` (index 0..7 of the first held direction
  key, else -1), joystick axes override via `JSREF[3][3] = {7,0,1,6,-1,2,5,4,3}`; `KEY[cur] = BUTTON()`:
  1 fire held, 2 next weapon (BUMPWEAPON: `weapon = next slot with shots>0`, HUD update), 3 next mech (BUMPMECH:
  next index (cur+1)%20.. with status>=0 and same player; old mech's CFL = -1 → AI takes it over), 4 toggle hold
  (MH[p]: -1 auto / 0 / 1), 5 toggle retreat (RETREAT[p]; MH[p]=0). Players with PSTAT[p] == -1 get no input.
  LFIRE/FIRE edge detection for LRM detonation.
* `_GETCOMP(m)` (inside DOCLICK): if CFL[m] != -1 (human) → nothing. Else: if target dead/-structure destroyed →
  `target = SELECTT(m)`; KEY=0; JOY=-1; if redraw>0 → nothing; else if nmissiles → DOMISSLE(m) (steer salvo toward
  target); else if type > 8 → DOTUR(m) else DOMECH(m).
* `_CHECKHOLDS(p)`: hold = MH[p] if != -1, else auto: 1 if any living cyborg of p has an LMIS slot (slots 0..2) with
  ammo != 0, or any living LRM turret (type 12) of p; else 0. Sets `MECHS[p*10+i].hold = hold` for i<10. So AI sides
  sit still and lob LRMs until they run out, then advance.
* `_COMPASS(m)` → direction toward target (RDIR[p] when retreating), via `_GETD(dx,dy,M)` which also stores `M.tdist`.
* `_SETPLAYER(p)`: PLAYER = p; selects the player's half of the screen (ACTION+4 pointer, DESX = 0 or 0xAF).

---------------------------------------------------------------------------------------------------
## 5. Pseudo-C of the in-scope functions (constants exact)

```c
#define NMECH 32
#define TURRET(t) ((t) > 8)
int ITIME;                         // tick counter
short JOY[32], KEY[32], CFL[32];   // CFL: -1 = AI controlled, else human (value = PSTAT[player])
short CURMECH[2], MH[2], RETREAT[2], RDIR[2], LHO[2], PSTAT[2];
int TRANGE, TR, FLAG, OBS[3];

void INITMECHS(void) {
  for (m=0;m<32;m++) BOOTMECH(m);
  PLOTHIT(CURMECH[0],0); PLOTHIT(CURMECH[1],1);
  SETPLAYER(0); SWITCHMECH(CURMECH[0]); SETPLAYER(1); SWITCHMECH(CURMECH[1]);
}
void BOOTMECH(int m) {               // after LOADCRUD filled x,y,type,dir,player,status,hits
  M=&MECHS[m]; for (i=0;i<4;i++) M->ammo[i] = STATMECH[M->type].ammo[i];
  M->weapon=0; M->anim=0; M->hold=1; M->bump=-1; M->nbullets=0;
  if (M->status==0) M->status=1;
}
void SWITCHMECH(int m) {             // make m the current mech of PLAYER
  CURMECH[PLAYER]=m;
  if (M->nmissiles==0) DRAWMAP(M->x-72, M->y-69); else DRAWMAP(M->missile[0]->x-72, M->missile[0]->y-69);
  CFL[m]=PSTAT[PLAYER]; UPWEAPONS(m, PLAYER);
}
int BUMPMECH(int p) {                // next living mech of p; returns -1 if none (=> battle over for p)
  CFL[CURMECH[p]] = -1; start = k = (CURMECH[p]+1)%20;
  do { if (MECHS[k].status>=0 && MECHS[k].player==p) { CURMECH[p]=k; SETPLAYER(p); SWITCHMECH(k); return 0; }
       k=(k+1)%20; } while (k!=start);
  return -1;
}
void BUMPWEAPON(int m,int p) { do M->weapon=(M->weapon+1)%4; while (STATMECH[M->type].shots[M->weapon]==0); UPWEAPONS(m,p); }
void UPWEAP2(int m) { if (m==CURMECH[0]) UPWEAPONS(m,0); if (m==CURMECH[1]) UPWEAPONS(m,1); }   // HUD weapon icons
int  NDIR(int cur,int want) { if (cur==want) return cur; d=want-cur; if (d<-4) d+=8; if (d>4) d-=8; return d<0 ? (cur+7)%8 : (cur+9)%8; }

void ADDHEAT(int m,int h) { M->heat+=h;
  if (M->heat>100) { M->lockout=-3; M->status=0; }
  if (M->heat>150) { M->heat=150; M->status=-2; M->hits=0; M->anim=0; PLAYBOOM(); } }
void PLAYBOOM(void){ if(SOUNDT==1) PLAYS(MUSBIN,15); if(SOUNDT==2) PLAYS(MUSBIN,11); }
void PLAYCRIT(void){ if(SOUNDT==1) PLAYS(MUSBIN,2);  if(SOUNDT==2) PLAYS(MUSBIN,8);  }

int TESTSHIT(int tx,int ty,int m,Mech*M) {        // returns via FLAG
  if (FLAG==1) return; tile=MAP[ty*80+tx];
  if (tile==2) { if (RETREAT[m/10]==0) { FLAG=1; M->avoid_cnt=-1; } else { M->status=-4; return; } }
  else for (i=0;i<6;i+=2) if (COL[TTYPE][i]<=tile && tile<=COL[TTYPE][i+1]) FLAG=1;
  if (FLAG==1 && M->bump==-1 && DESTROY(tile,M)) { M->goal_x=tx*32+16; M->goal_y=ty*32+16; M->bump=(int8)(-tile); }
}
int DESTROY(int tile,Mech*M) {                    // should the AI shoot this wall?
  if (M->target<0 && M->target>-40) return 1;      // heading for a structure: yes
  if (tile==2) return 0;
  if (TTYPE==1 && tile<60) return 1;              // conifer: trees
  if (TTYPE==4 && M->player==0 && ATTGOAL==0) return 1;   // city assault with goal 0
  return 0;
}
int LCHECK(int x,int y,int m) { FLAG=0; h=STATMECH[M->type].coll/2-4;
  TESTSHIT((x-h)/32,(y-h)/32,m,M); TESTSHIT((x-h)/32,(y+h)/32,m,M); TESTSHIT((x+h)/32,(y-h)/32,m,M); TESTSHIT((x+h)/32,(y+h)/32,m,M);
  return FLAG; }
int DCHECK(int bx,int by,int x0,int y0,int x1,int y1,int dir) {
  switch(dir){ case 0:case 4: return x0<=bx&&bx<=x1; case 2:case 6: return y0<=by&&by<=y1;
    case 1:case 5: return x0+y0<=bx+by && bx+by<=x1+y1; case 3:case 7: return x0-y1<=bx-by && bx-by<=x1-y0; } return 1; }
int TESTC(int a,int b) { A=&MECHS[a]; B=&MECHS[b]; r=(coll(A)+coll(B)+10)/2;
  if (B->x-r<A->x && A->x<B->x+r && B->y-r<A->y && A->y<B->y+r) {
    if (A->player!=B->player) { if (A->bump==-1){A->bump=b;A->goal=B->pos;} if (B->bump==-1){B->bump=a;B->goal=A->pos;} }
    return -1; }
  if (B->bump==a) B->bump=-1; if (A->bump==b) A->bump=-1; return 0; }
int TESTPOSITION(int x,int y,int m) {            // 1 if (x,y) is blocked for mech m
  if (LCHECK(x,y,m)) return 1; base=coll(M)+10;
  for (k=0;k<32;k++) { r=(coll(K)+base)/2;
    if (K->x-r<x && x<K->x+r && K->y-r<y && y<K->y+r && k!=m) { if (M->player==K->player) M->crowd++; return 1; } }
  return 0; }
void GETCOLLISIONMATRIX(int m) { sp=STATMECH[M->type].speed;
  for (s=0;s<3;s+=2) { d=(M->dir+s+7)%8; OBS[s]=TESTPOSITION(M->x+DIR[d].x*sp, M->y+DIR[d].y*sp, m); } }  // OBS[0]=left, OBS[2]=right
void CHECKOBSTICAL(int m) {
  if (++M->avoid_cnt==1) { (dx,dy)=target_pos(M)-M->pos (structure: +16); dx=|dx|; dy=|dy|;
     t=DIR_CORRECT[M->dir]; if (dy<dx) t=-t; M->avoid_turn=t; }
  M->avoid_dir=(M->dir+M->avoid_turn+8)&7; }
```

```c
void DOMECH(int m) {                 // AI for cyborgs (JOY[m], KEY[m] out)
  if ((ITIME&15)==1) M->target=SELECTT(m);
  KEY[m]=0;
  if (M->target==-40) { JOY[m]=-1; return; }
  cdir=COMPASS(m);
  if (M->hold==1) {                  // turret mode
    w=SELECTWEAPON(m); if (M->weapon!=w && w!=-1) { M->weapon=w; UPWEAP2(m); }
    if (w!=-1 && wtype(M,w)==7) KEY[m]=1;                     // LRMs: just fire (guided)
    range=WEAPONS[wtype(M,M->weapon)].range;
    td1=GETTD(m,range,M->player); r1=TRANGE;                  // nearest friendly in a firing line
    td2=GETTD(m,range,1-M->player); if (td2==-1) return;      // nearest enemy in a firing line
    if (r1<=TRANGE && td1==td2) return;                        // friendly would be hit first
    if (M->dir!=td2) JOY[m]=td2; else KEY[m]=1; return;
  }
  if (M->bump!=-1) {                 // collided with / targeting something concrete
    w = (0<=M->bump && M->bump<=32) ? SELECTWEAPON(m) : 0;    // walls: always slot 0
    M->avoid_cnt=0; if (M->weapon!=w) { M->weapon=w; UPWEAP2(m); }
    JOY[m]=GETD(M->goal_x-M->x, M->goal_y-M->y, M); if (JOY[m]==M->dir) KEY[m]=1;
    if (M->bump<0 || M->bump>33) {   // wall target
      tile=MAP[(M->goal_y/32)*80+M->goal_x/32]; if (((tile+M->bump)&0xFF)!=0) M->bump=-1;   // tile changed → forget
      if (M->heat>75) KEY[m]=0; return; }
    if (MECHS[M->bump].status!=-1) return; M->bump=-1; return;
  }
  w=SELECTWEAPON(m); if (M->weapon!=w && w!=-1) { M->weapon=w; UPWEAP2(m); }
  range=WEAPONS[wtype(M,M->weapon)].range;
  td1=GETTD2(m,range,M->player,M->dir); r1=TRANGE;            // friendly straight ahead?
  td2=GETTD2(m,range,1-M->player,M->dir);                       // target straight ahead?
  if (td2!=-1 && (r1>TRANGE || td1!=td2)) KEY[m]=1;
  if (M->status==0 && M->avoid_cnt==0) { CHECKOBSTICAL(m); JOY[m]=M->avoid_dir; return; }   // just got blocked
  if (M->avoid_cnt>0 && M->avoid_cnt!=32 && M->crowd<=32) {   // walking around an obstacle
    GETCOLLISIONMATRIX(m);
    if (OBS[1-M->avoid_turn]==0) { M->avoid_dir=(M->avoid_dir-M->avoid_turn+8)&7; M->avoid_cnt--; }   // side toward goal free → veer back
    else if (M->status==0) CHECKOBSTICAL(m);
    JOY[m]=M->avoid_dir; return;
  }
  M->avoid_cnt=0; M->crowd=0; JOY[m]=cdir;                    // head straight for the target
}
void DOTUR(int m) {                  // AI for turrets
  if (M->type==12) { M->target=SELECTT(m); if (M->target!=-40) KEY[m]=1; return; }
  range=WEAPONS[wtype(M,0)].range;
  td1=GETTD(m,range,M->player); r1=TRANGE; JOY[m]=GETTD(m,range,1-M->player);
  if (JOY[m]==-1) return; if (r1<=TRANGE && JOY[m]==td1) return; KEY[m]=1;
}
void DOMISSLE(int m) {               // steer salvo toward target: JOY = sign vector → DIRTAB
  L=M->missile[0]; (tx,ty)=target_pos(M) (structure +16); dx=tx-L->x; dy=ty-L->y;
  sx = dx>7?1 : dx<-7?-1 : 0; sy likewise; JOY[m]=DIRTAB[sy+1][sx+1];   // -1 when within ±7 → missiles fly straight
}
int SELECTWEAPON(int m) {            // first slot in PRIORITIES order with ammo and range > distance to target
  for (i=0;i<4;i++) { s=PRIORITIES[M->type][i]; if (s==-1) break;
    if (M->ammo[s]!=0 && (unsigned)WEAPONS[wtype(M,s)].range > (unsigned)M->tdist) return s; }
  return -1; }
int SELECTT(int m) {                 // nearest (weighted Manhattan) enemy; -(k+1) structure; -40 none
  bestS=32000; bestM=32000;
  if (M->player==0) for (k=0;k<26;k++) if (GS.structure[k].alive) {
      d=|sx+16-M->x|+|sy+16-M->y|; d/= (k==0?GOALTABLE[ATTGOAL][1]:GOALTABLE[ATTGOAL][2])*ATTAGG;
      if (d<bestS) { bestS=d; sidx=-k; } }
  for (k=0;k<32;k++) if (MECHS[k].status>=0 && MECHS[k].player!=M->player) {
      d=|dx|+|dy|; d/= (k<20?GOALTABLE[ATTGOAL][0]:GOALTABLE[ATTGOAL][3])*ATTAGG;
      if (d<bestM) { bestM=d; midx=k; } }
  if (bestM==32000) return -40;
  if (bestS>bestM) { TR=bestM; return midx; } TR=bestS; return sidx-1;
}
int GETTD(int m,int range,int tplayer) {   // best firing direction at any mech of tplayer within range; TRANGE=dist
  best=32000; bd=-1;
  for (k=0;k<32;k++) if (MECHS[k].status>=0 && k!=m && MECHS[k].player==tplayer) {
    dx=K->x-M->x; dy=K->y-M->y; ax=|dx|; ay=|dy|;
    for (s=0;s<8;s++) { if (s==RDIR[m/10] || (s+1)%8==RDIR[m/10] || (s+7)%8==RDIR[m/10]) continue;   // never shoot "backwards" (retreat dir ±1)
      if (ax+ay>=range || ax+ay>=best) continue;
      if (|D2[s].y*ax - D2[s].x*ay| >= 32) continue;          // within a 32-px corridor along s
      if (D1[s].x*dx<0 || D1[s].y*dy<0) continue;              // correct half-plane
      best=ax+ay; bd=s; } }
  TRANGE=best; return bd; }
int GETTD2(int m,int range,int tplayer,int dir) {   // same test for one given direction; if tplayer is the enemy only M->target is tested
  (tplayer!=M->player) ? single k=M->target (may be a structure, uses x+16,y+16) : loop over all mechs of tplayer; corridor 48 px
  returns dir or -1; TRANGE=best; }
```

```c
void LOADCRUD(void) {   // GAMESTUFF -> globals (see §1.3)
  TTYPE=GS.s16[288]; PSTAT[0]=GS.i8[290]; PSTAT[1]=GS.i8[291]; SOUNDT=GS.i8[467];
  CURMECH[0]=GS.s16[463]; CURMECH[1]=GS.s16[465]+10; HUMPLAYER=GS.s16[484];
  JOYAX=GS.s16[472]; JOYAY=GS.s16[474]; JOYBX=GS.s16[480]; JOYBY=GS.s16[482];
  ATTGOAL=GS.s16[459]; ATTAGG=PRIOR[GS.s16[457]];
  for (i=0;i<32;i++) { r=&GS.mech[i]; M->x=r->x; M->y=r->y; M->type=r->type; M->player=r->player; M->dir=r->dir;
     if (r->type==-1) { M->status=-1; M->hits=1; } else { M->status=0; M->hits=(long)STATMECH[M->type].armor*r->health/100; } }
  RDIR[0]=(MECHS[0].dir+4)%8; RDIR[1]=(MECHS[10].dir+4)%8; PC1=GS.s16[451]; PC2=GS.s16[453];
}
void UNLOADCRUD(void) {
  GS.s16[288]=TTYPE; GS.i8[290]=PSTAT[0]; GS.i8[291]=PSTAT[1]; a=d=0; ARET=DRET=0;
  for (i=0;i<32;i++) { if (M->status>=0) { if (M->player==0) a=1; else d=1; }
     if (M->status==-4) { if (M->player==1) DRET=1; else ARET=1; }
     r->x=M->x; r->y=M->y; r->type=M->type; r->player=M->player; r->dir=M->dir;
     if (M->status==-1||M->status==-2) r->health = (M->hits==0) ? 101 : 0;
     else r->health = (long)M->hits*100/STATMECH[M->type].armor; }
  GS.i8[292] = (!a && !d) ? 0 : (a ? 2 : 1); GS.i8[293]=ARET; GS.i8[294]=DRET;
}
void INITMAP(void) { LOAD_FILE("matrix.dat", MAPLOC); for (i=0;i<6240;i++) HITLOC[i]=WALL_POWER[MAPLOC[i]]; }
void SETMAP(void) {     // scroll PLAYER's 6x6-tile viewport (MAPINFO[p].x,y = tile origin) so the current mech (or its lead LRM) stays inside
  M=&MECHS[CURMECH[PLAYER]]; (fx,fy) = (M->x-72, M->y-69) or lead missile (x-72,y-69) if nmissiles>0 || redraw>0;
  rx=fx-MAPINFO.x*32; ry=fy-MAPINFO.y*32;
  loop { if (rx>=48) { LEFTSCROLL; MAPINFO.x++; rx-=32; redraw right column; } if (rx<0) { RIGHTSCROLL; MAPINFO.x--; rx+=32; redraw left column; }
         if (ry>=54) { UPSCROLL; MAPINFO.y++; ry-=32; redraw bottom row; } if (ry<0) { DOWNSCROLL; MAPINFO.y--; ry+=32; redraw top row; } until no scroll }
}
void DRAWMAP(int px,int py) { MAPINFO[PLAYER].x=clamp(px/32-1,0,74); .y=clamp(py/32-1,0,72); mark all 36 tiles touched; REDRAW_TOUCHED(); }
void TOUCHED(int p,int cell) { if (0<=cell<36) TOUCH[p][cell]=1; }
void SPSET(int x,int y,int c) { radar pixel plot with undo stack (STACK[600] of {ptr,colour}); "Stack overflow" if >600 }
```

---------------------------------------------------------------------------------------------------
## 6. Open questions
1. `_STATMECH.s[16]` (5,4,3,3,2,2,1,1,1,1,1,1,1) and the 32 bytes at GAMESTUFF+256..287 are never read by ACTION —
   presumably strategy-layer data (class / factory size?). `_WEAPON_RANGE` is dead data.
2. Achilles slot 1 is weapon type 6 (SMIS, 30 rounds, 2-shot volleys) in STATMECH although its muzzle table is named
   `_ACHIL_LAUTO`; `_BEL_LAUTO` likewise has zero rows 3..4. Check against CYBOR's `_CYBWEAP` / manual which is right.
3. Who fills the 26-entry structure list (GAMESTUFF+295) — CYBOR's `_WRITE_MATRIX`? Which entry is index 0 (capital /
   factory?) — the AI weights it with GOALTABLE column 1. Needs CYBOR-side confirmation.
4. `pinfo.iat` `_P[0]` is zeroed during the battle and restored at exit — probably a "battle in progress / crashed"
   marker read by CYBOR on restart; semantics of the remaining 90 bytes unknown (not used by ACTION).
5. `GAMESTUFF.result` is 2 whenever the attacker has a survivor, even if the defender also has survivors (the loop only
   exits when one side's current mech cannot be replaced, so both-alive should not happen except after -4 retreats of
   a whole side — the retreated side has status -4 < 0 and counts as "no survivor" while ARET/DRET flag the retreat).
6. BIOS INT 8 chaining: the ISR re-enters the original handler every 0x5D interrupts when flag cs:0x56D is set (set by
   a sound routine), else every 256 — only matters for DOS clock drift, not the sim.
7. `_HUMPLAYER` / key-set mapping in CONTROL (both players read J1[1] when HUMPLAYER==1 and no joystick) — verify with INTRO's option screen.
8. MSIZE depends on sprite pixel content; the Rust port must compute it from MECHSET8/NEWMARS/NEWMARS2 exactly as §2.6
   (colour index 0 = transparent) or hit boxes will differ.
