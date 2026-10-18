# CYBOR.PRG — MOVE / REPAIR / SPY operations and end-of-turn battle resolution

Source: cybor.asm (see CONTEXT.md for file layout). All offsets below are relative to `_MECHRECORD`
unless written as `cty+N` (country record, base = c*236) or `slot+N` (cyborg slot, base = c*236+234+10*i).
Compact annotated dumps of every function analysed here are in `re/dump/mb_*.txt`.
Floating-point code is Turbo C emulator-encoded (`int 34h..3Dh`); every FP sequence was decoded from the raw
bytes (and the two decisive ones verified against the bytes of orig/.../CYBOR.PRG).

## 0. Data structures used by this subsystem

### Cyborg slot (10 per country, 5 shorts each, at cty+234+10*i)
| off | meaning |
|---|---|
| +0 | type 0..8 (TITAN..MERCURY), **10 = empty** |
| +2 | status 0..100 (health %) |
| +4 | **destination country**. == own country number when the unit is not moving. Set by the move box; an attack is simply a destination owned by someone else. |
| +6, +8 | never referenced anywhere in CYBOR.PRG (always 0). |

A unit is "present" in a country when `type != 10`; it is "leaving" when `dest != c`. The per-country count
`cty+228` is a UI counter only (it is maintained by `_MOVE_UNIT` for the move box and never consulted by the
battle code).

### Country fields referenced here
| off | meaning |
|---|---|
| +172 | owner 0..4, 5 = neutral |
| +192..+204 | 7 "build space" slots (used count = `cty+224`): 0 empty, 1 government/capital, 2 fortification (level in +226), 3/4/6 factory (uni/dual/quadro; value = bays+2), 7 factory enhancement. `_GET_AMOUNT_OF_SPACE` counts `3<=v<=6` as a factory, `v==7` as an upgrade. |
| +206..+222 | adjacency list, up to 9 country numbers, 0-terminated |
| +224 | build space (number of build slots) |
| +226 | fortification level 0..3 |
| +228 | cyborg counter (UI) |
| +334 | 24 factory bay shorts (10 = empty) |
| +384 | **city damage %** (0..100, only meaningful for terrain 5) |
| +386 | value ("networth", displayed ×1000) |
| +388 | total number of factory bays ("factory potential") |
| +390 | terrain 1 arctic, 2 coniferous, 3 desert, 4 volcanic, 5 city |
| +392 | **attack flag**: owner id of the player who attacked this country this turn, 5 = none (reset by `_CLEAR_ATTACK_FLAG`). Used by the map drawing (battle icon) and HAL. |

Player vault = `long` at player record +20 (`_MECHRECORD+32+32*p`).

### Battle bookkeeping (DGROUP)
```
_ALIST  @CE57  600 bytes = 100 records of 3 shorts {src_cty, tgt_cty, done}
                 one record per (source country, target country) attack pair.
                 A finished record is {0,0,1}; the list ends at a record {0,0,0}.
_CLIST  @D0AF  72 shorts: the distinct *source* countries that attack this turn; _CMAX = index of last entry (-1 = empty)
_TLIST  @D13F  11 shorts: scratch list of distinct targets of one source (0-terminated), cleared by _CLEAR_TLIST
_ACTIONLIST @D27B 80 bytes = 2 arrays of 20 shorts:
                 [0..9]   type of attacker i (10 = none)      [10..19] type of defender i
                 +40: [0..9] hits (tons of armour left) of attacker i, [10..19] of defender i  (-1 = none)
_ACTIONTEMP @D24D 20 shorts: copy of the 20 types made by _SORT_ACTIONLIST; it survives the battle
                 (ACTIONLIST types are set to 10 when a unit dies) and is the type used when placing survivors.
_ATTACKCTY @D277, _DEFENDCTY @D279 : the two countries of the current battle
_VICTOR @803E : -1 none/both destroyed, 1 defender won, 2 attacker won
_ARET @30AA, _DRET @30AB (bytes): attacker / defender retreated (only ever set by ACTION.PRG; the simulation leaves them 0)
_SOUNDS[5] @D1F5 : per-player counter of countries won (+1) / lost (-1) this turn (drives the end-of-turn fanfare/dirge)
_ANAME/_DNAME : player names of attacker/defender for the summary
```

### Stat tables
```
_ST[9][8]  (per cyborg type)        TITAN BEHEM HERC ACHIL CROSS MARS DRAG CYCL MERC
  [0] long-range firepower             8     8     0     0    16    0    0    0    0
  [1] short-range firepower           18    24    19    18     2    8    1    4    1
  [2] range                           80    68    75    69    68   60   23   28   28
  [3] heat                            15    23    23    21    10    7   21    3    3
  [4] armour (== _HL)                750   600   390   270   150  150   90   90   30
  [5] speed                            0     0     0    10    10   10   20   20   20
  [6] (unused by the simulation)      10     6     4     3     3    3    2    2    2
  [7] (unused by the simulation)       6     5     7     4     6    2    3    1    2
_HL[9]  = 750 600 390 270 150 150 90 90 30            full armour ("hits") of each type
_CD[9]  = 7.5 6.0 3.9 2.7 1.5 1.5 0.9 0.9 0.3 (float) = _HL/100 : hits per status-percent
_REPFACTOR[11] (float) = .60 .55 .50 .45 .40 .35 .30 .20 .10 .05 0   indexed by status/10
_UNITCOST[9] (long) = 100000 90000 40000 30000 70000 15000 10000 5000 2500
_HEATFACTOR @1679 (short, set by _SET_TERRAIN_FACTOR): 1 arctic, 2 coniferous/city, 3 desert/volcanic
_FORTFACTOR @27A2: shorts {0,3,6,9} + doubles 1.5, 1.3, 10.0, 1.0 — NOT used by the simulation
            (the simulation uses the literal doubles 1.5 / 2.0 / 4.0 at _CD+96/+104/+112).
_TERRAINVALUE @CC8C, _KILLTONS @851F, _TONNAGE @7DF5: _TONNAGE is the spy/"glimpse" tonnage sum
            (see §3); _TERRAINVALUE and _KILLTONS are not referenced by any function of this scope.
```

---------------------------------------------------------------------------------------------------

## 1. Movement

### 1.1 UI flow
`_MOV_REP_SPY_OP` is the common driver for the Move (`_OPCONTROL==2`), Spy (`==3`) and Repair (else) icons.
It loops while `_KEEPOP`: a click with y < 142 on the map prints the option banner (`_MOVE_MESSAGE` /
`_SPY_MESSAGE` / `_REPAIR_MESSAGE`), reads the click (`_GET_COORDS`) and calls `_VALIDATOR`; a click with
y >= 142 is passed to `_CHECK_SCROLL` (map scrolling). On exit it un-highlights the icon and calls `_SHOW_STATUS`.

```c
void VALIDATOR() {                       // _BUTTONCONTROL: 2 move, 3 spy, 4 repair
    COUNTRY1 = COUNTRY2 = 0; BUILDCHOICE = 0;
    GET_COUNTRY_NUM();  if (CTRYNUM == 0) return;            // click not on a country
    COUNTRY1 = CTRYNUM;  CHECK_OWNERSHIP();                  // OWNER = (owner(COUNTRY1) == CURPLAY)
    switch (BUTTONCONTROL) {
    case 2: if (!OWNER) { DISALLOW_OP(); return; }           // "YOU DO NOT OCCUPY THIS COUNTRY"
            ALLOW_MOVE();                                    // "SELECT A DESTINATION COUNTRY."
            GET_COORDS(); GET_COUNTRY_NUM(); if (CTRYNUM == 0) return;
            COUNTRY2 = CTRYNUM;  CHECK_ADJACENT();
            if (ADJFLAG) DO_MOVE_BOX(); else MESSAGE2();     // "YOU CAN ONLY MOVE TO ADJACENT COUNTRIES."
            break;
    case 3: if (OWNER) DO_COUNTRY_INFO(); else DO_SPY_INFO(); break;
    case 4: if (OWNER) ALLOW_REPAIR(); else DISALLOW_OP(); break;
    }
}
int CHECK_ADJACENT() { ADJFLAG = 0; for (i=0;i<9;i++) if (adj(COUNTRY1)[i] == COUNTRY2) ADJFLAG = 1; }
```
Note: nothing prevents COUNTRY2 == COUNTRY1 except that a country is never in its own adjacency list.
Moving into a country the player owns, a neutral country or an enemy country are all allowed; only
adjacency and ownership of the *source* are checked.

### 1.2 The move box (`_DO_MOVE_BOX`, `_SET_MOVE_BOX`, `_UNIT_FILL`, `_SHOW_UNITS`, `_GET_SELECTION`, `_MOVE_UNIT`)
* `_SET_MOVE_BOX`: draws "CLICK ON BATTLE UNIT NAME TO MOVE", `SOURCE: <name> <count>`, `DESTIN: <name>`
  and, if destination has the same owner, its count (and "MAX" if it is already 10). Column headers
  SOURCE (x=0x60) / STATUS (x=0x8E) / DESTIN (x=0xB4).
* `_UNIT_FILL(x=0x60, y=0x46, dx=0x36)`: lists the 10 slots of COUNTRY1 whose `dest == COUNTRY1` and
  `type != 10`: name at (x, y+6*n), status at x+dx (red if < 100). Each listed unit gets a button
  rectangle via `_SET_BUT_COORDS(i, x, y)` (`_BOXXCOORD[i]`, `_BOXYCOORD[i]`; cleared to -1 by
  `_CLEAR_BUT_COORDS`). In repair mode (`BUTTONCONTROL==4`) units with status >= 100 get no button.
  In move mode it then also lists, for every adjacent country owned by CURPLAY, the units of that country
  whose `dest == COUNTRY1` (incoming units) with the tag "TRN" at x=0xB4 — informational only, no button.
* `_SHOW_UNITS`: event loop: a click inside the OK button (x 0xBF..0xD0, y 0x1A..0x2D) ends the box; any
  other click → `_GET_SELECTION` → `_CHECK_NAME_FIELDS` (hit test: x in [BOXX, BOXX+40], y in
  [BOXY, BOXY+5]) → `_MOVE_UNIT(i, hit)` (or `_REPAIR_UNIT` when `BUTTONCONTROL != 2`).

```c
void CHECK_NUM_UNITS(int *ok) {          // ok = "a unit may be plotted into COUNTRY2"
    *ok = 1;
    if (owner(COUNTRY1) == owner(COUNTRY2) && cty[COUNTRY2].count >= 10) *ok = 0;   // 10-unit cap, friendly moves only
}
void MOVE_UNIT(int i, int hit) {
    if (!hit) return;
    CHECK_NUM_UNITS(&ok);
    if (!ok && BOXXCOORD[i] < 0xB4) return;              // unit still in SOURCE column and target full
    if (BOXXCOORD[i] <= 0x88) {                           // --- plot the move: SOURCE -> DESTIN column
        redraw name at x=0xB4; BOXXCOORD[i] = 0xB4;
        slot(COUNTRY1,i).dest = COUNTRY2;
        if (owner(COUNTRY1) == owner(COUNTRY2)) { cty[COUNTRY2].count++; redraw; if (count==10) print "MAX"; }
        cty[COUNTRY1].count--; redraw;
    } else if (BOXXCOORD[i] >= 0xB4) {                    // --- take the move back
        if (owner(COUNTRY1) == owner(COUNTRY2)) { cty[COUNTRY2].count--; redraw; erase "MAX"; }
        cty[COUNTRY1].count++; redraw;
        slot(COUNTRY1,i).dest = COUNTRY1;
        redraw name at x=0x60; BOXXCOORD[i] = 0x60;
    }
}
```
Rules that follow:
* Only the owner of the source may plot; source and destination must be adjacent.
* **Max 10 units per country** is only enforced for moves into a country you already own (the counter
  `+228` of the destination is incremented immediately when plotting). Attacks are not limited (the units
  are not counted in the destination).
* A plotted move is reversible only while the box is open for that source/destination pair (the unit is
  no longer listed by `_UNIT_FILL` afterwards because its `dest != COUNTRY1` — "once plotted you can no
  longer manipulate them"). Units in transit cannot be repaired or re-plotted.
* Moving into a neutral or enemy country is an **attack**: the only state is `slot.dest = enemy country`.
  Nothing else (no attack flag) is written at plot time; `cty+392` is set during resolution.

---------------------------------------------------------------------------------------------------

## 1.3 End of turn: `_SET_ACTION_PHASE` (movement + battle list)
Called once after the last player's turn. State for the ACTION.PRG round trip is kept in `_PZ`
(written with `_ALIST` to `pass2.dat`; `_PASSFLAG != 0` means "we are being re-entered after ACTION.PRG",
and execution resumes at the "returned from action" label with all locals restored from `_PZ+44..`).

```c
void SET_ACTION_PHASE() {
    int pass=0, aidx=0, bcty, movedFlag=1, battleFlag=1, foughtInAction=0, simulatedWithSummary=0;
    if (PASSFLAG) goto returned_from_action;
    FADEFLAG=0; VICTOR=-1; QUICKL=0;
    CLEAR_ATTACK_FLAG();            // cty[c]+392 = 5 for c=1..72
    CLEAR_SOUNDS(); SOUNDNOTPLAYED=1; LASTROWY[0..4]=TESTROWY[0..4];
    while (movedFlag || battleFlag) {
        movedFlag = battleFlag = 0;
        // ---- (a) peaceful moves, 8 passes so that chains of moves settle
        for (pass=0; pass<8; pass++)
          for (c=1; c<=72; c++) if (owner(c) != 5)
            for (i=0; i<10; i++) if (slot(c,i).dest != c) {
                d = slot(c,i).dest;
                if (owner(d)==owner(c) || owner(d)==5)          // friendly move, or move into a neutral country
                    if (FRIENDLY_COUNTRY(d)) UNIT_MOVES(c, i, &movedFlag);
            }
        // ---- (b) build the attack list
        LOAD_ALIST();
        // ---- (c) resolve battles in random order of attacking country
        while (CLIST[0] != 0) {
            GET_BATTLE(&bcty);                     // bcty = CLIST[rand(0, CMAX)]
            DRET = ARET = 0;
            FIND_IN_ALIST(&aidx, bcty);            // first live ALIST record with src == bcty
            if (BATTLES_CONFIRMED(aidx) && owner(ALIST[aidx].src) != owner(ALIST[aidx].tgt)) {
                LOAD_ATTACK(src, tgt); LOAD_DEFEND(tgt);
                SORT_ACTIONLIST(0,10); SORT_ACTIONLIST(10,20);
                ANAME = name(owner(ATTACKCTY)); DNAME = name(owner(DEFENDCTY));
                if (SELECTION == 2) {                              // "Strategy Campaign": everything simulated
                    if (SOUNDNOTPLAYED) { PLAY_SIM_SOUND(); SOUNDNOTPLAYED = 0; }
                    BATTLE_SIMULATION();                           // no summary screen
                } else if (ATTCONFIRMED && DEFCONFIRMED) {         // always true here
                    SET_PLAYER_TYPE();                             // CHOICE 0 H/H, 1 H att vs C def, 2 C att vs H def, 3 C/C
                    if (CHOICE == 3) BATTLE_SIMULATION();          // computer vs computer: silent simulation
                    else {
                        if (!FADEFLAG) { FADE_OUT(); FADEFLAG=1; }
                        ACTION_SETUP();                            // attack/defence setup screens (other agent)
                        if (SIMULATE) {                            // SIM button pressed
                            PLAY_SIM_SOUND(); BATTLE_SIMULATION(); M_ON(); BATTLE_SUMMARY(); simulatedWithSummary=1;
                        } else {
                            INSTRAT=0; SET_TERRAIN_LOAD(); save locals+CPCOUNT1..3+FADEFLAG+LTERRAIN+SELECTION+DELAYTIME into _PZ;
                            WRITE_FILE2("pass2.dat", &_ALIST, 0x483); LOADSTRAT=0; START();   // exec ACTION.PRG
                          returned_from_action:
                            LOAD_FILE2("pass2.dat", &_ALIST, 0x483); START2(); restore locals from _PZ;
                            if (LTERRAIN == 5) {                   // city: derive city damage from destroyed map pieces
                                COUNT_CTY_PIECES(&CPCOUNT2);       // tiles 0x20..0x3B inside the 74x72 playfield
                                CPCOUNT2 = CPCOUNT3 - 3*(CPCOUNT3 - CPCOUNT2);
                                OLDCITYDAM = cty[DEFENDCTY]+384;
                                if (CPCOUNT1 != CPCOUNT2) {
                                    cty[DEFENDCTY]+384 = (int)(100.0 - (double)CPCOUNT2 / CPCOUNT1 * 100.0);
                                    if (cty[DEFENDCTY]+384 > 100) cty[DEFENDCTY]+384 = 100;
                                }
                            }
                            QUICKL=1; if (JOYSTICKCONTROL) NEW_MOUSE();
                            CHECK_PIECES();                        // structure losses from destroyed pieces (§4.9)
                            BATTLE_SUMMARY(); INSTRAT=1; LOADSTRAT=1; foughtInAction=1;
                        }
                    }
                }
                // ---- (d) apply the outcome
                if (VICTOR == 2) {                                 // attacker won
                    if (DRET) DEFENDER_RETREAT();
                    PLACE_VICTOR(1);
                    SOUNDS[owner(DEFENDCTY)]--; SOUNDS[owner(ATTACKCTY)]++;
                    owner(DEFENDCTY) = owner(ATTACKCTY);           // *** capture ***
                    cty[DEFENDCTY]+392 = owner(ATTACKCTY);
                } else if (VICTOR == 1) {                          // defender won
                    if (ARET) ATTACKER_RETREAT();
                    cty[DEFENDCTY]+392 = owner(ATTACKCTY);
                    SOUNDS[owner(ATTACKCTY)]--;
                    PLACE_VICTOR(0);
                } else {                                           // -1: both sides destroyed / aborted
                    if (ARET) ATTACKER_RETREAT();
                    if (DRET) DEFENDER_RETREAT();
                    RESET_DESTIN();
                }
            }
            ALIST[aidx] = {0, 0, 1};                               // mark done
            UPDATE_CLIST(bcty); battleFlag = 1;
        }
        pass = 1; aidx = 0; CLEAR_ALIST();
    }
    RETURN_STRANDED();              // every slot of every country: dest = own country
    SET_OWNERS();                   // countries without government AND without units become neutral and are razed
    if (SELECTION == 1 && VICTOR != -1) { INSTRAT=0; M_OFF(); LOADMAP2(); FADE_IN(); }
    else if (simulatedWithSummary && VICTOR != -1) RESET_STRATEGY();
    ... music-track bookkeeping ([0xCAE5]/[0xCADB], other agent) ...
    CHECK_PLAYERS(); M_ON(); if (FADEFLAG) FADE_IN();
}
```

### 1.4 Helpers of the movement phase
```c
// A country is "friendly to enter" when none of its units is leaving it towards a country of another owner.
int FRIENDLY_COUNTRY(int d) {
    for (i=0;i<10;i++) { t = slot(d,i).dest;
        if (t != d && owner(t) != owner(d)) return 0; }
    return 1;
}
// Executes one plotted peaceful move. NOTE: also used for moving into a neutral country -> capture without a fight.
void UNIT_MOVES(int c, int i, int *flag) {
    d = slot(c,i).dest;
    owner(d) = owner(c);                                 // (no-op for friendly, capture for neutral)
    for (k=0;k<10;k++) if (slot(d,k).type == 10) {       // first free slot of destination
        slot(d,k).type   = slot(c,i).type;
        slot(d,k).dest   = d;                            // (copied from slot(c,i).dest == d)
        slot(d,k).status = slot(c,i).status;
        slot(c,i) = {10, 100, c};                        // source slot emptied
        *flag = 1; break;
    }
    // no free slot: nothing happens, the unit keeps dest=d ("stranded", see RETURN_STRANDED)
}
// Attack list: every (source, target) pair with different owners.
void LOAD_ALIST() {
    n=0; CMAX=-1; cm=0;
    for (c=1;c<=72;c++) {
        tl=0; any=0; CLEAR_TLIST();
        for (i=0;i<10;i++) if (slot(c,i).dest != c) {
            d = slot(c,i).dest;
            if (owner(d) == owner(c)) continue;          // blocked friendly move, not an attack
            if (slot(c,i).type == 10) { slot(c,i).dest = c; continue; }   // empty slot with stale dest
            any=1; if (NOT_IN_TLIST(d)) TLIST[tl++] = d;
        }
        for (j=0; TLIST[j]; j++) ALIST[n++] = {c, TLIST[j], 0};
        if (any) { CLIST[cm++] = c; CMAX++; }
        CLEAR_TLIST();
    }
}
int  NOT_IN_TLIST(int d) { for (i=0;i<10;i++) if (TLIST[i]==d) return 0; return 1; }
void CLEAR_TLIST() { for (i=0;i<11;i++) TLIST[i]=0; }
void GET_BATTLE(int *c) { *c = CLIST[RAND_NUM(0, CMAX)]; }          // random attacking country
void FIND_IN_ALIST(int *idx, int c) {                                // scan until a never-used {0,0,0} record
    for (i=0; ALIST[i].src != 0 || ALIST[i].done == 1; i++) if (ALIST[i].src == c) { *idx = i; return; }
}
void UPDATE_CLIST(int c) {           // remove c from CLIST when it has no live ALIST record left
    live = 0; for (i=0; ALIST[i].src || ALIST[i].done; i++) if (ALIST[i].src == c) live = 1;
    if (!live) for (i=0; CLIST[i]; i++) if (CLIST[i] == c) { CLIST[i] = CLIST[CMAX]; CLIST[CMAX] = 0; CMAX--; }
}
void CLEAR_ALIST() { for (i=0; ALIST[i].src; i++) ALIST[i].src = ALIST[i].tgt = 0;  for (i=0; CLIST[i]; i++) CLIST[i]=0; }
// (note: done flags are not cleared and the scan stops at the first src==0 record)

// Decides whether an ALIST record is a real battle. Side effect: undefended target is captured and razed.
int BATTLES_CONFIRMED(int a) {
    s = ALIST[a].src; t = ALIST[a].tgt; attackers=0; defenders=0;
    for (i=0;i<10;i++) {
        if (slot(s,i).type != 10 && slot(s,i).dest == t) attackers = 1;
        if (slot(t,i).type != 10) defenders = 1;
    }
    if (!defenders && attackers) {
        CLEAR_STRUCT(t);                      // *** all structures destroyed ***
        owner(t) = owner(s);                  // capture
        for (i=0;i<10;i++) if (slot(s,i).dest == t && slot(s,i).type != 10) {
            slot(t,i).type = slot(s,i).type; slot(t,i).status = slot(s,i).status;   // same slot index! (dest of slot(t,i) unchanged)
            slot(s,i).type = 10;
        }
    }
    return attackers && defenders;
}
void CLEAR_STRUCT(int c) {               // raze a country
    cty[c]+226 = 0;                       // fort level
    cty[c]+388 = 0;                       // factory bays
    for (i=0;i<24;i++) { bay[c][i] = 10; if (i < 7) buildslot[c][i] = 0; }    // +334.., +192..+204 incl. government
}
void RETURN_STRANDED() { for (c=1;c<=72;c++) for (i=0;i<10;i++) slot(c,i).dest = c; }
void CLEAR_ATTACK_FLAG() { for (c=1;c<=72;c++) cty[c]+392 = 5; }
void SET_OWNERS() {                       // (called right after; in scope of turn flow but needed to understand neutrality)
    for (c=1;c<=72;c++) if (buildslot[c][0] != 1) {           // no government
        n=0; for (i=0;i<10;i++) if (slot(c,i).type != 10) n++;
        if (n == 0) { owner(c) = 5; CLEAR_STRUCT(c); }
    }
}
```

### 1.5 Resulting movement/attack rules (for the Rust port)
1. Peaceful moves (same owner) and moves into neutral countries are executed first, in country order
   1..72, slot order 0..9, repeated 8 times, and the whole move+battle cycle repeats while anything changed.
   A move is executed only if the destination is "friendly" (`FRIENDLY_COUNTRY`: no unit in it is leaving
   towards another owner) and has a free slot. Units that cannot be placed stay plotted and, if still
   unplaced at the very end, are returned home by `RETURN_STRANDED` (their `dest` is reset; they keep their
   position in the source country).
2. **Moving into a neutral country captures it immediately, without a battle** (neutral countries never
   hold units in CYBOR.PRG; `_SET_OWNERS` guarantees a neutral country has no units). Several players moving
   into the same neutral country the same turn: the lowest-numbered source country captures it in pass 0;
   every other source now targets an enemy-owned country and becomes an attack against the units that just
   moved in.
3. Attacks: `LOAD_ALIST` collects one record per (source, target) pair. Battle order: a random source
   country (`rand(0, CMAX)` over `CLIST`), then its targets in slot-discovery order. Multiple source countries
   attacking the same target fight **separate sequential battles** against whatever is in the target at that
   moment (after the first attacker wins, the target belongs to him and his survivors are its defenders;
   if the second attacker is the same player, `owner(src)==owner(tgt)` → no battle; the units are left
   plotted, and the next cycle of the outer loop moves them in as a friendly move).
4. All units present in the target defend it — including units that were themselves plotted to leave it.
   If they survive as winners they are put back with their original `dest` and may still carry out their own
   attack later in the same resolution (ALIST order is random).
5. An attack on a country without units is an immediate capture; **every capture razes the country**
   (`CLEAR_STRUCT`: government, fort, factories, enhancements, all bays, +388 = 0). This happens both for
   undefended targets (`BATTLES_CONFIRMED`) and for won battles (`PLACE_VICTOR(1)`).
6. Attack flag `cty+392 = attacker owner` is set for every fought battle (won or lost), never for the
   undefended capture.
7. After everything: a country with no government and no units reverts to neutral and is razed (`SET_OWNERS`).

---------------------------------------------------------------------------------------------------

## 2. Repair

`_ALLOW_REPAIR` (own country only) draws the box (`_SET_REPAIR_BOX`: "CLICK ON BATTLE UNIT NAME TO REPAIR",
COUNTRY:, VAULT: <long>, columns BATTLE UNIT / STATUS) and lists units via `_UNIT_FILL(0x74, 0x46, 0x3E)` —
only units with `dest == COUNTRY1` (not in transit) and `status < 100` are selectable. Repairs can be done in
any owned country, any number of times in a turn, as long as the vault allows; a repair is always full (to 100).

```c
void REPAIR_UNIT(int i, int hit) {
    if (!hit) return;
    t = slot(COUNTRY1,i).type;
    ADJUST_VAULT(i, &ok, UNITCOST[t]);
    if (ok) {
        slot(COUNTRY1,i).status = 100;  redraw status (white) and VAULT; BOXXCOORD[i] = BOXYCOORD[i] = -1;   // not selectable any more
    } else {
        wait for button release; SND_ON(_ERROR); print "YOU DO NOT HAVE / ENOUGH MONEY TO / REPAIR THIS CYBORG"; DELAY2(DELAYTIME*60); erase
    }
}
void ADJUST_VAULT(int i, int *ok, long unitcost) {
    decile = slot(COUNTRY1,i).status / 10;                       // 0..10
    long cost = (long)(REPFACTOR[decile] * (float)unitcost);     // float multiply, truncated
    if (vault[CURPLAY] > cost) { *ok = 1; vault[CURPLAY] -= cost; }   // strictly greater; ok is left untouched (0) otherwise
}
```
Cost = `UNITCOST[type] * REPFACTOR[status/10]`: status 0-9 → 60 %, 10-19 → 55 %, 20-29 → 50 %, 30-39 → 45 %,
40-49 → 40 %, 50-59 → 35 %, 60-69 → 30 %, 70-79 → 20 %, 80-89 → 10 %, 90-99 → 5 % of the base unit price (the
manual's table says 6 % per decile; the code is authoritative). A unit must have strictly more money than the
cost in the vault.

---------------------------------------------------------------------------------------------------

## 3. Spy

Entry: Status/Spy icon on a country **not** owned by the current player → `_DO_SPY_INFO`.
```c
void DO_SPY_INFO() {
    if (year == 2200 && season == 1) {          // [0xCADD]==0x898 && [0xCADB]==1: first turn of the game
        print "SPYING ON THE INITIAL TURN IS PROHIBITED." "SELECTION CANCELLED."; error sound; delay; return;
    }
    GET_AMOUNT_OF_SPACE();                       // computes FORTLEVEL, FACTNUM, ENHANCENUM, TONNAGE, ... of COUNTRY1 (see below)
    FORTTYPE[0..3] = "NONE","LIGHT","MEDIUM","HEAVY";
    draw box "INVESTIGATE: <country>", "ENTER THE AMOUNT MONEY YOU WISH TO SPEND.", "VAULT: <vault>", "AMOUNT TO SPEND", 4 arrow buttons
    GET_MONEY();
}
// GET_AMOUNT_OF_SPACE (country COUNTRY1): FREESPACE = #build slots == 0; FORTFLAG/FORTLEVEL from slot==2 (level = cty+226);
//   FACTFLAG, FACTNUM = #slots in 3..6; ENHANCENUM = #slots == 7; MECHSPACE = #empty cyborg slots;
//   TONNAGE = sum of HL[type] over units present with dest == COUNTRY1 (units in transit out are not counted);
//   BAYSPACE = #empty bays among the first (cty+388) bays.
```
`_GET_MONEY` (amount selection loop, `_SPYMONEY` long, starts at 0):
* single down arrow (x 0x73..0x8A, y 0x6E..0x7C): −1000, or −5000 when the amount is exactly 5000 (→ 0);
* double down arrow (y 0x5E..0x6C): −10000; results in (0, 5000) are raised to 5000; negative → 0;
* single up arrow (x 0xB4..0xCB, y 0x6E..0x7C): +5000 when the amount is 0, else +1000;
* double up arrow: +10000;
* every increase is capped at 100000 and at the player's vault; an amount below 5000 after an increase is reset to 0;
* OK (x 0xBF..0xD0, y 0x1A..0x2D) → `vault -= SPYMONEY; RESOLVE_SPY()`.
So the valid amounts are 0 or 5000..100000 in steps of 1000 (≤ vault); the money is spent whatever the result.
(Edge case: the double up arrow does not apply the "<5000 → 0" reset, so with a vault below 5000 it leaves
the amount = vault and the player can spy with that.)

```c
void RESOLVE_SPY() {
    if (SPYMONEY == 0) return;                                       // nothing happens
    if (owner(COUNTRY1) == 5) { SHOW_NEUTRAL(); return; }
    m = SPYMONEY / 1000;
    s = FORTLEVEL*10 - m;  if (FORTLEVEL == 3) s += 10;  if (s < 0) s = 0;
    if (RAND_NUM(s, s+9) < 10) SHOW_SPY(); else SHOW_NEUTRAL();     // success probability = max(0, 10-s)/10
}
```
Success chance (m = thousands spent): no fort → always; light fort → m/10 (certain at 10000); medium →
(m-10)/10 (certain at 20000, impossible below 11000); heavy → (m-30)/10 (certain at 40000, impossible
below 31000). Failure shows the "neutral"-style vague report (and the money is still gone).

```c
void SHOW_SPY() {
    acc = SPYMONEY/1000 + (100 - 25*(FORTLEVEL+1));  if (acc > 100) acc = 100;   // accuracy 0..100
    half = (int)(acc / 2.0);
    roll = RAND_NUM(half, 100);                       // one roll used for both estimates below
    play _SPYING sound
    if (acc > 60) est_tonnage = (RAND_NUM(1,2)==1) ? (int)(TONNAGE * (roll/100.0))
                                                   : (int)(TONNAGE * (2.0 - roll/100.0));   // under- or over-estimate
    else est_tonnage = 0;
    est_value = (RAND_NUM(1,2)==1) ? (int)(value * (roll/100.0)) : (int)(value * (2.0 - roll/100.0));   // value = cty+386
    print "THIS COUNTRY IS OCCUPIED BY: <owner name>"
          "GOVERNMENT CONTROL" + (buildslot[0]==1 ? "IS ESTABLISHED" : "NOT ESTABLISHED")
          "NETWORTH: <est_value*1000>"            (as long)
          "BUILD SPACE: <cty+224>"                (exact)
          "EST. TOTAL TONNAGE: <est_tonnage>"
    if (acc > 60)  "FORTIFICATION LEVEL: <FORTTYPE[FORTLEVEL]>"   (printed in colour 0 when level 3, else 1)
    if (acc > 70)  "NUMBER OF FACTORIES: <FACTNUM>"  and  "FACTORY POTENTIAL: <cty+388>"
    if (acc > 80)  "FACTORY UPGRADES: <ENHANCENUM>"
    CHECK_OK(); SHOW_STATUS();
}
void SHOW_NEUTRAL() {                                  // neutral country, or failed spy attempt
    acc = SPYMONEY/1000 + (100 - 25*(FORTLEVEL+1));  if (acc > 100) acc = 100;
    roll = RAND_NUM(acc, 100);                         // note: not acc/2 here
    est_value = (RAND_NUM(1,2)==1) ? (int)(value * (roll/100.0)) : (int)(value * (2.0 - roll/100.0));
    print "THIS COUNTRY APPEARS TO NETWORTH:" <est_value*1000>, "BUILD SPACE: <cty+224>"; CHECK_OK();
}
```
Accuracy therefore equals m + 75 (no fort), m + 50 (light), m + 25 (medium), m (heavy), capped at 100;
estimates are exact when acc = 100 (roll forced to 100). Fog of war: the spy report is the only way to see
enemy tonnage/fort/factories; the own-country "glimpse" uses the same `_TONNAGE` sum (`_DO_COUNTRY_INFO`,
other agent). `_MOUSE_DOWN` / `_MOUSE_UP` are just `EVNT_BUTTON` wrappers waiting for press / release into
`_EV_BMX/_EV_BMY`.

---------------------------------------------------------------------------------------------------

## 4. Battle: loading the parties and the simulation

### 4.1 Loading (`_LOAD_ATTACK`, `_LOAD_DEFEND`, `_HITS_LEFT`, `_SORT_ACTIONLIST`, `_FIND_NEXT_CYB`)
```c
int HITS_LEFT(int c, int i, int isAttacker) {           // tons of armour left for the unit in slot i
    idx = isAttacker ? i : i+10;
    return (long)( (double)HL[ACTIONLIST[idx]] * ((double)slot(c,i).status / 100.0) );   // truncated
}
void LOAD_ATTACK(int s, int t) {
    ATTCONFIRMED = 0; ATTACKCTY = s; DEFENDCTY = t;
    for (i=0;i<10;i++) { ACTIONLIST[i] = 10; HITS[i] = 0; ACTIONTEMP[i] = 10; }
    for (i=0;i<10;i++) if (slot(s,i).dest == t) {        // (empty slots have dest == s, so only real units)
        ACTIONLIST[i] = slot(s,i).type;  HITS[i] = HITS_LEFT(s, i, 1);
        slot(s,i) = {10, 100, s};                        // removed from the map
        ATTCONFIRMED = 1;
    }
}
void LOAD_DEFEND(int t) {
    DEFCONFIRMED = 0;
    for (i=10;i<20;i++) { ACTIONLIST[i] = 10; HITS[i] = 0; ACTIONTEMP[i] = 10; }
    for (i=0;i<10;i++) {                                 // ALL slots, including empty ones (type 10, hits computed from HL[10] garbage*status/100 -> irrelevant, they are dropped by SORT)
        ACTIONLIST[10+i] = slot(t,i).type;  HITS[10+i] = HITS_LEFT(t, i, 0);
        slot(t,i).type = 10; slot(t,i).status = 100;     // dest is NOT touched
        DEFCONFIRMED = 1;
    }
}
void SORT_ACTIONLIST(int lo, int hi) {                   // compact live entries to the front of [lo,hi)
    for (i=lo;i<hi;i++) {
        if (ACTIONLIST[i] == 10) { j = FIND_NEXT_CYB(i+1, hi);          // first live entry after i, or -1
            if (j != -1) { ACTIONLIST[i]=ACTIONLIST[j]; HITS[i]=HITS[j]; ACTIONLIST[j]=10; } }
        ACTIONTEMP[i] = ACTIONLIST[i];
    }
    for (i=lo;i<hi;i++) if (ACTIONLIST[i] == 10) HITS[i] = -1;
}
```
After loading, the attacker's slots in the source country and all slots of the target are empty; the units
exist only in ACTIONLIST until placed again. `_PREPARE_FOR_BATTLE(player)` is a pure splash ("prepare for
battle", player flag + name, fade in / delay / fade out).

### 4.2 Party statistics (`_INIT_PARTIES`, `_SET_TERRAIN_FACTOR`)
```c
void SET_TERRAIN_FACTOR() { switch (terrain(DEFENDCTY)) { case 1: HEATFACTOR=1; case 2: case 5: HEATFACTOR=2; case 3: case 4: HEATFACTOR=3; } }
void INIT_PARTIES() {
    zero ATTHET,ATTPOW,ATTRAN,ATTSPD,ATTSIK,ATTFH,ATTTP,ATTTON and the DEF* twins; na = nd = 0;
    closeTerrain = (terrain(DEFENDCTY) == 2 || terrain(DEFENDCTY) == 5);      // coniferous or city: long-range guns useless
    for (i=0;i<20;i++) if (ACTIONLIST[i] != 10) {
        t = ACTIONLIST[i];  side = (i<10) ? ATT : DEF;
        side.HET += ST[t][3];
        side.POW += closeTerrain ? ST[t][1] : ST[t][0] + ST[t][1];
        side.RAN += ST[t][2];  side.SPD += ST[t][5];  side.SIK += ST[t][6];  side.FH += ST[t][7];  side.TP += ST[t][4];
        side.TON += HITS[i];
        side.n++;
    }
    ATTHET *= HEATFACTOR;  DEFHET *= HEATFACTOR;              // 16-bit multiply
    ATTPOW = (ATTPOW + ATTHET) / 4;   DEFPOW = (DEFPOW + DEFHET) / 4;
    ATTRAN /= na;  DEFRAN /= nd;  ATTSPD /= na;  DEFSPD /= nd;   // averages (na, nd > 0 is guaranteed by BATTLES_CONFIRMED)
}
```
Only POW, RAN, SPD, TON are used afterwards (SIK/FH/TP are computed and ignored).

### 4.3 The simulation (`_BATTLE_SIMULATION`, `_RESOLVE_BATTLE`, `_DAMAGE_CITY`, `_DEDUCT_DAMAGE`)
```c
void BATTLE_SIMULATION() {
    VICTOR = -1; ATTTONLOSS = DEFTONLOSS = 0;
    INIT_PARTIES(); SHOTS = 0; SET_TERRAIN_FACTOR();
    att0 = ATTTON; def0 = DEFTON;
    switch (cty[DEFENDCTY]+226) {                     // fortification multiplies defender firepower
        case 1: DEFPOW = (int)(DEFPOW * 1.5); break;
        case 2: DEFPOW = (int)(DEFPOW * 2.0); break;
        case 3: DEFPOW = (int)(DEFPOW * 4.0); break;
    }
    RESOLVE_BATTLE();
    if (terrain(DEFENDCTY) == 5) DAMAGE_CITY();
    if (ATTTON <= 0 && DEFTON <= 0) return;           // mutual annihilation: VICTOR stays -1
    if (ATTTON > 0) {                                 // attacker wins
        VICTOR = 2;
        ATTTONLOSS = att0 - ATTTON;  if (ATTTONLOSS + 6 > att0) ATTTONLOSS -= 6;
        DEDUCT_DAMAGE(ATTTONLOSS, /*winner base*/ 0, /*loser base*/ 10);
    } else {                                          // defender wins
        VICTOR = 1;
        DEFTONLOSS = def0 - DEFTON;  if (DEFTONLOSS + 6 > def0) DEFTONLOSS -= 6;
        DEDUCT_DAMAGE(DEFTONLOSS, 10, 0);
    }
}
void RESOLVE_BATTLE() {                                // exchange of volleys until one side's tonnage is gone
    while (ATTTON > 0 && DEFTON > 0) {
        if (RAND_NUM(1,100) < ATTRAN - DEFSPD) DEFTON -= RAND_NUM(ATTPOW/4, ATTPOW);
        if (RAND_NUM(1,100) < DEFRAN - ATTSPD) ATTTON -= RAND_NUM(DEFPOW/4, DEFPOW);
    }
}
void DAMAGE_CITY() {                                   // only when the defending country is a city
    if (SHOTS > 50) SHOTS = 15; else SHOTS += 30;      // SHOTS was 0 -> 30 volleys hit the city
    CITYDAMAGE = 0;
    while (SHOTS > 0) { p = DEFPOW + ATTPOW; CITYDAMAGE += RAND_NUM(p/4, p); SHOTS--; }
    cty[DEFENDCTY]+384 += (int)((double)CITYDAMAGE / 20000.0 * 100.0);     // bytes verified: fdiv [20000.0], fmul [100.0]
    if (cty[DEFENDCTY]+384 > 100) cty[DEFENDCTY]+384 = 100;
}
// Distribute the winner's tonnage loss randomly over its units; the loser's units all die.
void DEDUCT_DAMAGE(int loss, int winBase, int loseBase) {
    while (loss > 0) {
        tries = 15;
        u = RAND_NUM(winBase, winBase+9);
        if (ACTIONLIST[u] == 10 || HITS[u] <= 0) continue;        // pick again
        while (tries > 0 && loss > 0) {
            d = RAND_NUM(3,6); tries--;
            HITS[u] -= d; loss -= d;
            if (HITS[u] < 0) { ACTIONLIST[u] = 10; tries = 0; }   // unit destroyed (HITS == 0 survives the loop but is never placed)
        }
    }
    for (u=loseBase; u<loseBase+10; u++) HITS[u] = -1;
}
```
Facts for the port: the hit chance of a volley is `(RAN_avg - SPD_avg - 1)/100`; each hit removes
`rand(POW/4, POW)` tons where `POW = (Σfirepower + HEATFACTOR·Σheat)/4` (long-range firepower only counts
outside coniferous/city terrain), multiplied by 1.5/2/4 for light/medium/heavy fortification on the defender.
Both sides fire every round, so both can reach ≤0 in the same round (draw). Tonnage = Σ `HL[type]·status/100`.
The winner's loss is reduced by 6 tons when it would otherwise leave less than 6 tons, and is then dealt
in random 3..6-ton bites to random living units (a unit dies when its hits go below 0). The simulation
never sets `_ARET/_DRET`, so no retreats happen in simulated battles. `_PLAY_SIM_SOUND`: 10× one of
cannon/PPC/laser (rand 1..3) then an explosion.

### 4.4 Placing survivors (`_PLACE_VICTOR`, `_CALCULATE_DAMAGE`, `_RESET_DESTIN`)
```c
int CALCULATE_DAMAGE(int hits, int type) {             // hits -> status %
    if (HL[type] - 2 < hits) return 100;
    return (int)( (float)( (float)hits / CD[type] ) );  // == hits*100/HL[type], via float
}
void PLACE_VICTOR(int attackerWon) {
    if (attackerWon) {
        CLEAR_STRUCT(DEFENDCTY);                         // conquest razes everything
        for (i=0;i<10;i++) if (HITS[i] > 0) {           // attackers with hits left (compacted order)
            slot(DEFENDCTY,i).type   = ACTIONTEMP[i];
            slot(DEFENDCTY,i).dest   = DEFENDCTY;
            slot(DEFENDCTY,i).status = CALCULATE_DAMAGE(HITS[i], ACTIONTEMP[i]);
        }
    } else {
        for (i=10;i<20;i++) if (HITS[i] > 0) {          // defenders go back into slots 0..9 (dest field unchanged)
            slot(DEFENDCTY,i-10).type   = ACTIONTEMP[i];
            slot(DEFENDCTY,i-10).status = CALCULATE_DAMAGE(HITS[i], ACTIONTEMP[i]);
        }
    }
    RESET_DESTIN();
}
void RESET_DESTIN() {   // every EMPTY slot of both countries gets dest = own country
    for (i=0;i<10;i++) { if (slot(DEFENDCTY,i).type == 10) slot(DEFENDCTY,i).dest = DEFENDCTY;
                         if (slot(ATTACKCTY,i).type == 10) slot(ATTACKCTY,i).dest = ATTACKCTY; }
}
```
Units with exactly 0 hits (and the losers, hits = -1) vanish. Status after battle = hits·100/HL (so a Titan
with 300 tons left is at 40 %); 100 if hits > HL-2.

### 4.5 Retreats (ACTION.PRG only — `_ARET/_DRET`)
```c
void ATTACKER_RETREAT() {                     // survivors return to the country the attack came from
    for (u=0;u<10;u++) if (HITS[u] > 0)
        for (i=0;i<10;i++) if (slot(ATTACKCTY,i).type == 10) {              // first free slot
            slot(ATTACKCTY,i) = { ACTIONTEMP[u], CALCULATE_DAMAGE(HITS[u], ACTIONTEMP[u]), ATTACKCTY }; break; }
}
void DEFENDER_RETREAT() { for (u=10;u<20;u++) if (HITS[u] > 0) LOCATE_ACCEPTING_CTY(u); }
void LOCATE_ACCEPTING_CTY(int u) {            // first adjacent country with the same owner that has a free slot
    notPlaced = 1;
    for (k=0; adj(DEFENDCTY)[k] != 0 && notPlaced; k++) {
        a = adj(DEFENDCTY)[k];
        if (owner(a) == owner(DEFENDCTY)) PLACE_RETREATY(&notPlaced, a, u);
    }
    // nobody accepts -> the unit is lost ("if there are no adjacent friendly countries left, the cyborgs are destroyed")
}
void PLACE_RETREATY(int *notPlaced, int a, int u) {
    for (i=0;i<10;i++) if (slot(a,i).type == 10) {
        slot(a,i) = { ACTIONTEMP[u], CALCULATE_DAMAGE(HITS[u], ACTIONTEMP[u]), a }; *notPlaced = 0; break; }
}
```
Note `owner(DEFENDCTY)` is still the defender's owner when `DEFENDER_RETREAT` runs (the capture is applied
after `PLACE_VICTOR`). A retreating attacker that finds no free slot at home (impossible in practice, since
its own slots were freed by LOAD_ATTACK) would be lost.

### 4.6 Structures after a real-time battle (`_CHECK_PIECES`, `_ADJUST_DATA_STRUCT`, `_GET_DAMAGE`)
Only on return from ACTION.PRG. `_PIECES` (26 records of 6 bytes, +4 = alive) and `_PSTRUCT[7]` (structure
type per build slot, filled by the terrain-load code of the action setup) describe the structures that were
drawn on the battle map.
```c
void CHECK_PIECES() {
    BDLEVEL = 0; destroyedTotal = 0; first = 1;
    if (PIECES[0].alive == 0 && buildslot[DEFENDCTY][0] == 1) {      // the capital piece is gone
        CLEAR_STRUCT(DEFENDCTY); BDLEVEL = 30; return;               // "CAPITAL HAS BEEN DESTROYED ... ALL STRUCTURES CEASE TO EXIST"
    }
    for (s=1; s<7; s++) if (PSTRUCT[s] != 0) {
        width = (PSTRUCT[s] == 7) ? 1 : PSTRUCT[s] - 2;              // pieces belonging to this structure (fort: 0, factories: bays, upgrade: 1)
        destroyed = 0;
        for (p=first; p<first+width; p++) if (PIECES[p].alive == 0) { destroyed++; BDLEVEL++; }
        if (destroyed > 0) ADJUST_DATA_STRUCT(destroyed, s);
        first += width;
    }
}
void ADJUST_DATA_STRUCT(int destroyed, int s) {
    if (buildslot[DEFENDCTY][s] == 7) { buildslot[DEFENDCTY][s] = 0; return; }        // enhancement gone (bays untouched)
    if (buildslot[DEFENDCTY][s] - destroyed <= 2) buildslot[DEFENDCTY][s] = 0;         // factory wiped out
    else buildslot[DEFENDCTY][s] -= destroyed;                                          // factory downgraded (quadro 6 -> ... )
    cty[DEFENDCTY]+388 -= destroyed;                                                    // bay count reduced
}
void GET_DAMAGE(int type, int hits) {   // ACTION-side helper, same formula as CALCULATE_DAMAGE, result in _CYDAMAGE
    CYDAMAGE = (HL[type]-2 < hits) ? 100 : (int)((float)hits / CD[type]);
}
```
(Bays that are removed this way are not cleared in +334; the cyborgs under construction in them are simply
beyond the new bay count. Whether that is intended is an open question for the build/economy agent.)

### 4.7 Battle summary (`_BATTLE_SUMMARY`, `_CYBORG_DAMAGE_MESSAGE`, `_GET_RIGHT_MESSAGE`, `_BUILD_DAMAGE_MESSAGE`)
Drawing: loads button10.gif/door3.gif, `_BUILD_BATTLE_STATS`, both player flags/names, then lists the
destroyed units: every ACTIONLIST entry with `HITS < 0` and `ACTIONTEMP != 10` — attackers at x=0x1D,
defenders at x=0xFC, 6 px per line — counting `attLost` and `defLost`. Then:
* `VICTOR != 0`: `CYBORG_DAMAGE_MESSAGE(attLost, defLost)`:
  winner name + verb + loser name, verb chosen by `GET_RIGHT_MESSAGE(d)` where
  d = loserLosses − winnerLosses (attacker won: `defLost − attLost`; defender won: `attLost − defLost`):
  d < 3 " HAS DEFEATED ", 3..5 " HAS CRUSHED ", ≥ 6 " HAS DECIMATED ". If the loser retreated (`_DRET`
  when attacker won, `_ARET` when defender won) the line is instead "<loser> HAS SPINELESSLY RETREATED".
  (VICTOR == -1 also goes through this path and prints the defender as winner — see open questions.)
  Then `BUILD_DAMAGE_MESSAGE(VICTOR == 1)`.
* `VICTOR == 0` (never produced by this code): "OPPOSING FORCES WERE BOTH DESTROYED".
```c
void BUILD_DAMAGE_MESSAGE(int defenderWon) {
    if (BDLEVEL == 30 && defenderWon)      print "CAPITAL HAS BEEN DESTROYED............." / "ALL STRUCTURES CEASE TO EXIST";
    else if (BDLEVEL > 1 && defenderWon)   print "<BDLEVEL> STRUCTURES HAVE BEEN DESTROYED";
    else if (BDLEVEL == 1 && defenderWon)  print "1 STRUCTURE HAS BEEN DESTROYED";
    else if (T_NUM != 4 && defenderWon)    print "NO STRUCTURES HAVE BEEN DESTROYED";
    else if (T_NUM == 4) {                                             // city map
        delta = max(0, cty[DEFENDCTY]+384 - OLDCITYDAM);
        print "CONFLICT HAS DAMAGED CITY <delta> PERCENT";  print "CITY DAMAGE CURRENTLY STANDS AT <cty+384>";
    }
}
```
Structure messages are only shown when the defender won (when the attacker won everything was razed
anyway). The screen waits for a click on OK (x 0xF2..0x12B, y 0x2A..0x37) and fades out. `_CLEAR_SOUNDS`
zeroes `_SOUNDS[5]`.

### 4.8 `_RESOLVE_MECHS2`, `_DO_CAPTURE` (Battle Practice / "showdown" only)
Not part of the strategic resolution. `_RESOLVE_MECHS2(side)` fills ACTIONLIST[side?10:0 ..+9] from
`_TEMPBAY[i]` (bought units encoded as `type + 100*k`: `type = TEMPBAY[i] % 100`) with full hits
`_ARMOR(type) = HL[type]`; used by `_DO_SHOWDOWN` for computer-bought forces. `_DO_CAPTURE` =
`GET_T_PLAYERS(); CHOOSE_TERRAIN();` (practice-battle setup). `_ARMOR(t)` returns `HL[t]`.

---------------------------------------------------------------------------------------------------

## 5. Pure drawing / UI helpers (one line each)
* `_REPAIR_MESSAGE` / `_MOVE_MESSAGE` / `_SPY_MESSAGE`: 3–4 lines of banner text in the message box
  ("REPAIR OPTION / SELECT THE COUNTRY / YOU WISH TO REPAIR", "MOVE OPTION: / SELECT A COUNTRY / TO MOVE FROM.",
  "SPY OPTION / SELECT THE COUNTRY / YOU WOULD LIKE TO / INVESTIGATE") then `_DMAP`.
* `_ALLOW_MOVE`: "SELECT A DESTINATION / COUNTRY."; `_MESSAGE2`: adjacency error; `_DISALLOW_OP`: ownership error (both with error sound and DELAYTIME·60 ticks delay).
* `_DO_MOVE_BOX` / `_ALLOW_REPAIR`: blit the box background, `_CLEAR_I_BOX`, `_SET_MOVE_BOX`/`_SET_REPAIR_BOX`, `_UNIT_FILL`, `_SHOW_UNITS`, `_CLEAR_BUT_COORDS`.
* `_SHOW_UNITS`: click loop (see 1.2); `_GET_SELECTION`: hit test + dispatch; `_SET_BUT_COORDS`/`_CLEAR_BUT_COORDS`: button rectangles.
* `_MOUSE_DOWN` / `_MOUSE_UP`: wait for button press / release (`_EVNT_BUTTON`), coordinates in `_EV_BMX/_EV_BMY`.
* `_PREPARE_FOR_BATTLE(p)`: "prepare for battle" splash for player p.
* `_PLAY_SIM_SOUND`: weapon sound ×10 + explosion. `_CLEAR_SOUNDS`: zero `_SOUNDS[]`.
* `_SET_ACTION_PHASE` also saves/restores `_PZ` and calls `_START/_START2` (exec of ACTION.PRG) — see the turn-flow / action-setup notes.

---------------------------------------------------------------------------------------------------

## 6. Open questions
1. **Neutral countries with units.** `UNIT_MOVES` captures a neutral country without a fight; this is only
   sound if neutral countries never hold cyborgs. Writers of the slot type field are `_GAME_INIT`,
   `_INC_DATE` (bay completion) and the functions above; the turn-flow agent should confirm `_GAME_INIT`
   never places units in neutral countries (and that `_INC_DATE` cannot complete a bay in a country that
   became neutral — `CLEAR_STRUCT` empties the bays, so it should be impossible).
2. `VICTOR == -1` (both sides annihilated in the simulation) still produces a "<defender> HAS DEFEATED
   <attacker>" summary line (`CYBORG_DAMAGE_MESSAGE` treats everything that is not 2 as a defender win)
   and no `SOUNDS` change; `RESET_DESTIN` is applied. The "OPPOSING FORCES WERE BOTH DESTROYED" text is
   only reachable with `VICTOR == 0`, which nothing in CYBOR sets — probably ACTION.PRG returns 0 for a draw
   (check in action.asm).
3. `LOAD_DEFEND` computes `HITS_LEFT` for empty slots too (reads `HL[10]`, i.e. 2 bytes past the table);
   harmless because `SORT_ACTIONLIST` discards them, but a port must not index out of range.
4. `CLEAR_ALIST` does not reset the `done` flags and stops at the first record with `src == 0`; since done
   records also have `src == 0`, records after the first done one are not cleared — harmless because
   `LOAD_ALIST` rewrites from index 0 and `FIND_IN_ALIST` skips done records, but the stale tail is written
   to pass2.dat.
5. `_FORTFACTOR` (shorts 0,3,6,9 + doubles 1.5/1.3/10/1.0), `_TERRAINVALUE`, `_KILLTONS` are not used by any
   function of this scope; they are probably used by the action setup / ACTION-return code (other agents).
6. Repair cost table in the manual (6 % steps) differs from `_REPFACTOR` (.60 .55 … .05); the code was
   taken as authoritative. The manual also says spying needs "at least 5,000" — matches `_GET_MONEY`.
7. The city-damage adjustment after a real-time battle uses `CPCOUNT1/2/3` set by the action-setup code
   (CPCOUNT1 = pieces before, CPCOUNT3 = ?); the exact meaning of `CPCOUNT2 = CPCOUNT3 − 3·(CPCOUNT3 − CPCOUNT2)`
   must be settled by the action-setup agent.
8. Slot fields +6/+8 are never read or written in CYBOR.PRG; check action.asm before dropping them from the
   save-file model.
