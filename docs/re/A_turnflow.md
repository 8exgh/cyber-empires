# A. CYBOR.PRG — game/turn flow and game-state bookkeeping

Source: cybor.asm (annotated disassembly of CYBOR.PRG). Offsets quoted as `FUNC+0xNN` or absolute
segment offsets as printed in the listing (`seg=.. off=..` on each `;===` line). Immediate constants are
quoted from the instruction bytes. DGROUP globals whose address falls *inside* `_MECHRECORD` were not
annotated by the disassembler (they appear as bare `[0xCAxx]`); `_MECHRECORD` is at DS:0x85F7, size 17652
(0x44F4), so `[0xCADB]` = `_MECHRECORD+0x44E4` = +17636 etc.

Turbo C floating point is done through the Borland emulator interrupts (`int 34h..3Dh`); the disassembler
desynchronised on those sequences, so the two FP formulas below (income, stats networth) were decoded by
hand from the raw bytes (`CD 37 46 EC` = `FILD dword [bp-14h]`, `CD 38 2E xx xx` = `FSUBR qword [xx]`,
`CD 3A C9` = `FMULP`, `CD 38 0E xx xx` = `FMUL qword`, `CD 3A C1` = `FADDP`, `CD 38 36 xx xx` = `FDIV qword`).

---------------------------------------------------------------------------------------------------------
## 1. `_MECHRECORD` layout (17652 bytes = GAME?.DAT / MECHS.DAT / ACTS.DAT image)

### 1.1 Header (+0 .. +11), six shorts (byte-swapped by `_REVERSE` loop of 6 `_CINT`)

| off | type  | meaning | evidence |
|----:|-------|---------|----------|
| +0  | short | number of active (non-"none") players. `_GET_OPTIONS+0x23` sets 0; `_CHANGE_BUTTONS` inc/dec it (0x031C / 0x02F3) each time a player column changes between NONE(0x48) and HUMAN/COMPUTER, tracked with `_ADDFLAG[p]`. | `mov word [0x85F7],0` ; `inc/dec [_MECHRECORD]` |
| +2  | short | **never referenced** in CYBOR.PRG (no `[_MECHRECORD+2]` anywhere). Always 0. | grep |
| +4  | short | planet worth / "country worth" factor 1=poor 2=average(default) 3=rich. Multiplies country value. | `_GET_OPTIONS+0x1D: mov word [0x85FB],2`; `_SET_COUNTRY_WORTH+0xBE: mov [0x85FB],si`; `_GAME_INIT` `mul word [0x85FB]` |
| +6  | short | **garbage**: MAP_X of the last start position. Written by `_GET_PLAYER` on the very first call when `_CURPLAY == -1`: `player[-1].map_x` = `_MECHRECORD+38-32` = +6. (This is why saves show 96/200 here.) | `_GET_PLAYER+0x12..0x25` with `_CURPLAY=-1` set by `_GAME_INIT+0x350` |
| +8  | short | garbage: MAP_Y of the last start position (same bug). | same |
| +10 | short | current player index `_CURPLAY` (0..4) saved for reload. | `_GET_PLAYER+0x2E: mov [0x8601],ax`, `_RESTORE_DATA+0x4F: mov ax,[0x8601]; mov [_CURPLAY],ax` |

### 1.2 Player record (32 bytes each, player p at +12 + 32*p, p = 0..4)

Byte-swap pattern from `_DOPLAY`: CINT @+0, CINT @+18, LINT @+20, CINT @+24,+26,+28,+30. Name bytes +2..+17 untouched.

| off | type | meaning | evidence |
|----:|------|---------|----------|
| +0  | short | player id = p (0..4) | `_GAME_INIT+0x2A..0x46`: writes 0,1,2,3,4 at MR+12,+44,+76,+108,+140 |
| +2  | char[16] | player name, NUL terminated, max 11 chars typed (A–Z, '-', space). Human: typed in `_READ_HUMAN_NAME` (stored at `[bx-0x79FB]` = MR+14 = p*32+2). Computer: `strcpy` from ENG.DAT string 14767+25*p ("WAR HAMMER","DESTROYER","SUBJUGATOR","DEVOURER","DEATHDEALER") in `_GET_PLAYER_NAMES+0x79`. Printed everywhere as `0x8605 + 32*p`. | |
| +18 | short | turn-order slot of this player (0..4): `player[_ORDER[k]].slot = k`. | `_SET_ORDER+0x7B/+0xA3: mov [bx-0x79EB],..` (MR+30) |
| +20 | long  | vault (money). Start value 0x00127690 = **1,210,000**. Income added in `_INC_DATE`. | `_GAME_INIT+0x2BE: mov word [MR+34],0x12; mov word [MR+32],0x7690` |
| +24 | short | player type = row-y of the options radio button: **0x48 (72) = NONE / eliminated, 0x54 (84) = HUMAN, 0x60 (96) = COMPUTER (HAL)**. Runtime copy is `_LROWY[p]`. | `_GET_OPTIONS+0x200..0x216: [MR+36+32p] = _LROWY[p]`; `_RESTORE_DATA+0x37`: back-copy; `_CHECK_STATUS+0x52: mov word [MR+36],0x48` on elimination |
| +26 | short | saved map scroll X for this player | `_GET_PLAYER+0x12`, `_GAME_INIT+0x32A` (MR+38) |
| +28 | short | saved map scroll Y | `_GET_PLAYER+0x1F`, `_GAME_INIT+0x338` (MR+40) |
| +30 | short | computer skill level 0..5 (0 = "WIMPY" .. 5 = "NASTY") chosen in `_READ_HUMAN_NAME` for computer players; read by HAL (`_HALS_TURN`, `_CHECK_MOVEMENT`, `_SET_MOVEMENT_LEVELS`). Not initialised for humans. | `_READ_HUMAN_NAME+0x43C: mov [bx-0x79DF],di` (MR+42) |

### 1.3 Country record (236 = 0xEC bytes; country c (1..72) at `c*236`, fields below relative to `c*236`)

Byte-swap pattern from `_DOCOUNT` (called with base `172 + 236*i`, i = 0..73): CINT @+0 (owner), skip 16 bytes (name), 22 CINT (+18..+60), 10× `_DOMECHS` (5 CINT each), 37 CINT. I.e. everything except the name is a big-endian short on disk.

| off (rel. c*236) | meaning | evidence (this scope) |
|---:|---|---|
| +172 | owner 0..4, **5 = neutral** | `_GAME_INIT+0xB1: mov word [+172],5`; start country ← player id (`_GAME_INIT+0x24A`) |
| +174 | char[16] name (14 + 2 pad; +188 is inside this area and is never referenced) | `_DOCOUNT` skips 16 bytes |
| +190 | country number | (loaded from MECHS.DAT template; not written here) |
| +192 | government/capital established (1) | `_GAME_INIT+0x2A0: mov word [+192],1` for start countries; `_INC_DATE+0x10A` income requires ==1; `_SET_OWNERS` |
| +194..+204 | six building slots (shorts). `_GAME_INIT+0xDC` zeroes +192..+204 (loop di=0..6 over `[+192+2*di]`). `_FILL_STATS` counts slot values 1..9 into `_BUDSTATS[value]`, treating values 3..6 as "factory-class" (adds `value-2` to BUDSTATS+18). Exact codes belong to the build agent. | |
| +206..+222 | 9 shorts adjacency list (0-terminated) | `_INTERROGATE_MAP_FOG+0x4F..0xB2` walks `[+206+2*di]`, di=0..8 |
| +224 | build space: `rand(2,7)` normally, **1 for cities, 7 for start countries** | `_GAME_INIT+0x98/+0x1BE/+0x286` |
| +226 | fortification level 0..3 | `_GAME_INIT+0xC0: 0` |
| +228 | number of cyborgs present (recomputed in `_INC_DATE+0x1DA..0x1FF`) | |
| +230,+232 | icon x,y on the 640x400 strategic map | `_GET_COUNTRY_NUM` hit-test ±8/+25 |
| +234..+333 | 10 cyborg slots × 5 shorts `{type, status%, home_country, ?, ?}`; type 10 = empty. `_GAME_INIT+0x106`: type=10, home=c. `_INC_DATE` deploy: type=bay type, status=100, home=c. | |
| +334..+381 | 24 factory bays. **Encoding: `turns_left*100 + cyborg_type`; 10 = empty bay; value < 100 = finished cyborg of that type waiting for deployment.** | `_INC_DATE+0xAA..0xF1`: `if bay>=100: bay-=100`; `+0x207..+0x220`: deploy when `bay<100 && bay!=10` |
| +382 | never referenced in CYBOR.PRG | grep |
| +384 | **city damage %** (0..100). Decays by 2 per round (`_INC_DATE+0x4D..0x8E`), set by battle result (`_SET_ACTION_PHASE+0x427`, `_DAMAGE_CITY`). 100 = no income at all. | |
| +386 | country value (networth units): `rand(3,10)*worth`; cities `*worth + 10`; start countries `rand(7,10)*worth` | `_GAME_INIT+0x5C..0x8B`, `+0x174..0x1BE`, `+0x1C9` |
| +388 | factory type / bays capacity (0 at init) — build agent | `_GAME_INIT+0xD0` |
| +390 | terrain 1..5 | (template) |
| +392 | "attacked by" marker: set to the attacker's owner id when a battle is resolved (`_SET_ACTION_PHASE+0x523/+0x556`); cleared (presumably to 5/≥5) by `_CLEAR_ATTACK_FLAG`; `_FILL_MAP+0x40` plots an explosion icon if `< 5`. | |
| +394..+406 | never referenced in CYBOR.PRG (padding; still byte-swapped as shorts) | grep |

### 1.4 Trailing area (+17400 .. +17651)

`_REVERSE` applies `_DOCOUNT` 74 times (i = 0..73), so bytes 17400..17635 are swapped **as if they were a 74th country record** (they are otherwise unused), then 8 more `_CINT`:

| abs off | DS addr | meaning | evidence |
|---:|---|---|---|
| +17636 | 0xCADB | season 1..7 (names ENG.DAT 14992+15*(s-1): TRIGLAV, SAVITAR, ADITYA, VISHNU, RUDRA, THOTH, CHOLE) | `_GAME_INIT+0x4C: =1`; `_INC_DATE+0x23..0x34`; `_SHOW_STATUS+0xFC` prints `0x786A + 15*(season-1)` |
| +17638 | 0xCADD | year, starts **2200** (0x898) | `_GAME_INIT+0x50`; `_INC_DATE+0x34: inc`; `_CHECK_CRITICAL+0x28: cmp 0x898` |
| +17640 | 0xCADF | unreferenced (0) | |
| +17642 | 0xCAE1 | previous player index (`_GET_PLAYER` scratch, used for the "only one player left" test) | `_GET_PLAYER+0x5/+0x88` |
| +17644 | 0xCAE3 | main-menu mode: 1 = COMPLETE CAMPAIGN (battles fought in ACTION.PRG), 0 = STRATEGY CAMPAIGN (battles auto-simulated). Written only in `_DO_MAIN_BODY` (+0x6 and +0xE0); never read by CYBOR (the live decision uses `_SELECTION`). | |
| +17646 | 0xCAE5 | game-progress counter for HAL: 0 at init; `_SET_ACTION_PHASE+0x634..0x670`: if `<10` and season ∈ {1,3,5} → `++`; then if `>8`: 9→10, otherwise(10)→9 (so it climbs 0..9 then alternates 10,9,10,9 …). Read by `_HALS_TURN`, `_LOCATE_CAPITOL`. | |
| +17648 | 0xCAE7 | fog-of-war flag (0 normal, 1 fog) | `_SET_NORMAL+0x79: =0`, `_SET_FOG+0x5D: =1`, `_DRAW_MAP+0x9`, `_FILL_MAP` |
| +17650 | 0xCAE9 | unreferenced | |

Related runtime (not in file): `_ORDER[5]` (DS 0x7BE3) turn order; `_LROWY[5]` (0x13BA) player types; `_LASTROWY[5]`/`_TESTROWY[5]` (0xCDAE/0xCDB8) elimination detection; `_NUMPLAY` (0x8475) players still alive; `_ALIVE` (0x1150); `_CONTINUE` (0x13A5); `_CURPLAY` (0x13A7); `_DOORFLAG` (0x13AB); `_DELAYTIME` (0x13A9, default 2, 0..9 message delay in seconds); `_HUMANNUMBER` (0xCCAD); `_SOUNDS[5]` (0xD1F5, per-player battle win/loss tally during an action phase).

---------------------------------------------------------------------------------------------------------
## 2. Game loop

### 2.1 Program start / main menu
```
_MAIN:
  srand(_CLOCK2())
  _LOAD_FILE2("pinfo.iat", _PZ, 113)          // written by INTRO.PRG (or by CYBOR before exec'ing ACTION.PRG)
  _PASSFLAG = _PZ.passflag (+0); _PZ.passflag = 0
  _EGA = _PZ+86;  language = _PZ+94
  _WRITE_FILE2("pinfo.iat", _PZ, 113)         // passflag cleared on disk immediately
  _INIT_STRINGS()
  if (_PASSFLAG != 0) _LOAD_FILE2("actstuf.iat", _GAMESTUFF, 486)   // returning from ACTION.PRG
  _GAMESTUFF+461 = _PZ+90 (joystick count); _GAMESTUFF+467(byte) = _PZ+92 (sound mode)
  graphics init (_SCRBOOT(_EGA), _SETFADE(0), _SET_MFDB, _MBOOT), _LOAD_LANGUAGE(language)
  // scroll-rate calibration: count _CLOCK ticks in one tick… -> _TIMEDELAY; _SCRATE = (_TIMEDELAY/50000)*(_EGA+1), min 1   (_MAIN+0x99..+0xFA)
  _LOAD_DATA_FILE(0)                           // MECHS.DAT template -> _MECHRECORD (country names/terrain/adjacency/positions)
  _DO_MAIN_BODY()
  _SCREENDOWN(); _MDOWN(); _EXIT2()
```
```
_DO_MAIN_BODY:
  _FOREVER = 1; mode(+17644) = 0; mouse off
  if (_PASSFLAG != 4) { GIFLOAD door3.gif -> SCREEN2; GIFLOAD button10.gif -> SCREEN1 }
  if (_PASSFLAG != 0 && _PASSFLAG < 4) { _LOAD_SCORES(); goto tactical_loop }     // back from BATTLE PRACTICE
  if (_PASSFLAG == 4) goto strategy_resume                                         // back from a campaign battle
  if (_BOOT) _INTRO_S();  _SETFADE(0); _WRITEPAL(_TEMPPALLET); mouse on; goto check_forever
menu:
  _HUMANNUMBER = 0; load mainscr3.gif, fade in, (first time: _PLAY_SAMPLE(7)); _BOOT = 0
  _GET_MAIN_SELECT()      // _SELECTION: 1 = COMPLETE CAMPAIGN, 2 = STRATEGY CAMPAIGN, 3 = BATTLE PRACTICE, -1 = DOS (sets _FOREVER=0)
  _FADE_OUT()
  if (_SELECTION == 1) { mode(+17644) = 1; }               // falls through to strategy
strategy_resume:
  if (_SELECTION == 1 || 2) { _DO_STRATEGY(); reload door3.gif; goto check_forever }
  if (_SELECTION == 3) { _LOAD_SCORES(); _CONTINUE = 1; _QUICKL = 0; goto tact_check }
tactical_loop: do { _DO_ACTION(); _GET_TACTICAL_SELECTION(); } while (_CONTINUE != 0);   // tact_check enters at the while
  _QUICKL = 0; _SAVE_SCORES()
check_forever:
  if (_FOREVER) goto menu
  _MDOWN(); _SCREENDOWN(); _EXIT2()
```
`_GET_MAIN_SELECT` hit boxes (mouse x 0x6E..0xD0): y 0x16..0x4C → 1, 0x4C..0x84 → 2, 0x84..0xBA → 3; the two DOS boxes y 0x9E..0xC7, x 0..0x38 or 0x107..0x13F → -1. Plays `_SND_ON(_BUTTONSD,-1,15,-1,5)` on exit.

Note: when returning from ACTION.PRG with `_PASSFLAG==4` the code jumps straight to `_DO_STRATEGY` **without** `_GET_MAIN_SELECT`, so `_SELECTION` is restored later from PINFO (`_PZ+88`) in `_SET_ACTION_PHASE+0x358`.

### 2.2 Strategy session
```
_DO_STRATEGY:
  _FBTU = 0; mouse off; _CONTINUE = 1; _DOORFLAG = 0
  _LOAD_STRAT_SCREENS(0); _CLEAN_COPY_SCREENS(); _SET_MAP()
  if (_PASSFLAG != 0) { _LOAD_DATA_FILE(2); /* acts.dat -> _MECHRECORD */ _READMF = 0; }
  _GET_OPTIONS()                                  // options screen (or, if _PASSFLAG!=0, silent reload: see below)
  for i in 0..4: _LASTROWY[i] = _TESTROWY[i] = _LROWY[i]
  srand(_CLOCK())
  if (_PASSFLAG == 0) { _CLEAR_ATTACK_FLAG(); _LOADMAP2(); _GAME_INIT(); }   // _GAME_INIT is a no-op when _READMF (loaded game)
  if (_PASSFLAG == 0) _INIT_MODE()                // first player prompt / HAL turns
  _INIT_PARTS(); _TOGGLE_SCREENS(); _COPY_SCREENS(); mouse on
  if (_PASSFLAG == 0) { _DRAW_MAP(); _FADE_IN(); }
  _INSTRAT = 1; _CHECK_BUTTON();  _INSTRAT = 0    // <- the whole game runs inside _CHECK_BUTTON until _CONTINUE == 0
  _FADE_OUT(); _CLEAR_MAP()
```
`_CHECK_BUTTON` (not in scope, summarised): loops `while (_CONTINUE != 0)`; the door button (x 0xC4..0xDF, y 0x9F..0xB0) calls `_DOOR_ACTION`; all other operation buttons (build/move/repair/spy/… via `_OPCONTROL`) require `_DOORFLAG == 1`; the status/options button (y 0xB4..0xC6, x 0xC4..0xDF) → `_OPTIONS_STATS`; map clicks → `_CHECK_OWNERSHIP`/`_SHOW_CTRY_STUFF`/`_SHOW_STATUS`; scroll arrows → `_CHECK_SCROLL`.

### 2.3 A player's turn (the "doors")
`_DOORFLAG` = 0 means the hangar doors are closed (between turns), 1 = open (turn in progress).
```
_DOOR_ACTION:                                     // called when the door button is clicked
  if (_PASSFLAG != 0) goto next_player            // resuming after a battle: go straight on
  wait for button release; mouse off; (blit) ; _TOGGLE_SCREENS; _COPY_SCREENS
  if (_DOORFLAG == 0) {                           // START OF TURN
      if (fog) _DRAW_MAP()
      _OPEN_DOORS(); (joystick: _NEW_MOUSE)
      _DOORFLAG = 1
      _CHECK_ELIMINATION()                        // "X HAS BEEN ANNIHILATED" reports
      _CHECK_STATUS()                             // is current player alive? count players, win/lose sounds
      _CHECK_CRITICAL()                           // warning sample 1 if player owns < 6 countries
      goto end                                    // end: if _DOORFLAG==1 _SHOW_STATUS()
  }
  // _DOORFLAG == 1: END OF TURN confirmation
  print "END OF TURN." / "RE-CLICK BUTTON" / "TO CONFIRM." (ENG 3754/3778/3802); wait click
  if click in (x 0xC9..0xE0, y 0x9F..0xB0) {
      _CLOSE_DOORS(); _SAVE_MAP()
next_player:
      _DOORFLAG = 0
      _INIT_MODE()                                // advance to next player, run HAL turns
  } else _SHOW_STATUS()                           // cancelled
```
```
_INIT_MODE:
  if (_READMF == 0) _GET_PLAYER()                 // a freshly loaded game keeps its saved _CURPLAY
  _READMF = 0
  while (_LROWY[_CURPLAY] == 0x60 /*computer*/ && _CONTINUE != 0) {
      _FADE_IN(); _CHECK_STATUS()
      if (_ALIVE) { _SET_PLAYER_PROMPT(1); _TOGGLE_SCREENS(); _DELAY2(_DELAYTIME*60); _HALS_TURN(); }
      _FADE_IN(); _GET_PLAYER(); _CHECK_FOR_HUMANS()      // _CONTINUE = any human (0x54) left in _LROWY
  }
  mouse on; _SET_PLAYER_PROMPT(0)                 // human prompt "TAKE THE COMMAND CHAIR."
```
```
_GET_PLAYER:                                      // advance _CURPLAY; wraps -> end of round
  if (_PASSFLAG == 0) {
      prev(+17642) = _CURPLAY
      player[_CURPLAY].map_x/y = _MAP_X/_MAP_Y    // (bug: writes header +6/+8 when _CURPLAY == -1)
      do { _CURPLAY++; _MECHRECORD+10 = _CURPLAY; } while (_LROWY[_CURPLAY] == 0x48 && _CURPLAY < 5)
      if (_CURPLAY <= 4) goto check_end
  }
  _INC_DATE()                                     // END OF ROUND: date, battles, income, production
  _CURPLAY = 0; _MECHRECORD+10 = 0
  if (fog == 0) _DRAW_MAP()
  while (_LROWY[_CURPLAY] == 0x48 && _CURPLAY < 5) { _CURPLAY++; _MECHRECORD+10 = _CURPLAY; }
check_end:
  if (prev == _CURPLAY) _CONTINUE = 0             // only one player left -> game over
```
Turn order within a round is therefore plain **player index 0..4** skipping NONE/eliminated players; `_ORDER` from `_SET_ORDER` is only used at game creation to pick starting locations (and recorded in player +18).

### 2.4 End of round: `_INC_DATE` (seg 0x13E7 off 0x82F)
```
_INC_DATE:
  if (_PASSFLAG == 0) {                           // not when resuming after a battle
      season(+17636)++
      if (season > 7) { season = 1; year(+17638)++ }
  }
  _SET_ACTION_PHASE()                             // BATTLE PHASE (see 2.5). May exec ACTION.PRG and never return.
  for c = 1..72:
      if (dmg(+384) > 0) { dmg -= 2; if (dmg < 0) dmg = 0; }          // _INC_DATE+0x4D..0x8E  (city repair)
      if (owner(+172) == 5) continue                                   // neutral
      for b = 0..23: if (bay[b] >= 100) bay[b] -= 100;                 // production countdown (one turn)
      if (gov(+192) == 1 && dmg != 100)                                // INCOME
          vault[owner] += (long)( (double)value(+386) * (100.0 - (double)dmg) * 1000.0 );
                                                   // constants: 100.0 @DS:0x285D, 1000.0 @DS:0x2865; FTOL@ truncates
                                                   // i.e. value * 100,000 per round at 0% damage
      count = #slots with type != 10;  cyborgs(+228) = count
      // DEPLOY finished cyborgs
      for b = 0..23 while count < 10:
          if (bay[b] < 100 && bay[b] != 10)                            // finished (bay holds just the type 0..8)
              s = first slot with type == 10:  slot.type = bay[b]; slot.status(+236) = 100; slot.home(+238) = c;
              bay[b] = 10; count++; cyborgs(+228) = count
```
Locals: `[bp-8]=0x3C` is set but unused. Note cyborgs count (+228) is recomputed before deployment, so stale counts are corrected every round. There is no repair of cyborgs here (repairs are a player operation, other agent).

### 2.5 Battle (action) phase — `_SET_ACTION_PHASE` (other agent; only the flow that matters here)
* Called once per round from `_INC_DATE`, **before** income/production.
* `_PASSFLAG == 0`: `_LASTROWY[i] = _TESTROWY[i]` (+0x5A); clears `_SOUNDS`; builds the list of battles from unit moves. For each battle: if `_SELECTION == 2` (STRATEGY CAMPAIGN) or HAL-vs-HAL etc. → `_BATTLE_SIMULATION`; if `_SELECTION == 1` and a human is involved → `_ACTION_SETUP` then `_START` which writes `acts.DAT` (`_SAVE_DATA_FILE2`), `pinfo.iat` (passflag = 4 or the tactical selection), `actstuf.iat`, `MATRIX.DAT` and **exits the program** (ACTION.PRG is run by the launcher; CYBOR.PRG restarts with `_PASSFLAG == 4`).
* `_PASSFLAG != 0` (+0x32D): loads `pass2.dat` (1155 bytes → `_ALIST`), `_START2` (reads `actstuf.iat` result, sets `_PASSFLAG = 0`, `_VICTOR`, retreats), restores `_DELAYTIME` from `_PZ+96`, `_SELECTION` from `_PZ+88`, loop state from `_PZ+44..62`, `_CPCOUNT1..3` from `_PZ+64..68`, `_FADEFLAG` `_PZ+70`, `_LTERRAIN` `_PZ+72`; city damage: if terrain==5(city) new `dmg = f(pieces destroyed)`, clamped to 100; then resolves victor exactly like a simulated battle and continues with the remaining battles.
* Victory bookkeeping: attacker wins (`_VICTOR==2`): `_SOUNDS[def]--`, `_SOUNDS[att]++`, `owner(def) = owner(att)`, `+392(def) = owner(att)`; defender wins: `+392(def) = owner(att)`, `_SOUNDS[att]--`.
* End: `_RETURN_STRANDED`, `_SET_OWNERS` (see pseudo-C), the +17646 progress counter update, `_CHECK_PLAYERS` (recompute `_TESTROWY`).

### 2.6 Elimination / victory rules
* `_CHECK_PLAYERS` (end of every action phase): `_TESTROWY[i] = 0x48`; for every country with owner < 5: `_TESTROWY[owner] = _LROWY[owner]`. So `_TESTROWY[i]` = player i's type if he owns ≥ 1 country, else 0x48.
* `_CHECK_ELIMINATION` (start of each human turn, door opening): for each i ≠ `_CURPLAY` with `_TESTROWY[i] != _LASTROWY[i]` → `_DISPLAY_DEAD(i)` ("<name> HAS BEEN ANNIHILATED", intelligence report, wait for OK click at x 0xBF..0xD0, y 0x1A..0x2D). Because `_LASTROWY` is refreshed at the start of every action phase, each annihilation is reported once (to the first human whose door opens after it).
* `_CHECK_STATUS` (door opening and before each HAL turn): `_ALIVE = (∃ country with owner == _CURPLAY)`; if not alive → `_LROWY[_CURPLAY] = 0x48` and `player.type(+24) = 0x48` (**the player is eliminated: he is skipped by `_GET_PLAYER` from now on**). `_NUMPLAY = #players with _LROWY != 0x48`. Sound: if `_SOUNDS[cur] > 0 && human` → `_PLAY_SAMPLE(6)` (won a battle), `< 0 && human` → `_PLAY_SAMPLE(5)`, then `_DELAY2(60)`.
* `_SHOW_STATUS` (status panel, every redraw while doors open): if `!_ALIVE && _CRUSHNOTPLAYED` → prints "YOU HAVE BEEN / CRUSHED" (ENG 4510/4534), plays two samples once. If `_NUMPLAY == 1 && _ALIVE && _CONGRATSPLAYED` → prints "CONGRATULATIONS / HUMAN YOU ARE / SUPREME" (4558/4582/4606), sample, delay, shows **OPENFII2.gif** (victory picture) for a delay, then restores the frame (framebt3.gif + button10.gif) and the map. (`_CRUSHNOTPLAYED`/`_CONGRATSPLAYED` are set to 1 in `_CHECK_STATUS` and cleared after playing, so each message shows once per door-opening.)
* Game end (`_CONTINUE = 0`): (a) `_GET_PLAYER`: after skipping eliminated players the next player equals the previous one (one player left); (b) `_CHECK_FOR_HUMANS` after a HAL turn finds no human with `_LROWY == 0x54`; (c) `_OPTIONS_STATS` "QUIT GAME" confirmed. `_CHECK_BUTTON` then returns, `_DO_STRATEGY` fades out and `_DO_MAIN_BODY` goes back to the main menu. **No score is written for the strategy game** (SCORES.DAT is only used by BATTLE PRACTICE).
* `_CHECK_CRITICAL`: counts countries 1..71 (bug: `cmp si,0x48; jl` excludes country 72) owned by `_CURPLAY`; if `< 6 && year > 2200 && _ALIVE && _NUMPLAY > 1` → `_PLAY_SAMPLE(1)` (warning only, no game effect).

---------------------------------------------------------------------------------------------------------
## 3. Pseudo-C per function

Conventions: `MR` = `_MECHRECORD`; `C[c].f` = short at `MR + c*236 + f` (f as in §1.3); `P[p].f` = `MR + 12 + 32*p + f`; `rand(lo,hi)` = `_RAND_NUM(lo,hi)` inclusive (pushed hi then lo).

### _MAIN / _DO_MAIN_BODY / _GET_MAIN_SELECT — see §2.1.

### _DO_STRATEGY — see §2.2.

### _GAME_INIT (seg 0x13E7 off 0xB, 860 bytes)
```c
void GAME_INIT(void) {
  if (_READMF) { _CURPLAY = -1; return; }           // loaded game: keep file contents   (+0x1E jmp 0x361)
  P[0..4].id = 0..4;                                 // MR+12,+44,+76,+108,+140
  season = 1; progress(+17646) = 0; year = 2200;     // [0xCADB]=1 [0xCAE5]=0 [0xCADD]=0x898
  for (c = 1; c <= 72; c++) {
    C[c].value(386) = rand(3,10) * worth(MR+4);
    C[c].space(224) = rand(2,7);
    C[c].owner(172) = 5;  C[c].fort(226) = 0;  C[c].f388 = 0;  C[c].dmg(384) = 0;
    for (i = 0; i <= 6; i++) C[c].short[192 + 2*i] = 0;         // gov flag + 6 building slots
    for (i = 0; i <= 9; i++) { C[c].slot[i].type(234) = 10; C[c].slot[i].home(238) = c; }
    for (i = 0; i <= 23; i++) C[c].bay[i](334) = 10;
    C[c].cyborgs(228) = 0;
  }
  for (i = 0; i <= 12; i++) {                        // _CITYVALUE[13] = {2,9,10,14,20,27,32,35,39,43,45,51,72}
    c = _CITYVALUE[i];
    C[c].value = C[c].value * worth + 10;            // NOTE: value already includes worth once -> cities get worth^2 * r + 10
    C[c].space = 1;
  }
  startValue = rand(7,10) * worth;  startSpace = 7;  // [bp-0xA], [bp-8]
  _SET_ORDER();
  for (k = 0; k <= 4; k++) {
    p = _ORDER[k];
    if (_LROWY[p] == 0x48) continue;                 // player not in game
    r = rand(0,2);
    c = _STARTINGLOCATION[k*3 + r];                  // rows: {1,4,7},{41,30,36},{47,50,55},{18,22,28},{65,61,66}
    C[c].owner = p;  C[c].value = startValue;  C[c].space = startSpace;  C[c].gov(192) = 1;
    P[p].vault = 1210000L;                           // 0x00127690
    _CURPLAY = p;
    for (j = 0; j < 15; j++) if (_SCTRY[j] == c) { _MAP_X = _SMAPX[j]; _MAP_Y = _SMAPY[j]; }
    P[p].map_x = _MAP_X;  P[p].map_y = _MAP_Y;
    _I_MAP(); _SAVE_MAP();
  }
  _CURPLAY = -1;
}
```
Note: the start-location row is indexed by **order slot k** (0..4), not by player id; `startValue` is the same for all players (rolled once).

### _SET_ORDER (0x13E7:0xBBD)
```c
void SET_ORDER(void) {                 // random permutation by "largest random draw first"
  for (i = 0; i <= 4; i++) _RANRESULT[i] = rand(1,100);
  for (k = 0, left = 5; left > 0; left--, k++) {
    best = 0; hi = 0;
    for (i = 0; i <= 4; i++) if (_RANRESULT[i] > hi) { hi = _RANRESULT[i]; best = i; }   // ties -> lower index
    _RANRESULT[best] = 0;
    _ORDER[k] = best;  P[best].slot(+18) = k;
  }
  for (k = 0; k <= 4; k++) P[_ORDER[k]].slot(+18) = k;   // repeated (harmless)
}
```
(If all remaining draws are 0 — impossible since rand ≥ 1 — `best` would stay 0.)

### _INC_DATE — see §2.4.

### _RESTORE_DATA (0x13E7:0xB66) — after loading a saved game
```c
for (p = 0; p <= 4; p++) { _MAP_X = P[p].map_x; _MAP_Y = P[p].map_y; _CURPLAY = p; _I_MAP(); _SAVE_MAP(); }
for (p = 0; p <= 4; p++) _LROWY[p] = P[p].type(+24);
_CURPLAY = MR+10;
```

### _GET_PLAYER — see §2.3.

### _CHECK_ELIMINATION / _CHECK_PLAYERS / _CHECK_STATUS / _CHECK_CRITICAL — see §2.6.

### _DISPLAY_DEAD(p) (0x76D:0x20F6) — UI: blits report frame, prints "INTELLIGENCE"/"      REPORT" (DS strings 0x26A7/0x26B4), the dead player's flag (`p<4`: from LOGSCREEN at y = p*32+0xB1; `p==4`: fixed box), name (`0x8605+32p`) and "HAS BEEN ANNIHILATED" (DS 0x26C1); waits for a click in (0xBF..0xD0, 0x1A..0x2D); restores screen, `_DELAY2(10)`, `_DMAP()`. No state change.

### _GET_OPTIONS (0x16DB:0x4) — the options screen
```c
void GET_OPTIONS(void) {
  loadedGame = 0;
  if (_PASSFLAG != 0) {                              // resuming after ACTION.PRG
     _LOAD_DATA_FILE(2); _RESTORE_DATA(); _EXIT3 = 1; loadedGame = 1; goto count;
  }
  _EXIT3 = 0; worth(MR+4) = 2; MR+0 = 0; fog = 0;
  for (p..) _LROWY[p] = 0x48;
  _CLEAR_LOG(); _TOGGLE_SCREENS(); _BUILD_MENU2(); _FADE_IN(); mouse on;
  do {
    _GET_COORDINATES();
    // player type columns (x ranges): 0:0x4C..0x5D 1:0x79..0x8A 2:0xA6..0xB7 3:0xD3..0xE4 4:0x100..0x111
    if (x in column p) _CHECK_Y(p, colx);
    if (x 0x1B..0x41 && y 0x95..0xAF) _TRY_EXIT();               // "GO" button
    if (y 0xA8..0xB0) { x 0x4C..0x7F -> _SET_COUNTRY_WORTH(1); 0x8B..0xC3 -> (2); 0xC8..0x100 -> (3); }
    if (y 0x7D..0x8B) { x 0x3D..0x85 -> _SET_NORMAL(); x 0xC1..0x109 -> _SET_FOG(); }
    if (x 0x114..0x11B && y 0x9B..0xB3) { _MENU_1_LOAD(); if (_EXIT3) loadedGame = 1; }
  } while (_EXIT3 == 0);
  _FADE_OUT();
  for (p..) P[p].type(+24) = _LROWY[p];
  if (!loadedGame) { mouse off; _CLEAR_LOG(); _GET_PLAYER_NAMES(); }
  _COPY_SCREENS();
count:
  for (p..) if (_LROWY[p] == 0x54) _HUMANNUMBER++;
}
```
### _CHECK_Y(col, colx): y 0x48..0x50 → `_CHANGE_BUTTONS(colx,0x48,col)`; 0x54..0x5C → (…,0x54,…); 0x60..0x68 → (…,0x60,…).
### _CHANGE_BUTTONS(x, rowy, p): UI blit of the radio button; `_LROWY[p] = rowy`; `_SND_ON(_BUTTONSD…)`; maintains `MR+0` (player count) via `_ADDFLAG[p]` (NONE→other: ++, other→NONE: --).
### _RESET_TYPE: UI only — redraws the un-highlighted worth button for current `MR+4` (1/2/3).
### _SET_COUNTRY_WORTH(w): `_RESET_TYPE()`; highlight w; `MR+4 = w`.
### _SET_NORMAL: UI; `fog(+17648) = 0`.   ### _SET_FOG: UI; `fog = 1`.
### _TRY_EXIT: `sum = Σ _LROWY[0..4]`; if `sum >= 0x180` (= 384, i.e. at least **two** players are non-NONE, 5*72=360 … 360+12+12=384) **and** at least one is HUMAN (0x54) → button blit, `_DELAY2(10)`, sound, `_EXIT3 = 1`. Otherwise nothing.
### _GET_PLAYER_NAMES: blits name-entry frame; for p=0..4: HUMAN → `_READ_HUMAN_NAME(p, 0x54, 0)`; COMPUTER → `strcpy(P[p].name, ENG[14767+25p])` then `_READ_HUMAN_NAME(p, 0x60, 0)`.
### _READ_HUMAN_NAME(p, type, flag) (0x16DB:0x787)
```c
  draw flag of p (same geometry as _DISPLAY_DEAD)
  if (type == 0x54) {                              // human: type name
     print "ENTER YOUR NAME" (ENG 7397); (flag? "CONGRATULATIONS YOU HAVE ENTERED"/"THE HALL…" : ENG[14642+25p] "<X> GENERAL:")
     n = 0; done = 0;
     while (!done) {
        ch = _CHAR_IN() -> _CHARVAL
        if (ch == 13 && n > 0) { name[n] = 0; done = 1; }
        else if (n < 11) {
           if (ch > 'Z') ch -= 0x20;                                   // upper-case
           if ('A' <= ch <= 'Z') { echo; name[n++] = ch; }
           else if (ch == '-') { echo '-'; name[n++] = '-'; }
           else if (ch == ' ') { name[n++] = ' '; }                    // not echoed
        }
        if (ch == 8 && 0 < n <= 12) { erase; name[n-1] = ' '; n--; }   // backspace (writes to MR+13+p*32+n = name[n-1])
     }
  } else {                                         // computer: choose skill
     print "CHOOSE COMPUTER"/"SKILL LEVEL..." (7497/7517), ENG[14642+25p], P[p].name, "NASTY"(7537) left, "WIMPY"(7557) right
     draw 6 buttons at x = 0x70 + 16*i (i=0..5), y 0x68..0x71, labelled "%d" i+1
     wait for click: if 0x68 < _YCORD < 0x71 and _XCORD in (bx, bx+9) for button i -> P[p].skill(+30) = i   (0 = NASTY .. 5 = WIMPY by position)
  }
  _FADE_OUT(); _DELAY2(4);
```
(Which end is "NASTY" vs "WIMPY" is only visual; HAL agent must map skill 0..5.)

### _SET_PLAYER_TYPE (0x9BA:0x2AA): `_CHOICE` = 0 if both attacker-country owner and defender-country owner are non-computer, 1 if attacker computer & defender not, 2 if defender computer & attacker not, 3 if both computer (uses `_LROWY[owner(_ATTACKCTY)]`, `_LROWY[owner(_DEFENDCTY)]` == 0x60).
### _CHECK_FOR_HUMANS: `_CONTINUE = (∃p: _LROWY[p] == 0x54)`.
### _SET_PLAYER_PROMPT(isHal): blits current player's flag; prints "GENERAL" (ENG 3826) and the player name; isHal: "PLOTS YOUR"/"DESTRUCTION" (3850/3874); human: "TAKE THE"/"COMMAND"/"CHAIR." (3898/3922/3946). UI only.
### _SHOW_STATUS (0x76D:0x1D96): if `_DOORFLAG == 0` return. Prints "VAULT:" `%ld` vault, "YEAR:" `%d`, "SEASON:" name `ENG[14992+15*(season-1)]`, "GENERAL:" name, flag; then the crushed/victory logic of §2.6. Side effects: clears `_CRUSHNOTPLAYED`/`_CONGRATSPLAYED`, plays samples, loads OPENFII2.gif on victory. No game-state change.
### _OPTIONS_STATS (0x76D:0xE96): stats/options screen: `_BUILD_STATS_MENU`, `_FILL_STATS`; loop: (0x118..0x12F, 5..0x14) → back to map; (0x75..0xB3, 0xA6..0xB1) → `_SAVE_DATA_FILE()` then `_DRAW_OPTIONS`; (0x75..0xB3, 0xB2..0xBD) → "RE-CLICK BUTTON / TO CONFIRM" (4378/4402), second click same box → `_DELAY2(_DELAYTIME*60)`, **`_CONTINUE = 0` (quit game)**; (0xB6..0xC4, 0xA6..0xB0) → `_DELAYTIME--` (min 0); (0xD5..0xE3, …) → `_DELAYTIME++` (max 9).
### _FILL_STATS: for every country owned by `_CURPLAY`: `_BUDSTATS[0]++` (countries); for each slot type != 10: `_CYBSTATS[type]++`, `_CYBSTATS[9]++` (total); if `gov == 1`: if `dmg != 100` `networth += (long)( value * (100.0 - dmg) / 100.0 * 1000.0 )` (FP sequence FILD/FILD/FSUBR [0x268A]/FDIV [0x268A]/FMULP/FMUL [0x2692]/FADDP decoded by hand; doubles at DS:0x268A = 100.0 and DS:0x2692 = 1000.0 in table `_YOF`; see open question 1), `_BUDSTATS[1]++`; for slots 1..6 of +192: if v != 0 `_BUDSTATS[v]++`, if 3 ≤ v ≤ 6 `_BUDSTATS[8]++` and `_BUDSTATS[9] += v-2`. Prints the counts (x = 0x25 + 7*i), networth `%ld`, vault `%ld`, BUDSTATS 0,1,2,7,8,9 (x = 0x33 + 7*k).
### _CLEAR_STATS: zero `_CYBSTATS[0..9]`, `_BUDSTATS[0..9]`.
### _SET_OWNERS (0x1508:0xC63), end of action phase:
```c
for (c = 1; c <= 72; c++) {
  if (C[c].gov(192) == 1) continue;
  n = #slots with type != 10;
  if (n == 0) { C[c].owner = 5; _CLEAR_STRUCT(c); }     // ungoverned, empty country reverts to neutral
}
```
### _INIT_MODE — see §2.3.   ### _CHECK_CRITICAL — §2.6.

### File functions (seg 0x4B2)
```c
int  _CINT(short *p)  { swap bytes of *p; }
void _LINT(long *p)   { reverse the 4 bytes of *p; }
void _DOMECHS(short *s) { 5 × _CINT }                       // one cyborg slot
void _DOCOUNT(char *rec) { _CINT(rec+0); for i<22: _CINT(rec+18+2i); for i<10: _DOMECHS(rec+62+10i); for i<37: _CINT(rec+112+2i); }
void _DOPLAY(char *rec)  { _CINT(rec+0); _CINT(rec+18); _LINT(rec+20); for i<4: _CINT(rec+24+2i); }
void _REVERSE(char *mr)  { for i<6: _CINT(mr+2i); for p<5: _DOPLAY(mr+12+32p); for i<74: _DOCOUNT(mr+172+236i); for i<8: _CINT(mr+17636+2i); }
void _LOAD_FILE2(name, buf, len)  { fd = _AOPEN(name, 0x8004 /*O_RDONLY|O_BINARY*/); if (fd == -1) { printf("File Error, Could not open %s\n\n"); exit; } read(fd, buf, len); close(fd); }
void _WRITE_FILE2(name, buf, len) { same with write }
void _CREATE_DATA_FILE(name)   { fd = _AOPEN(name, 0x100 /*O_CREAT*/); close(fd); }
void _CREATE_SCORES_FILE()     { same for "SCORES.DAT" }
void _LOAD_DATA_FILE(mode) {
  retry: _FILENUM = -1; first = 1;
  do {
    if (mode == 1) { _SELECT_LOAD(); if (_EXIT3 != 1) { fade out; _CLEAR_LOG; _TOGGLE_SCREENS; _BUILD_MENU2; fade in; mouse on; }   // cancelled -> back to options
                     else _REVERSE(MR);  _FILENUM = 1; }
    if (mode == 0) _FILENUM = _AOPEN("MECHS.DAT", 0x8004);
    if (mode == 2) _FILENUM = _AOPEN("acts.dat", 0x8004);
    if (_FILENUM < 0 && first) { _NO_DISK(); first = 0; }
  } while (_FILENUM < 0);
  if (mode != 1) { read(_FILENUM, MR, 0x44F4); close; _READMF = 1; _REVERSE(MR); }
}
void _SELECT_LOAD() {   // load box with 6 buttons (_LOADB rects, 0 = CANCEL, 1..5 = GAME 1..5)
  loop until a button is hit: i==0 -> cancel (leave _EXIT3 as is);
  else { sprintf(name,"GAME%d.DAT",i); fd=_AOPEN(name,0x8004); if (fd<0) print "SAVED GAME NOT ON DISK", delay, continue;
         read(fd, MR, 0x44F4); close; _READMF = 1; _EXIT3 = 1; }
}
void _MENU_1_LOAD() { _LOAD_DATA_FILE(1); if (_EXIT3) _RESTORE_DATA(); }
void _SAVE_DATA_FILE() {  // save box with 6 buttons (_SAVEB rects): 0 = cancel, 1..5
  i chosen -> sprintf("GAME%d.DAT"); fd = open(name, 0x8004); if (fd < 0) { _CREATE_DATA_FILE(name); fd = open(name, 1 /*O_WRONLY*/); }
  _REVERSE(MR); write(fd, MR, 0x44F4); _REVERSE(MR); close(fd); print "GAME HAS BEEN SAVED" (ENG 4330); _DELAY2(_DELAYTIME*60);
}
void _SAVE_DATA_FILE2() { fd = _AOPEN("acts.DAT", 0x8004); _REVERSE(MR); write(fd, MR, 0x44F4); _REVERSE(MR); close(fd); }  // (opens read-only! see open questions)
void _LOAD_SCORES() { fd=_AOPEN("SCORES.DAT",0x8004); if (fd<0) { _NO_DISK(); return; } read(_PSCORE,72); read(_PNAME,135); read(_PLEVEL,72); close; }
void _SAVE_SCORES() { fd=_AOPEN("SCORES.DAT",0x8004); if (fd<0) { _CREATE_SCORES_FILE(); fd=_AOPEN("SCORES.DAT",1); } write(_PSCORE,72); write(_PNAME,135); write(_PLEVEL,72); close; }
```
### _INIT_STRINGS: copies the 9 cyborg GIF names ("TITAN.gif","BEHEMOTH.gif","HERCULES.gif","ACHILLES.gif","CROSBOLT.gif","MARS.gif","DRAGON.gif","CYCLOPS.gif","MERCURY.gif") into `_CYBFILES` (15 bytes each).
### _LOAD_LANGUAGE(n): opens ENG.DAT / FRE.DAT / GER.DAT / ITA.DAT (n=0..3), reads 15413 bytes into `_LANGS` (DS 0x3DDA). All ENG offsets in this document are relative to `_LANGS`.
### _GET_FREEMEM: empty (`retf`).
### _LOAD_STRAT_SCREENS(x): if `_PASSFLAG == 0`: load openfii.gif → SCREEN1, write palette, page-move, fade in, `_LOADMAP()`; always: `_SETFADE(0)`, `_WRITEPAL`, `_LOADSTRAT = 0`.
### _INTERROGATE_MAP_REG: `_MAPFILLARRAY[1..72] = 5`; for each country with owner != 5: `_MAPFILLARRAY[c] = owner`.
### _INTERROGATE_MAP_FOG: `_MAPFILLARRAY[1..72] = 5`; for each country owned by `_CURPLAY`: `_MAPFILLARRAY[c] = P[_CURPLAY].id`; for each adjacent country a (9 entries) with owner != 5: `_MAPFILLARRAY[a] = P[owner(a)].id` (i.e. only own + neighbouring countries are revealed).
### _FILL_MAP (0x13E7:0x612): for c = 1..72 with `_MAPFILLARRAY[c] != 5`: if (`!fog` or owner==cur) and `C[c].+392 < 5` → `_PLOT_EXPLODE(+392, x+16, y)`; if `gov == 1`: if `_FACTORY_PRESENT(c)` (and visible) `_PLOT_FACTORY(fill, x, y, SCREEN3, c, 0)` else `_PLOT_FLAG(fill, x, y, SCREEN3, 0)`; if `_FORT_PRESENT(c)` (and visible) `_PLOT_FORT_ICON(x+19, y+4, SCREEN3, 0)`; if `_MECHS_PRESENT(c)` (and visible) `_PLOT_CYB(fill, x-14, y+6)`. ("visible" = `!fog || owner == _CURPLAY`.)
### _DRAW_MAP: `_TLOADMAP(); _CLEAR_MAP(); fog ? _INTERROGATE_MAP_FOG() : _INTERROGATE_MAP_REG(); _FILL_MAP();`
### _CHECK_SCROLL: while mouse button held on an arrow (`_DOORFLAG` must be 1): right arrow (x 8..0x1F, y 0xAB..0xB9) → `_SCROLL(1)`; left (0x35..0x4C) → `_SCROLL(0)`; up (0x22..0x32, 0x9E..0xB0) → `_SCROLL(2)`; down (0x22..0x32, 0xB3..0xC5) → `_SCROLL(3)`. Then restores the arrow graphic.
### _SCROLL(dir): step = `_SCRATE`; dir 1: `_MAP_X += step` (max 0x15E = 350); dir 0: `-=` (min 0); dir 3: `_MAP_Y += step` (max 0x10E = 270); dir 2: `-=` (min 0); `_DMAP()`. (Map view is 0x11F+1 = 288 wide, 0x50 = 80 tall; full map 640x350 ⇒ x ≤ 350, y ≤ 270.)
### _DMAP: blits the visible map window (`_MAP_X.._MAP_X+0x11F`, `_MAP_Y.._MAP_Y+0x4F`) from SCREEN3 to the display, plus the 0x50..0x7D strip. UI only.
### _GET_COUNTRY_NUM: `_CTRYNUM = 0; _OWNER = 0;` for c = 1..72: if `C[c].x-8 <= _COORDX <= C[c].x+25 && C[c].y-8 <= _COORDY <= C[c].y+25` → `_OWNER = 1; _CTRYNUM = c;` sound. If none and `_YCORD < 0x8E` → `_MESSAGE1()`.
### _CHECK_OWNERSHIP: `_OWNER = (C[_CTRYNUM].owner == _CURPLAY)`.
### _MESSAGE1: prints "CLICK IN THE CENTER / OF THE COUNTRY. / SELECTION CANCELLED. / SELECT NEW OPERATION" (ENG 5155/5179/5203/5227), `_SND_ON(_ERROR…)`, delay `_DELAYTIME*60`.
### _MESSAGE2: "YOU CAN ONLY MOVE TO / ADJACENT COUNTRIES. / SELECTION CANCELLED. / SELECT NEW OPERTION" (5251/5275/5299/5323), same.

---------------------------------------------------------------------------------------------------------
## 4. On-disk formats

### GAME1..5.DAT, MECHS.DAT (template), acts.dat / acts.DAT (hand-off to ACTION.PRG)
Exactly the 17652-byte `_MECHRECORD` image, written after `_REVERSE` (so **big-endian** shorts/longs) and
re-reversed in memory afterwards. Swapped fields: header 6 shorts; per player: id(+0), slot(+18), vault
long(+20), +24,+26,+28,+30 (name bytes raw); per country (74 records from byte 172, the last one being the
unused 17400..17635 block): owner, then every short from +190 to +406 (name bytes +174..+189 raw); then
8 shorts 17636..17651 (season, year, -, prev player, mode, progress, fog, -). Country record order is country
number 1..72 (record 0 = bytes 172..407 = unused "country 0" area; its bytes 172..235 overlap nothing else).
MECHS.DAT supplies names (+174), number (+190), adjacency (+206), x/y (+230/232), terrain (+390) — all other
fields are reset by `_GAME_INIT`.

### SCORES.DAT (BATTLE PRACTICE hall of fame; 279 bytes, **not byte-swapped**)
`_PSCORE` 72 bytes (9 entries × 8, indexed `<<3` in tactical code) + `_PNAME` 135 bytes (9 × 15 chars) +
`_PLEVEL` 72 bytes. Created empty if missing. Contents are produced by `_CHOOSE_CYBORG`/tactical code (other agent).

### PINFO.IAT (113 bytes = `_PZ`, native little-endian; written by INTRO.PRG, rewritten by CYBOR)
| off | written by | meaning |
|---:|---|---|
| +0 short | INTRO: 0; CYBOR `_START`: 4 (campaign battle) or `_TACT_SELECTION` (1..3, battle practice modes); CYBOR `_MAIN` clears to 0 after reading | passflag |
| +2 char[19] | CYBOR `_START` ← `_ANAME` | attacker name |
| +21 char[19] | `_START` ← `_DNAME` | defender name |
| +40 short | `_START` ← `_T_NUM` | terrain/map number |
| +42 short | `_START` ← `_CHOICE` (who is computer: see `_SET_PLAYER_TYPE`) | |
| +44..+62 10 shorts | `_SET_ACTION_PHASE` loop state (restored into locals on resume) | |
| +64,+66,+68 | `_CPCOUNT1..3` (city pieces counts for city damage) | |
| +70 | `_FADEFLAG` | |
| +72 | `_LTERRAIN` (terrain of battle; 5 = city) | |
| +74 short | `_LOADSTRAT` (byte sign-extended) | |
| +76 | `_KILLTONS` | |
| +78 | `_UPBOUND` ; +80 `_LOWBOUND` ; +82 `_GAMECOUNT` ; +84 `_CLASS` | tactical settings |
| +86 short | INTRO: `_EGA` (0 VGA / 1 EGA, INTRO forces 0 at +0xB3) | video mode |
| +88 short | `_SET_ACTION_PHASE` ← `_SELECTION` (1/2) | restored on resume |
| +90 short | INTRO: number of joysticks 0..2 ('0'..'2' key) | |
| +92 short | INTRO: sound mode = key '0'..'3' − 0x31 → -1 (none), 0, 1, 2 (used by `_SND_ON`, `_SET_MFDB`, door sounds) | |
| +94 short | INTRO: language 0=ENG 1=FRE 2=GER 3=ITA (INTRO always writes 0) | |
| +96 byte | `_SET_ACTION_PHASE` ← `_DELAYTIME` | |
| +97..+112 | INTRO `_DOCAL`: joystick calibration, 2 sticks × 4 shorts (centre x, centre y, half-range x, half-range y) ; copied to `_GAMESTUFF+468..` | |

INTRO.PRG does **not** pass player names, human/computer flags or skill levels — those are entered in CYBOR's
options screen (`_GET_OPTIONS`/`_READ_HUMAN_NAME`) and live in `_MECHRECORD`. `_HUMANNUMBER` is passed to
ACTION.PRG via `_GAMESTUFF+484`.

### ACTSTUF.IAT (486 bytes `_GAMESTUFF`) — written by `_START`, read back by `_START2` (other agent). Fields touched here: +461 joysticks, +467 sound (from `_MAIN`), +468.. joystick calibration, +484 `_HUMANNUMBER`.
### PASS2.DAT — 1155 (0x483) bytes = `_ALIST` (the pending battle list), loaded by `_SET_ACTION_PHASE+0x32D` when resuming (`_PASSFLAG != 0`); written in the action-setup path (other agent).

---------------------------------------------------------------------------------------------------------
## 5. Open questions
1. `_FILL_STATS` networth constant: the second double at DS:0x2692 decodes as **1000.0** (the `_YOF` table dump: `{100.0, 1000.0}` at 0x268A/0x2692), so the stats-screen "NETWORTH" is `Σ value*(100-dmg)/100*1000`, i.e. 1/100 of the per-round income (`value*(100-dmg)*1000`). Whether the display multiplies elsewhere was not checked — verify against the original screen.
2. Exact semantics of country fields +194..+204 (building slot codes 1..9), +388 (factory type), +382/+394..+406 (never referenced here) — building/economy agent.
3. `_SET_ACTION_PHASE`'s use of `_PZ+44..62` locals and the city-damage formula at +0x3F6..0x432 (FP sequence not decoded here) — action-setup agent.
4. `_SAVE_DATA_FILE2` opens `acts.DAT` with mode 0x8004 (read-only|binary) and then `write`s — works only because `_AOPEN` probably ignores/forces the mode; check `_AOPEN`.
5. The 1155-byte PASS2.DAT structure (`_ALIST`, 6-byte entries: `{a, b, flag}` seen at +0x596) — action-setup agent.
6. `_CHECK_CRITICAL` and `_INTERROGATE_*`/`_GET_COUNTRY_NUM` use `si < 0x48` vs `<= 0x48` inconsistently; country 72 is excluded from the critical-warning count only (cosmetic).
7. Skill button ordering: button index 0 is drawn under the "NASTY" label side (x=0x70) and 5 under "WIMPY"; confirm HAL's interpretation of `P.skill` (0 = hardest?).
8. `_SOUNDS` is only inc/dec'd for attacker/defender owners during the action phase; semantics beyond "won/lost battles this round" (sample 6 / 5) unknown.
9. `_LROWY` has only three values (0x48/0x54/0x60) in CYBOR; there is no per-human skill level.
