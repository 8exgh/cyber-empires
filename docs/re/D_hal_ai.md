# D — Computer player AI ("HAL", module HAL.C) in CYBOR.PRG

Source: cybor.asm functions in segment 0x1266 (`_HALS_TURN` .. `_CLEAR_FACTORY_PLACE`), plus
`_COMPUTER_BUY`/`_TRYBUY` (seg 0x1815) and the action-setup helpers `_SET_AGGRESSION`,
`_DECIDE_ON_GOAL`, `_INCREASE/DECREASE_ATTACKER_AGGRESS`, `_CHANGE_AGGRESS_MESSAGE`, `_SET_MONKEY`
(seg 0x9BA). Helper used to extract a function: `re/fn.sh _NAME`.

All data below was read directly from the disassembly; every constant is exact. Where the original has
an out-of-bounds read or a stale-global dependency it is called out as **QUIRK** (reproduce for
fidelity, or deliberately fix — decision for the Rust port).

---------------------------------------------------------------------------------------------------

## 0. Data model facts discovered here (extend CONTEXT.md)

Country record (236 bytes, country c at `_MECHRECORD + c*236`), offsets relative to record start:

| off | meaning |
|-----|---------|
| +172 | owner 0..4, 5 = neutral |
| +192 .. +204 | **7 build slots** `slot[i]` (shorts, i = 0..6). Codes: 0 empty, 1 capital/government, 2 fortification, 3 uni-post factory (1 bay), 4 dual-post factory (2 bays), 6 quadro-post factory (4 bays), 7 enhancement. (Code 5 is never written by the AI; tables still carry an entry for it.) Only the first `[+224]` slots are usable ("build space" = rand(2,7), cities 1); the capital always lives in slot 0 (`slot[0]==1` is the "has capital" test everywhere). Factory bays = code-2. |
| +206 .. +222 | adjacency list, 0-terminated (max 9) |
| +224 | build space (number of usable slots) |
| +226 | fortification level 0..3 (there is also a slot with code 2 when a fort exists) |
| +234 + 10*u | unit slot u (0..9): +0 type (0..8, 10 = empty), +2 status % (100 = healthy), +4 **destination country** (the AI writes the move target here; equals own country when staying — CONTEXT's "home country" is really the move destination), +6, +8 unknown |
| +334 + 2*b | factory bay b (0..23): 10 = empty, otherwise **turns_left*100 + cyborg_type** (e.g. 104 = CROSSBOLT with 1 turn left, 302 = HERCULES 3 turns). (CONTEXT had the two parts swapped.) |
| +386 | country value (networth) |
| +388 | number of factory bays (sum of FACTPOT of factories built here) |

Player record (32 bytes at `_MECHRECORD + 12 + 32*p`):

| off | meaning |
|-----|---------|
| +20 | long: vault / money (`[_MECHRECORD+32]`/`+34` for p=0) |
| +30 | **computer skill level** 0..5 (`[_MECHRECORD+42]` for p=0). Set in `_READ_HUMAN_NAME` ("CHOOSE COMPUTER SKILL LEVEL...") from the clicked button index: six buttons at x=0x70+0x10*i labelled 1..6, 'NASTY' printed left of button 0 and 'WIMPY' right of button 5. So **0 = NASTY (hardest) … 5 = WIMPY (easiest)**. Only the AI reads it. |

Trailing words of `_MECHRECORD` (after the 72 country records; raw addresses in the asm because the
symboliser did not annotate them):

| addr | `_MECHRECORD+` | meaning |
|------|----------------|---------|
| 0xCADB | +17636 | season/phase counter (init 1 in `_GAME_INIT`) |
| 0xCADD | +17638 | year (init 0x898 = 2200) |
| 0xCAE5 | +17646 | **game stage 0..10** = row index into `_BUYMATRIX`. Init 0. In `_SET_ACTION_PHASE`: `if stage < 10 && season in {1,3,5} stage++; if stage > 8 { stage = (stage == 9) ? 10 : 9; }` — i.e. it climbs 0→9 at three steps per year, then alternates 9/10 each action phase. |

Other global AI state (all in DGROUP, zero-initialised unless stated):

| symbol | type | meaning |
|--------|------|---------|
| `_HALSCTY[73]` | short | 0-terminated list of the current AI player's countries (built each turn) |
| `_HALCTYTOTAL[5]` | short | per player: country count at the end of the *previous* AI turn |
| `_AGGRESSION` | short | per turn: 1 normally, 0 if the player lost more than 2 countries since its last turn (sign of the enemy-unit term in move priority) |
| `_PRIMOVE[73]` | short | move priority of every country (destination attractiveness) |
| `_NUMUNITS[73]` | short | for every non-own country c: number of own cyborgs in countries adjacent to c |
| `_MARRAY[11]` | short | unit-slot indices that will leave the country being processed |
| `_BLITZ`, `_NOBUY`, `_NOENY` | short | per-turn flags (see below) |
| `_NUMFORTS`, `_NUMBAYS`, `_NUMCYBORGS` | short | enemy fortified capitals (capped 5), own factory bays (min 1), own cyborg count |
| `_MAXPURCHASE` | long | max price of a single cyborg this turn |
| `_TMATRIX[21]` | short | saved copy of the active `_BUYMATRIX` row |
| `_STAYNOCAP`, `_STAYCAP`, `_STAYENEMY`, `_STAYHOME` | short | garrison sizes (units that must stay) |
| `_ANEUTRAL1`, `_ANEUTRAL2` | short | the two neutral countries designated as targets (never reset — **QUIRK**, stale across turns/players) |
| `_FACPLACE[20]`, `_FACINDEX` | short | candidate countries for a factory (fully surrounded by own countries) |
| `_FLEVEL` | short | fort level chosen by `_GET_ACCEPTING_COUNTRY(kind 4)` |
| `_TEMPBAY[24]`, `_MFCOST[9]` (long), `_MFBT[9]` | | scratch for unit purchase |
| `_COUNTRY1` | short | "current country" global shared with the UI code; many helpers (`_GET_AMOUNT_OF_SPACE`, `_FILL_BUILD_AREA`, `_INSERT_BAY`, `_ADJUST_VAULT`) operate on it implicitly |
| `_FREESPACE`, `_BAYSPACE`, `_ENHANCENUM`, `_FORTFLAG`, `_FORTLEVEL`, `_FACTFLAG`, `_FACTNUM`, `_MECHSPACE`, `_TONNAGE` | short | outputs of `_GET_AMOUNT_OF_SPACE(COUNTRY1)` |

Tables (initial values, `re_cybor_tables.txt`):

* `_BUYMATRIX[12][21]` shorts (row = game stage 0..10, row 11 all zero/unused). Columns:

  | col | item | col | item |
  |-----|------|-----|------|
  | 0 | capital (30000) | 11 | CYCLOPS |
  | 1 | uni-post factory | 12 | MERCURY |
  | 2 | dual-post factory | 13 | fort (light) |
  | 3 | quadro-post factory | 14 | fort (medium) — never non-zero |
  | 4 | TITAN | 15 | fort (heavy) — never non-zero |
  | 5 | BEHEMOTH | 16 | enhancement (400000) |
  | 6 | HERCULES | 17–19 | unused (0) |
  | 7 | ACHILLES | 20 | **starting priority level** |
  | 8 | CROSSBOLT (gets `+NUMFORTS`) | | |
  | 9 | MARS | | |
  | 10 | DRAGON | | |

  ```
  stage  CAP F1 F2 F3 | TIT BEH HER ACH CRO MAR DRA CYC MER | FT1 FT2 FT3 | ENH | -- -- -- | LEVEL
    0      8  0  0  5 |   0   0   0   0   0   0   0   3   6 |   0   0   0 |   2 |  0  0  0 |   8
    1     15  0  0  1 |   0   0   6   6   6   8   7   7   1 |   0   0   0 |   2 |  0  0  0 |  15
    2     15  0  0  1 |   0   0   8   8   9   8   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    3     15  0  0  1 |   0   6   8   8   9   6   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    4     15  0  0  1 |   0   8   8   8   9   5   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    5     15  0  0  1 |   8   8   8   8   9   6   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    6     15  0  0  1 |  10  10  11  11  13   8   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    7     15  0  0  1 |  10  10  10  10  10   8   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    8     15  0  0  1 |  12  12  13  13  13   8   4   4   1 |   1   0   0 |   2 |  0  0  0 |  15
    9     35  0  0  1 |  17  17  20  12  24   8   4   4   2 |   0   0   0 |   2 |  0  0  0 |  35
   10     35  0  0  1 |  24  24  26  25  26  20   4   4   2 |   2   0   0 |   2 |  0  0  0 |  35
  ```
  A cell value v means "this item is attempted v times per turn, interleaved with the other items by
  descending priority level" (see `_HAL_BUYS`). Column 0 always equals column 20, so the first
  attempt of every turn is a capital.

* `_BUILDPRI[7]` = {100, 0, 1000, 2000, 0, 5000, 200} — move-priority bonus per occupied build slot,
  indexed by slot code - 1 (capital 100, fort 0, uni 1000, dual 2000, [code5] 0, quadro 5000, enhancement 200).
* `_CTYVAL[7]` = {30, 0, 500, 800, 1200, 1500, 200} — "fort-worthiness" value per slot code - 1
  (capital 30, fort 0, uni 500, dual 800, [code5] 1200, quadro 1500, enhancement 200). `_CTYVAL[-1]`
  (read for empty slots, **QUIRK**) is the last short of `_BUILDPRI` = **200**.
* `_FCYB[9]` = {200, 180, 130, 90, 50, 50, 30, 20, 10} — threat weight per cyborg type
  TITAN..MERCURY (used for enemy stacks and to spread own units).
* `_STAY[6][2]` = {{1,0},{1,0},{1,1},{2,1},{2,2},{4,4}} indexed by skill: {units that must stay in a
  country without capital, with capital} when no enemy is adjacent.
* `_FORTFACTOR[4]` shorts = {0, 3, 6, 9} (extra garrison the AI assumes a fort level is worth; the
  symbol's remaining 32 bytes are the compiler's double constants 1.5, 1.3, 10.0, 1.0 used below).
* `_FACTCOST` {500000, 800000, 1200000}, `_FACTPOT` {1,2,4}, `_FACTFILL` {3,4,6} (slot code written),
  `_FORTCOSTS` {100000, 200000, 300000}, `_UNITCOST`, `_BUILDTIME` {5,4,3,3,4,2,1,1,1},
  `_MFBTMIN` {3,2,1,1,2,1,1,1,1}, `_REPFACTOR` floats {.6,.55,.5,.45,.4,.35,.3,.2,.1,.05,0}.
* `_HCYB[9]` = {500,401,302,303,204,205,106,107,108} looks like "bay code with full build time per
  type" but is **not referenced anywhere in CYBOR.PRG** (dead data).
* `_PSCORE[36]`/`_PLEVEL[36]` are the hall-of-fame score records (`_LOAD_SCORES`/`_SAVE_SCORES`,
  `_CHOOSE_CYBORG`), not AI state.

---------------------------------------------------------------------------------------------------

## 1. The AI turn, in order

`_HALS_TURN` is called from `_INIT_MODE` for each computer-controlled player after
`_SET_PLAYER_PROMPT(1)`, `_TOGGLE_SCREENS` and a `_DELAY2(_DELAYTIME*60)`. `_CURPLAY` = that player.

```
HALS_TURN:
  1. TMATRIX = BUYMATRIX[stage]            (save row; restored at the very end, so the row is constant)
  2. BLITZ = 0; NOBUY = 1; NOENY = 0
  3. ADD_ENEMY_UNITS                       -> NUMUNITS[]
  4. ADD_ENEMY_FORTS                       -> NUMFORTS (clamped to <=5), NUMBAYS (>=1), NUMCYBORGS
  5. BUYMATRIX[stage][8] += NUMFORTS       (more CROSSBOLTs when enemies have fortified capitals)
  6. CLEAR_HALS_COUNTRIES; GET_HALS_COUNTRIES  -> HALSCTY[], AGGRESSION, HALCTYTOTAL[CURPLAY]
  7. CLEAR_COUNTRY_PRIORITY                (PRIMOVE[1..72] = 0)
  8. SET_FRONT_PRIORITY                    (own frontier countries 3000 / neutral-border 100, BUILD_FRONT ripples, sets NOENY)
  9. SET_MOVE_PRIORITY                     (for every country: value, owner, build, units terms)
 10. if NOENY: SET_NEUTRAL_PRIORITY        (designate ANEUTRAL1 = 5000, ANEUTRAL2 = 4990)
 11. SET_MOVEMENT_LEVELS                   (STAYNOCAP/STAYCAP from STAY[skill])
 12. HAL_REPAIRS                           (repair every damaged own cyborg the vault can pay for)
 13. money = player.vault
     if skill == 0: money = (long)(money * 1.5)      -- NASTY buys with 150 % of its money
     else if skill == 1: money = (long)(money * 1.3) -- level 2 buys with 130 %
     MAXPURCHASE = (long)(money / NUMBAYS)
     if MAXPURCHASE > 60000 && year < 2201: MAXPURCHASE = 50000
     if money < 300000:                     MAXPURCHASE = 40000
     if NUMCYBORGS < 15:                    MAXPURCHASE = 27000
 14. buyflag = 1
     while buyflag && money > 0:
        HAL_BUYS(&item, &buyflag)
        GET_AMOUNT_OF_SPACE()              (on stale COUNTRY1 — side effects only)
        switch item:
          0      : LOCATE_CAPITOL(&money)
          1..3   : LOCATE_FACTORY(item, &money)        (uni/dual/quadro)
          4..12  : LOCATE_BATTLE_UNIT(item, &money)    (cyborg type = item-4)
          13..15 : LOCATE_FORTIFICATION(item, &money)  (item ignored; level from country value)
          16     : LOCATE_ENHANCEMENT(&money)
        item = 100
 15. player.vault = money   (NOTE: the 1.5x/1.3x bonus money that was not spent is KEPT — the bonus
                             is real income for NASTY/level-2 computers, not just a buying allowance)
 16. HAL_MOVES
 17. BUYMATRIX[stage] = TMATRIX
```

There is no separate "build" phase: factories/forts/capitals are placed directly into the country
records during the buy loop, and cyborgs are queued into factory bays (the normal production code
counts them down).

### 1.1 Purchase scheduler (`_HAL_BUYS`)

```
HAL_BUYS(int *item, int *buyflag):
  level = BUYMATRIX[stage][20]
  *buyflag = 1
  for i in 0..16:                              (17 item columns)
     if BUYMATRIX[stage][i] == level:
        *item = i; BUYMATRIX[stage][i]--; break
  if i >= 16:                                  (no match, OR the match was column 16 = enhancement)
     BUYMATRIX[stage][20]--
     if BUYMATRIX[stage][20] < 1: *buyflag = 0
```
Effect: items whose counter equals the current level are attempted one by one in column order; when
none is left at that level the level drops by one, so an item with initial value v is attempted v
times in total, interleaved with the others. Picking the enhancement column also drops the level
immediately. The loop in HALS_TURN stops when the level falls below 1 or money hits 0.
(`*item` is left unchanged when nothing matches; HALS_TURN resets it to 100 after each iteration, and
column 0 == column 20 guarantees a match on the very first call.)

### 1.2 Placement decisions (`_GET_ACCEPTING_COUNTRY(int *cty, int kind, int *found)`)

Iterates `HALSCTY[]` (own countries, in country-number order). Sets `COUNTRY1` as a side effect
(except kind 1).

* kind 1 (capital): the own country **with the highest value (+386) whose slot[0] == 0**. No
  free-space check (capital always goes in slot 0).
* kind 2 (factory): `CLEAR_FACTORY_PLACE; FACINDEX = 0`. For each own country h: `COUNTRY1 = h;
  GET_AMOUNT_OF_SPACE`; if `FREESPACE > 0 && slot[0]==1`: `CHECK_CTY_COVER(h)` (adds h to
  FACPLACE if **every** adjacent country is owned by CURPLAY, max 20 entries), `*found = 1`,
  `*cty = h`. After the loop `COUNTRY1 = last qualifying h` (0 if none). If `FACINDEX > 0`:
  `*cty = FACPLACE[RAND_NUM(0, FACINDEX-1)]`, `COUNTRY1 = *cty`. **Otherwise the factory goes to
  the last qualifying country even though it is not interior.**
* kind 3 (cyborg): first own country with `BAYSPACE > 0` (an empty bay among its +388 bays);
  `COUNTRY1 = *cty = h`.
* kind 4 (fort): see pseudo-C; chooses the country with the highest `CALCULATE_COUNTRY_VALUE` among
  own countries with free space, a capital and a cyborg in unit slot 0, and derives `FLEVEL`:
  value in (500,800] → 1, (800,1200] → 2, > 1200 → 3, unless the country already has a fort of
  level ≥ that → candidate discarded (best reset to 0). `COUNTRY1 = *cty` at the end (0 if none).
* kind 5 (enhancement): the **last** own country with `FREESPACE > 0 && slot[0]==1 &&
  ENHANCENUM < 2 && bays(+388) >= 4`. `COUNTRY1 = *cty`.

`CALCULATE_COUNTRY_VALUE(c)` = Σ over all 7 slots of `CTYVAL[slot-1]` (empty slot → **200**,
out-of-bounds read of `_BUILDPRI[6]`) + 5000 if any adjacent country is enemy-owned (not neutral,
not own). Consequence: a country with just a capital and six empty slots is already worth
30 + 6×200 = 1230 > 1200, so in practice `FLEVEL` is 3 (heavy) for every unfortified capital
country, and the 1/2 bands are only reachable when slots are filled with low-value items
(e.g. capital + fort + uni-factory + 4 empty = 30+0+500+800 = 1330 … still 3). With the +5000 for
an enemy neighbour the band is always 3.

### 1.3 Prices / affordability used by the AI

| item | condition | effect |
|------|-----------|--------|
| capital | money > 30000 | slot[0] = 1; money -= 30000; NOBUY = 0. If a country wanted a capital but money ≤ 30000 **and NOBUY still 1** (no capital bought yet this turn): `BUYMATRIX[stage][20] = -1` → buying ends on the next HAL_BUYS (saving up for the capital). |
| factory f (1..3) | money > FACTCOST[f-1] | `FILL_BUILD_AREA(FACTFILL[f-1])` (first empty slot of COUNTRY1 gets code 3/4/6); `bays(+388) += FACTPOT[f-1]`; money -= cost |
| cyborg t (0..8) | money > MFCOST[t] **and** MFCOST[t] ≤ MAXPURCHASE | bay = `MFBT[t]*100 + t` inserted into the first empty bay of COUNTRY1; money -= MFCOST[t] |
| fort | see `_LOCATE_FORTIFICATION` | up to 3 tries, lowering FLEVEL when unaffordable |
| enhancement | money > 400000 | `FILL_BUILD_AREA(7)`; money -= 400000 |

with, per factory country (ENHANCENUM = number of enhancement slots in COUNTRY1):
```
MFCOST[t] = (long)( UNITCOST[t] * (1.0 - ENHANCENUM / 10.0) )     (x87 double math, truncated)
MFBT[t]   = max( BUILDTIME[t] - ENHANCENUM, MFBTMIN[t] )
```

### 1.4 Move priorities (`PRIMOVE`)

After `CLEAR_COUNTRY_PRIORITY`:

1. `SET_FRONT_PRIORITY` (own countries only):
   * if some adjacent country is enemy-owned: `PRIMOVE[h] = 3000`, `NOENY = 1`, `BUILD_FRONT(h, 2000)`
   * else if some adjacent is neutral: `PRIMOVE[h] = 100`, `BUILD_FRONT(h, 50)`
2. `SET_MOVE_PRIORITY` for c = 1..72 (each term skipped when c is own):
   * value: `PRIMOVE[c] = value(c) / planet_factor` (**assignment** — overwrites whatever
     BUILD_FRONT put on non-own countries)
   * owner: enemy `+4000`; neutral `+100`
   * build: `+ BUILDPRI[slot-1]` for every non-empty slot
   * units: `+ Σ FCYB[type] * (AGGRESSION ? -1 : +1)` over the cyborgs in c
3. `SET_NEUTRAL_PRIORITY` (only when NOENY): first neutral found adjacent to any own country
   (HALSCTY order, adjacency order) → `ANEUTRAL1`, `PRIMOVE = 5000`; the next *different* neutral →
   `ANEUTRAL2`, `PRIMOVE = 4990`; then stop.

So from a unit's point of view: designated neutral 5000/4990 > enemy ≈ 4000 + value/planet +
buildings ± enemy stacks > own frontier 3000 > BUILD_FRONT ripple values on own countries
(2000, 1000, 500 … on the path toward the front) > neutral ≈ 100 + value/planet > own interior 0.

### 1.5 Movement (`_HAL_MOVES` → `_CHECK_MOVEMENT` → `_BATTLE_UNITS_MOVE`)

For each own country h (HALSCTY order):

```
noenemy = FIND_ENEMY(h)          (1 if no adjacent enemy-owned country)
n       = cyborgs in h
if  slot0(h) != 1 && noenemy: STAYHOME = STAYNOCAP              (= STAY[skill][0])
elif slot0(h) == 1 && noenemy: STAYHOME = STAYCAP               (= STAY[skill][1])
else:  dest = GET_DESTINATION(h); STAYHOME = STAYENEMY = min(9, FORTFACTOR[fort(dest)] + cyborgs in dest)
if !noenemy && skill < 3:                                        (levels NASTY,2,3 only)
    if n >= STAYHOME-1:            { if STAYHOME >= 4: BLITZ = 1;  STAYHOME = 0 }
    elif NUMUNITS[dest] >= STAYHOME+2: { BLITZ = 1; STAYHOME = 0 }
movers = unit slots u (0..9) that are occupied, skipping slot 0 if h has a fort (fort(h) != 0),
         taken in slot order after the first STAYHOME such units          -> MARRAY
if movers non-empty: BATTLE_UNITS_MOVE(h)
CLEAR_MARRAY
```
`BATTLE_UNITS_MOVE(h)`: `dest = GET_DESTINATION(h)` (adjacent country with max PRIMOVE, first
wins ties, adjacency order). For each mover u:
* if BLITZ and dest is enemy-owned: keep dest (everyone piles onto the same enemy);
* else recompute `dest = GET_DESTINATION(h)`; if dest == ANEUTRAL1 or ANEUTRAL2: `PRIMOVE[dest] = 0`
  (only one unit is sent to a designated neutral);
* `PRIMOVE[dest] -= FCYB[type(u)]` (`REDUCE_COUNTRY_PRIORITY`), then `unit[u].destination(+238) = dest`.
Finally `BLITZ = 0`.

The destination may be an own country (3000 frontier / ripple values), so units migrate toward the
front, or a neutral/enemy country (= an attack; resolution is handled by the normal move code).

### 1.6 Repairs (`_HAL_REPAIRS`, runs before buying)

For every own country, every cyborg with status < 100: `ADJUST_VAULT(u, &ok, UNITCOST[type])`
→ `cost = (long)(UNITCOST[type] * REPFACTOR[status/10])`; if `vault > cost`: `vault -= cost`,
status = 100. **QUIRK:** `ADJUST_VAULT` reads the status of unit slot u of the global `COUNTRY1`
(stale: whatever the UI / previous AI player last selected), not of the country being repaired;
HAL_REPAIRS never sets COUNTRY1. Only the *price* is affected, the repaired unit is the right one.

### 1.7 How skill and planet affect decisions

| skill (player+30) | money for buying | garrison (no enemy adjacent) no-cap / cap | may empty a country / BLITZ |
|---|---|---|---|
| 0 NASTY | ×1.5 (kept) | 1 / 0 | yes |
| 1 | ×1.3 (kept) | 1 / 0 | yes |
| 2 | ×1 | 1 / 1 | yes |
| 3 | ×1 | 2 / 1 | no |
| 4 | ×1 | 2 / 2 | no |
| 5 WIMPY | ×1 | 4 / 4 | no |

Planet factor (`_MECHRECORD+4`, 1 poor / 2 average / 3 rich) only enters the AI through
`value / planet_factor` in `SET_VALUE_PRIORITY` (normalising country values back to 3..10, cities
+10/planet). Country values themselves (rand(3..10)*planet) also drive the capital choice. Game
stage (`_MECHRECORD+17646`) selects the BUYMATRIX row; the year gates the 50000 cap on MAXPURCHASE.

---------------------------------------------------------------------------------------------------

## 2. Action-phase helpers in scope (seg 0x9BA / 0x1815)

These are **not** part of HAL's strategic turn; they belong to the battle set-up screen
(`_ACTION_SETUP`) and the "showdown" flow, but were listed in scope:

* `_AG[5][2]` = {{175,0},{201,1},{227,2},{253,3},{279,4}}: slider x-coordinate → aggression 0..4.
  `_AGGLEVEL` is the slider position in pixels (init 0xE3 = 227 = aggression 2).
* `_SET_AGGRESSION(ignored)`: `ATTAGG = AG[i][1]` for the i with `AG[i][0] == AGGLEVEL`.
* `_INCREASE_ATTACKER_AGGRESS`: if `AGGLEVEL < 279`: redraw slider, `AGGLEVEL += 26`,
  `_CHANGE_AGGRESS_MESSAGE`. `_DECREASE_ATTACKER_AGGRESS`: if `AGGLEVEL > 175`: `AGGLEVEL -= 26`,
  redraw, message. `_CHANGE_AGGRESS_MESSAGE`: prints the two-line text for the 5 slider positions
  (UNCOMMITTED / LOOSELY / MODERATELY / STRONGLY / EXTREMELY COMMITTED ATTACK, ENG strings
  7767..8127) at y = 0xAA / 0xB0. Pure UI.
* `_SET_MONKEY`: clears the mission selection box, prints the default mission text block (ENG strings
  at 0x5DC1..0x5E39 − 0x3DDA = 8167.. "GUERRILLA ATTACK: …") and sets `ATTMISSION = 0`. Pure UI.
* `_DECIDE_ON_GOAL` (computer attacker, called from `_ACTION_SETUP` with `AGGLEVEL = 227`,
  `ATTGOAL = 0`):
  ```
  heavyA = count of ACTIONLIST[0..9]  with type in {0 TITAN, 1 BEHEMOTH, 4 CROSSBOLT}
  heavyB = count of ACTIONLIST[10..19] with type in {0,1,4}            (computed, never used)
  if T_NUM == 4 (defending country terrain == CITY) && heavyA >= 2: ATTAGG = 0xFD (253)
  elif heavyA > 2: ATTGOAL = 2
  ```
  `_ACTION_SETUP` then calls `_SET_AGGRESSION(1)`, which **overwrites ATTAGG with 2**, so the
  `ATTAGG = 253` branch is dead code (it looks like a pixel value was stored where a level was
  meant). Net effect: a computer attacker always fights with aggression 2, and chooses goal 2
  when it brings more than two heavy cyborgs. (Meaning of ATTGOAL values is for the action-setup
  agent; 2 is probably "structure attack".)
* `_COMPUTER_BUY` / `_TRYBUY` (seg 0x1815, called twice from `_DO_SHOWDOWN`): a different, tiny
  purchaser used by the showdown mode, not by HAL:
  ```
  COMPUTER_BUY: TEMPVAULT = BUYMONEY; k = RAND_NUM(0, 9);
                for i in 0..k-1: TRYBUY(RAND_NUM(0, 8));      (random types)
                for ; i < 10; i++: TRYBUY(0)                   (fill the rest with the best affordable)
  TRYBUY(t):    for s = t..8: if TEMPVAULT >= UNITCOST[s]: INSERT_BAY(s); TEMPVAULT -= UNITCOST[s]; return
  ```
  (`INSERT_BAY(s)` puts code s — no build time — into the first empty `_TEMPBAY` of COUNTRY1.)

---------------------------------------------------------------------------------------------------

## 3. Pseudo-C (exact)

Conventions: `C(c)` = country record c; `P(p)` = player record p; `cur = _CURPLAY`;
`skill = P(cur).skill` (+30); `adj(c)[i]` = C(c)+206+2i, 0-terminated; `slot(c)[i]` = C(c)+192+2i;
`unit(c)[u].type/.status/.dest` = C(c)+234+10u (+0/+2/+4); `bay(c)[b]` = C(c)+334+2b;
`owner(c)` = +172; `value(c)` = +386; `nbays(c)` = +388; `fort(c)` = +226; `space(c)` = +224.
`RAND_NUM(lo,hi)` inclusive.

```c
#define NEUTRAL 5
#define EMPTY_UNIT 10
#define EMPTY_BAY 10

/* ---------------- bookkeeping ---------------- */
void CLEAR_HALS_COUNTRIES(void) { for (i = 0; HALSCTY[i] != 0; i++) HALSCTY[i] = 0; }

void GET_HALS_COUNTRIES(void) {
    n = 0;
    for (c = 1; c < 73; c++) if (owner(c) == cur) HALSCTY[n++] = c;
    AGGRESSION = (HALCTYTOTAL[cur] - 2 > n) ? 0 : 1;     /* lost > 2 countries since last turn -> 0 */
    HALCTYTOTAL[cur] = n;
}

void ADD_ENEMY_UNITS(void) {
    for (c = 1; c < 73; c++) {
        NUMUNITS[c] = 0;
        if (owner(c) == cur) continue;
        for (i = 0; adj(c)[i] != 0; i++) {
            a = adj(c)[i];
            if (owner(a) != cur) continue;
            cnt = 0; for (u = 0; u < 10; u++) if (unit(a)[u].type != EMPTY_UNIT) cnt++;
            NUMUNITS[c] += cnt;
        }
    }
}

void ADD_ENEMY_FORTS(void) {
    NUMFORTS = NUMBAYS = NUMCYBORGS = 0;
    for (c = 1; c <= 72; c++) {
        if (slot(c)[0] == 1 && owner(c) != cur && fort(c) != 0) NUMFORTS++;
        if (owner(c) != cur) continue;
        for (i = 0; i < 7; i++) if (slot(c)[i] > 2 && slot(c)[i] < 7) NUMBAYS += slot(c)[i] - 2;
        for (u = 0; u < 10; u++) if (unit(c)[u].type != EMPTY_UNIT) NUMCYBORGS++;
    }
    if (NUMBAYS == 0) NUMBAYS = 1;
}

/* ---------------- priorities ---------------- */
void CLEAR_COUNTRY_PRIORITY(void) { for (c = 1; c < 73; c++) PRIMOVE[c] = 0; }

void SET_FRONT_PRIORITY(void) {
    for (i = 0; HALSCTY[i] != 0; i++) {
        h = HALSCTY[i]; enemy = 0; neutral = 0;
        for (j = 0; adj(h)[j] != 0; j++) {
            a = adj(h)[j];
            if (owner(a) != NEUTRAL && owner(a) != cur) { enemy = 1; NOENY = 1; break; }
            if (owner(a) == NEUTRAL) neutral = 1;
        }
        if (enemy)        { PRIMOVE[h] = 3000; BUILD_FRONT(h, 2000); }
        else if (neutral) { PRIMOVE[h] = 100;  BUILD_FRONT(h, 50);   }
    }
}

void BUILD_FRONT(int cty, int pri) {
    for (i = 0; adj(cty)[i] != 0; i++) {
        a = adj(cty)[i];
        if (PRIMOVE[a] < pri && (owner(a) == cur || owner(a) == NEUTRAL)) PRIMOVE[a] = pri;
    }
    d = adj(cty)[0];                                   /* QUIRK: only the FIRST neighbour is recursed */
    for (i = 0; adj(d)[i] != 0; i++) {
        b = adj(d)[i];
        if (PRIMOVE[b] < pri && (owner(b) == cur || owner(b) == NEUTRAL) && pri >= 8)
            BUILD_FRONT(d, pri / 2);                   /* repeated for every qualifying b; idempotent */
    }
}

void SET_MOVE_PRIORITY(void) {
    for (c = 1; c < 73; c++) { SET_VALUE_PRIORITY(c); SET_OWNER_PRIORITY(c); SET_BUILD_PRIORITY(c); SET_UNITS_PRIORITY(c); }
}
void SET_VALUE_PRIORITY(int c) { if (owner(c) != cur) PRIMOVE[c] = value(c) / MECHRECORD.planet; }   /* assignment */
void SET_OWNER_PRIORITY(int c) {
    if (owner(c) != cur && owner(c) != NEUTRAL) PRIMOVE[c] += 4000;
    else if (owner(c) == NEUTRAL)               PRIMOVE[c] += 100;
}
void SET_BUILD_PRIORITY(int c) {
    if (owner(c) == cur) return;
    for (i = 0; i < 7; i++) if (slot(c)[i] != 0) PRIMOVE[c] += BUILDPRI[slot(c)[i] - 1];
}
void SET_UNITS_PRIORITY(int c) {
    sign = AGGRESSION ? -1 : 1;
    if (owner(c) == cur) return;
    sum = 0;
    for (u = 0; u < 10; u++) if (unit(c)[u].type != EMPTY_UNIT) sum += FCYB[unit(c)[u].type] * sign;
    PRIMOVE[c] += sum;
}

void SET_NEUTRAL_PRIORITY(void) {
    pri = 5000;
    for (i = 0; HALSCTY[i] != 0; i++) {
        h = HALSCTY[i];
        for (j = 0; adj(h)[j] != 0; j++) {
            a = adj(h)[j];
            if (owner(a) != NEUTRAL) continue;
            if (pri <= 2000) return;
            if (pri == 5000)          { ANEUTRAL1 = a; PRIMOVE[a] = pri; pri -= 10;   }   /* -> 4990 */
            else if (a != ANEUTRAL1)  { ANEUTRAL2 = a; PRIMOVE[a] = pri; pri -= 4000; }   /* -> 990, next neutral ends it */
        }
    }
}

void SET_MOVEMENT_LEVELS(void) { STAYNOCAP = STAY[skill][0]; STAYCAP = STAY[skill][1]; }

/* ---------------- repairs ---------------- */
void HAL_REPAIRS(void) {
    for (i = 0; HALSCTY[i] != 0; i++) { h = HALSCTY[i];
        for (u = 0; u < 10; u++) {
            if (unit(h)[u].status >= 100 || unit(h)[u].type == EMPTY_UNIT) continue;
            ok = 0; ADJUST_VAULT(u, &ok, UNITCOST[unit(h)[u].type]);   /* uses COUNTRY1 for the status! */
            if (ok) unit(h)[u].status = 100;
        }
    }
}
/* for reference (module BUILD/ECONOMY): */
void ADJUST_VAULT(int u, int *ok, long cost) {
    k = unit(COUNTRY1)[u].status / 10;
    long c = (long)(cost * (double)REPFACTOR[k]);
    if (P(cur).vault > c) { *ok = 1; P(cur).vault -= c; }
}

/* ---------------- buying ---------------- */
void HALS_TURN(void) {
    long money; int item, buyflag = 1;
    for (i = 0; i < 21; i++) TMATRIX[i] = BUYMATRIX[stage][i];
    BLITZ = 0; NOBUY = 1; NOENY = 0;
    ADD_ENEMY_UNITS(); ADD_ENEMY_FORTS();
    if (NUMFORTS > 5) NUMFORTS = 5;
    BUYMATRIX[stage][8] += NUMFORTS;
    CLEAR_HALS_COUNTRIES(); GET_HALS_COUNTRIES(); CLEAR_COUNTRY_PRIORITY();
    SET_FRONT_PRIORITY(); SET_MOVE_PRIORITY();
    if (NOENY) SET_NEUTRAL_PRIORITY();
    SET_MOVEMENT_LEVELS(); HAL_REPAIRS();
    money = P(cur).vault;
    if (skill == 0)      money = (long)(money * 1.5);
    else if (skill == 1) money = (long)(money * 1.3);
    MAXPURCHASE = (long)((double)money / (double)NUMBAYS);
    if (MAXPURCHASE > 60000L && year < 2201) MAXPURCHASE = 50000L;
    if (money < 300000L) MAXPURCHASE = 40000L;
    if (NUMCYBORGS < 15) MAXPURCHASE = 27000L;
    while (buyflag && money > 0) {
        HAL_BUYS(&item, &buyflag);
        GET_AMOUNT_OF_SPACE();                       /* on stale COUNTRY1; no effect on decisions */
        if (item == 0)                      LOCATE_CAPITOL(&money);
        else if (item >= 1 && item <= 3)    LOCATE_FACTORY(item, &money);
        else if (item >= 4 && item <= 12)   LOCATE_BATTLE_UNIT(item, &money);
        else if (item >= 13 && item <= 15)  LOCATE_FORTIFICATION(item, &money);
        else if (item == 16)                LOCATE_ENHANCEMENT(&money);
        item = 100;
    }
    P(cur).vault = money;
    HAL_MOVES();
    for (i = 0; i < 21; i++) BUYMATRIX[stage][i] = TMATRIX[i];
}

void HAL_BUYS(int *item, int *buyflag) {   /* see 1.1 */
    level = BUYMATRIX[stage][20]; *buyflag = 1;
    for (i = 0; i < 17; i++)
        if (BUYMATRIX[stage][i] == level) { *item = i; BUYMATRIX[stage][i]--; break; }
    if (i >= 16) { BUYMATRIX[stage][20]--; if (BUYMATRIX[stage][20] < 1) *buyflag = 0; }
}

void LOCATE_CAPITOL(long *money) {
    cty = 0; found = 0; GET_ACCEPTING_COUNTRY(&cty, 1, &found);
    if (found && *money > 30000L) { slot(cty)[0] = 1; *money -= 30000L; NOBUY = 0; }
    else if (found && NOBUY) BUYMATRIX[stage][20] = -1;          /* stop buying, save for the capital */
}

void LOCATE_FACTORY(int f, long *money) {               /* f = 1 uni, 2 dual, 3 quadro */
    cty = 0; found = 0; GET_ACCEPTING_COUNTRY(&cty, 2, &found);
    if (!found || *money <= FACTCOST[f-1]) return;
    FILL_BUILD_AREA(FACTFILL[f-1]);                     /* on COUNTRY1 (== cty) */
    nbays(cty) += FACTPOT[f-1];
    *money -= FACTCOST[f-1];
}

void LOCATE_BATTLE_UNIT(int item, long *money) {        /* type t = item - 4 */
    cty = 0; found = 0; GET_ACCEPTING_COUNTRY(&cty, 3, &found);
    if (!found) return;
    for (b = 0; b <= 23; b++) TEMPBAY[b] = bay(COUNTRY1)[b];
    for (t = 0; t <= 8; t++) {
        MFCOST[t] = (long)((double)UNITCOST[t] * (1.0 - (double)ENHANCENUM / 10.0));
        MFBT[t] = BUILDTIME[t] - ENHANCENUM; if (MFBT[t] < MFBTMIN[t]) MFBT[t] = MFBTMIN[t];
    }
    t = item - 4;
    if (*money > MFCOST[t] && MFCOST[t] <= MAXPURCHASE) {
        INSERT_BAY(MFBT[t] * 100 + t);                  /* first TEMPBAY == 10 among nbays(COUNTRY1) */
        *money -= MFCOST[t];
    }
    for (b = 0; b <= 23; b++) bay(COUNTRY1)[b] = TEMPBAY[b];
}

void LOCATE_FORTIFICATION(int item /*unused*/, long *money) {
    cty = 0; found = 0; GET_ACCEPTING_COUNTRY(&cty, 4, &found);   /* sets FLEVEL, COUNTRY1 */
    if (!found) return;
    done = 0;
    for (try = 0; try < 3; try++) {
        if (*money > FORTCOSTS[FLEVEL-1] && !done) {      /* QUIRK: FLEVEL may be 0 -> FORTCOSTS[-1] */
            if (fort(COUNTRY1) == 0) FILL_BUILD_AREA(2);
            fort(COUNTRY1) = FLEVEL;
            *money -= FORTCOSTS[FLEVEL-1];
            done = 1;
        } else if (FLEVEL > 0 && !done) FLEVEL--;
    }
}

void LOCATE_ENHANCEMENT(long *money) {
    cty = 0; found = 0; GET_ACCEPTING_COUNTRY(&cty, 5, &found);
    if (found && *money > 400000L) { FILL_BUILD_AREA(7); *money -= 400000L; }
}

void GET_ACCEPTING_COUNTRY(int *cty, int kind, int *found) {
    switch (kind) {
    case 1: best = 0; pick = 0;
        for (i = 0; HALSCTY[i]; i++) { h = HALSCTY[i];
            if (value(h) > best && slot(h)[0] == 0) { best = value(h); pick = h; } }
        if (pick) { *found = 1; *cty = pick; }
        return;                                           /* COUNTRY1 untouched */
    case 2: CLEAR_FACTORY_PLACE(); FACINDEX = 0; last = 0;
        for (i = 0; HALSCTY[i]; i++) { h = HALSCTY[i]; COUNTRY1 = h; GET_AMOUNT_OF_SPACE();
            if (FREESPACE > 0 && slot(h)[0] == 1) { CHECK_CTY_COVER(h); *found = 1; *cty = h; last = h; } }
        COUNTRY1 = last;
        if (FACINDEX <= 0) return;                        /* factory goes to 'last' (if found) */
        *cty = FACPLACE[RAND_NUM(0, FACINDEX - 1)];
        break;
    case 3:
        for (i = 0; HALSCTY[i]; i++) { h = HALSCTY[i]; COUNTRY1 = h; GET_AMOUNT_OF_SPACE();
            if (BAYSPACE > 0) { *found = 1; *cty = h; return; } }
        return;
    case 4: best = 0;
        for (i = 0; HALSCTY[i]; i++) { h = HALSCTY[i]; COUNTRY1 = h; GET_AMOUNT_OF_SPACE(); v = 0;
            if (FREESPACE <= 0 || slot(h)[0] != 1) continue;
            CALCULATE_COUNTRY_VALUE(h, &v);
            if (v <= best) continue;
            if (unit(h)[0].type == EMPTY_UNIT) continue;   /* needs a cyborg in slot 0 */
            best = v;
            if (v > 500 && v <= 800) {
                if (fort(h) == 1 || fort(h) == 2 || fort(h) == 3) best = 0;
                else { *cty = h; *found = 1; FLEVEL = 1; }
            } else if (best > 800 && best <= 1200) {
                if (fort(h) == 2 || fort(h) == 3) best = 0;
                else { *cty = h; *found = 1; FLEVEL = 2; }
            }
            if (best > 1200) {
                if (fort(h) == 3) best = 0;
                else { *cty = h; *found = 1; FLEVEL = 3; }
            }
        }
        break;
    case 5:
        for (i = 0; HALSCTY[i]; i++) { h = HALSCTY[i]; COUNTRY1 = h; GET_AMOUNT_OF_SPACE();
            if (FREESPACE > 0 && slot(h)[0] == 1 && ENHANCENUM < 2 && nbays(h) >= 4) { *cty = h; *found = 1; } }
        break;                                            /* last qualifying wins */
    default: return;
    }
    COUNTRY1 = *cty;                                      /* 0 if nothing found (kinds 2,4,5) */
}

void CALCULATE_COUNTRY_VALUE(int c, int *v) {
    for (i = 0; i < 7; i++) *v += CTYVAL[slot(c)[i] - 1];          /* slot 0 -> CTYVAL[-1] == 200 */
    enemies = 0;
    for (j = 0; adj(c)[j]; j++) if (owner(adj(c)[j]) != NEUTRAL && owner(adj(c)[j]) != cur) enemies++;
    if (enemies > 0) *v += 5000;
}

void CHECK_CTY_COVER(int c) {
    if (FACINDEX >= 20) return;
    covered = 1;
    for (j = 0; adj(c)[j] && covered; j++) if (owner(adj(c)[j]) != cur) covered = 0;
    if (covered) FACPLACE[FACINDEX++] = c;
}
void CLEAR_FACTORY_PLACE(void) { for (i = 0; i < 20; i++) FACPLACE[i] = 0; }

/* helpers from other modules, semantics needed here */
void GET_AMOUNT_OF_SPACE(void) {   /* on COUNTRY1 */
    FREESPACE = FORTFLAG = FORTLEVEL = FACTFLAG = MECHSPACE = BAYSPACE = ENHANCENUM = FACTNUM = TONNAGE = 0;
    for (i = 0; i < space(COUNTRY1); i++) { s = slot(COUNTRY1)[i];
        if (s == 0) FREESPACE++;
        if (s == 2) { FORTFLAG = 1; FORTLEVEL = fort(COUNTRY1); }
        if (s >= 3 && s <= 6) { FACTFLAG = 1; FACTNUM++; }
        if (s == 7) ENHANCENUM++; }
    for (u = 0; u <= 9; u++) { if (unit(COUNTRY1)[u].type == EMPTY_UNIT) MECHSPACE++;
        else if (unit(COUNTRY1)[u].dest == COUNTRY1) TONNAGE += HL[unit(COUNTRY1)[u].type]; }
    for (b = 0; b < nbays(COUNTRY1); b++) if (bay(COUNTRY1)[b] == EMPTY_BAY) BAYSPACE++;
}
void FILL_BUILD_AREA(int code) { for (i = 0; i < space(COUNTRY1); i++) if (slot(COUNTRY1)[i] == 0) { slot(COUNTRY1)[i] = code; return; } }
void INSERT_BAY(int code)      { for (b = 0; b < nbays(COUNTRY1); b++) if (TEMPBAY[b] == EMPTY_BAY) { TEMPBAY[b] = code; return; } }

/* ---------------- movement ---------------- */
void HAL_MOVES(void) {
    for (i = 0; HALSCTY[i]; i++) { flag = 0;
        CHECK_MOVEMENT(&flag, HALSCTY[i]);
        if (flag) BATTLE_UNITS_MOVE(HALSCTY[i]);
        CLEAR_MARRAY(); }
}

void CHECK_MOVEMENT(int *flag, int h) {
    m = 0; cnt = 0; noenemy = 1; n = 0; dest = 0;
    FIND_ENEMY(&noenemy, h);
    if (slot(h)[0] != 1 && noenemy)      STAYHOME = STAYNOCAP;
    else if (slot(h)[0] == 1 && noenemy) STAYHOME = STAYCAP;
    else { GET_DESTINATION(&dest, h); CHECK_ENEMY_UNITS(dest); STAYHOME = STAYENEMY; }
    for (u = 0; u < 10; u++) if (unit(h)[u].type != EMPTY_UNIT) n++;
    if (!noenemy && STAYHOME - 1 <= n && skill < 3) {
        if (STAYHOME >= 4) BLITZ = 1;
        STAYHOME = 0;
    } else if (!noenemy && NUMUNITS[dest] >= STAYHOME + 2 && skill < 3) {
        BLITZ = 1; STAYHOME = 0;
    }
    for (u = 0; u < 10; u++) {
        if (fort(h) != 0 && u <= 0) continue;             /* with a fort, slot 0 never leaves */
        if (unit(h)[u].type == EMPTY_UNIT) continue;
        cnt++;
        if (cnt > STAYHOME) MARRAY[m++] = u;
    }
    if (cnt > STAYHOME) *flag = 1;
}

void FIND_ENEMY(int *noenemy, int h) {
    for (j = 0; adj(h)[j]; j++) { a = adj(h)[j];
        if (owner(a) != NEUTRAL && owner(a) != cur) { *noenemy = 0; return; } }
}

void GET_DESTINATION(int *dest, int h) {       /* argmax PRIMOVE over neighbours, first wins ties */
    best = PRIMOVE[adj(h)[0]]; *dest = adj(h)[0];
    for (j = 1; adj(h)[j]; j++) if (PRIMOVE[adj(h)[j]] > best) { best = PRIMOVE[adj(h)[j]]; *dest = adj(h)[j]; }
}

void CHECK_ENEMY_UNITS(int d) {
    n = 0; for (u = 0; u < 10; u++) if (unit(d)[u].type != EMPTY_UNIT) n++;
    STAYENEMY = FORTFACTOR[fort(d)] + n;                   /* FORTFACTOR = {0,3,6,9} */
    if (STAYENEMY > 9) STAYENEMY = 9;
}

void BATTLE_UNITS_MOVE(int h) {
    dest = 0; GET_DESTINATION(&dest, h);
    for (k = 0; MARRAY[k] != 0 || k == 0; k++) {
        if (!(BLITZ && owner(dest) != cur && owner(dest) != NEUTRAL)) {
            GET_DESTINATION(&dest, h);
            if (dest == ANEUTRAL1 || dest == ANEUTRAL2) PRIMOVE[dest] = 0;
        }
        u = MARRAY[k];
        REDUCE_COUNTRY_PRIORITY(dest, unit(h)[u].type);    /* PRIMOVE[dest] -= FCYB[type] */
        unit(h)[u].dest = dest;
    }
    BLITZ = 0;
}
void REDUCE_COUNTRY_PRIORITY(int d, int type) { PRIMOVE[d] -= FCYB[type]; }
void CLEAR_MARRAY(void) { for (i = 0; i < 11; i++) MARRAY[i] = 0; }
```
Note on the `MARRAY` loop: the terminator test is `MARRAY[k] != 0 || k == 0`, so a leading slot 0
is processed and a list such as {0, 3} works, but a mover list that *starts* with slot 0 followed by
nothing is still correct; a list where slot 0 is not the first mover can never contain 0 later.
Only called when `*flag` was set, so MARRAY is never empty here.

---------------------------------------------------------------------------------------------------

## 4. Open questions / quirks to decide on for the port

1. **`CTYVAL[-1] == 200` for empty slots** in `CALCULATE_COUNTRY_VALUE` (out-of-bounds read of
   `_BUILDPRI[6]`). It dominates the fort-level choice (nearly always heavy). Reproduce or fix?
2. **`FORTCOSTS[-1]` when `FLEVEL` has been decremented to 0** in `LOCATE_FORTIFICATION`: the
   "cost" is the long at 0x1FFD = `_BL[15]` (174) + 65536×`_CYBCHOICE` (UI state). If money > that,
   a fort slot (code 2) is placed with `fort = 0`. Reachable (FLEVEL 1 or 2 start, money ≤ 100000
   on the first try). Decide: forbid level 0 in the port.
3. **`ADJUST_VAULT` reads the unit status from `COUNTRY1`** (stale) during `HAL_REPAIRS`, so
   repair prices use the wrong country's status (same slot index). The port should use the real
   country unless bug-compatibility is required.
4. **`ANEUTRAL1/2` are never cleared**; `BATTLE_UNITS_MOVE` zeroes `PRIMOVE[dest]` when `dest`
   equals a stale designation from an earlier turn or another AI player. Minor.
5. **`BLITZ` persists** when `CHECK_MOVEMENT` sets it for a country that ends up with no movers
   (it is only reset at the end of `BATTLE_UNITS_MOVE`). Minor.
6. `BUILD_FRONT` recurses only into the first neighbour (`adj(cty)[0]`), and its values on non-own
   countries are overwritten by `SET_VALUE_PRIORITY`; effectively the ripple (2000, 1000, 500, …)
   only survives on own countries. Reproduce as-is.
7. `GET_ACCEPTING_COUNTRY(2)` places a factory in the last capital country with free space when
   no fully-surrounded country exists; `GET_ACCEPTING_COUNTRY(5)` picks the *last* qualifying
   country. Both deterministic, just surprising.
8. Floating point: `MFCOST = (long)(UNITCOST * (1 - ENHANCENUM/10))` and `money*1.3` are done in
   x87 (emulated) double/extended precision and truncated; e.g. 90000 × (1-0.2) may truncate to
   71999 depending on precision. Needs a check against the real game for ENHANCENUM = 1, 2.
9. The 1.5×/1.3× skill bonus money is written back to the vault — the computer at skill 0/1 gains
   50 %/30 % interest every turn. Confirm that is intended (it is what the code does).
10. `_DECIDE_ON_GOAL`'s `ATTAGG = 253` is overwritten by `_SET_AGGRESSION` right after; the
    meaning of `ATTGOAL = 2` (mission type) belongs to the action-setup agent.
11. How the game identifies a computer player (the `_LROWY[p] == 0x60` test in `_INIT_MODE` /
    `_CHECK_FOR_HUMANS` uses 0x54 for humans) is turn-flow scope; HAL itself only uses `_CURPLAY`
    and the skill field.
12. `_HCYB` is dead data in CYBOR.PRG; check ACTION.PRG/INTRO.PRG for a user before dropping it.
