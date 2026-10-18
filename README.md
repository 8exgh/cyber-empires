# Cyber Empires — Rust / WebAssembly re-implementation

A from-scratch re-implementation of **Cyber Empires** (Silicon Knights / SSI, 1992, also known as
*Steel Empire*), the turn-based strategy game with real-time cyborg battles. The game logic was
reverse-engineered from the original DOS executables (which still carried their Turbo C debug
symbols) and from the manual; it is written in Rust, compiled to WebAssembly and runs in any modern
browser with a small hand-written JavaScript shell.

The repository contains **no copyrighted game data**. The original art, maps and data files are
loaded at run time from your own copy of the game.

## What is implemented

* **Strategy campaign** — 72-country map of Orion, up to five empires (human or computer), capitals,
  factories (uni/dual/quadro post), factory enhancements, fortifications, cyborg production with
  build times, movement, repairs, spying, fog of war, planet types, income, seasons/years,
  elimination and victory. The computer player ("HAL") uses the original shopping matrix, priority
  system, skill levels and garrison rules.
* **Battle resolution** — the original statistical battle simulator (used by the *Strategy Campaign*
  and whenever a battle is simulated), city damage, structure losses, retreats, survivor placement.
* **Complete campaign** — attacker setup (mission, commitment, control cyborg), defender setup
  (cyborg placement on the minimap, AUTO), and the real-time top-down battle: 80×78-tile
  destructible battlefields composed from the original country maps with the defender's capital,
  factories, enhancements, fort walls and gun turrets; nine cyborg classes with the original
  weapons, heat, cooling, ammunition and sprites; guided long-range missiles; hold fast / retreat
  orders; computer-controlled wingmen and enemies; split-screen views, radars and HUD.
* **Battle practice** — Showdown (human vs human, human vs computer, demo) and Survivor with the
  Hall of Cybernetic Legends.
* Saved games use the original `GAME<n>.DAT` byte layout (stored in the browser's local storage), so
  the format is interchangeable with the DOS version.

Not implemented: music and sound effects (AdLib/Roland), palette fades, joystick input, the
floppy-disk copy protection.

## Running it

You need the original game files (the DOS release, e.g. a zip containing `CYBOR.PRG`, `*.GIF`,
`*.MAP`, `MECHS.DAT` …).

```bash
# 1. build the WebAssembly module (installs the wasm32 target if needed)
scripts/build.sh

# 2. either copy your game files next to the page (git-ignored) ...
python3 scripts/extract-assets.py path/to/Cyber-Empires.zip      # or a folder
#    ... or skip this step and drop the zip/folder onto the page later

# 3. serve the web/ folder and open it
python3 -m http.server 8080 -d web
```

Open <http://localhost:8080>. If no game data is found on the server the page asks you to drop your
zip file or game folder onto it; the files never leave your browser.

### Controls

Strategy screens are mouse driven (arrow keys also scroll the map). In combat:

| action | defender / single player | attacker (second player) |
|---|---|---|
| move / turn | numeric keypad 1–9 or cursor keys | Q W E A D Z X C |
| fire | keypad + / keypad Enter | Space |
| next weapon | keypad 0 / keypad . | Tab or S |
| switch cyborg | F1 | F4 |
| hold fast | F9 | F2 |
| retreat | F8 | F3 |
| pause | Esc | Esc |

A single human player may use either set. A guided missile salvo is steered with the movement keys
and detonated with a fresh press of fire.

## Development

```bash
cargo build --release --features native        # native helper binary
./target/release/ce-tool render web/game script.txt out/   # headless screenshots (see tools/ce_tool.rs)
./target/release/ce-tool simgame 7 40          # all-computer strategy game, 40 rounds
./target/release/ce-tool battle web/game 3 2000 out/       # computer-vs-computer real-time battle
```

* `src/strategy/` — game state (`state.rs`, with the original save layout), player operations
  (`rules.rs`), end-of-round movement and battle resolution (`resolve.rs`), the computer player
  (`hal.rs`), turn flow (`turn.rs`).
* `src/battle/` — battlefield composition (`setup.rs`), the real-time simulation (`sim.rs`),
  rendering (`render.rs`), the battle screen (`screen.rs`), result application (`result.rs`) and the
  data tables extracted from the executables (`tables.rs`).
* `src/ui/` — all screens (campaign settings, strategy control screen, pre-battle setup, battle
  practice, summaries), `src/gfx.rs`/`src/text.rs` — the 8-bit software renderer and the bitmap fonts
  taken from the original button sheets, `src/gif.rs` — GIF87a decoder, `src/wasm.rs` — the C ABI
  used by `web/app.js`.
* `docs/re/` — the reverse-engineering notes (data formats, rules, AI, battle mechanics, UI
  rectangles) and `tools/re/` the scripts that produced the annotated disassembly and data tables.
  `docs/manual_ocr.txt` is an OCR of the original rule book used for cross-checking.

## Deployment

Pushing to `main` builds the image `ghcr.io/8exgh/cyber-empires` (multi-stage: Rust → nginx) and
dispatches `deploy-cyber-empires.yml` in the `8exgh/devops` repository, which runs it on Server7
(host port 3058) behind the Server3 Cloudflare tunnel as <https://cyberempire.fusenv.com>
(`configure-cyber-empires-cloudflare.yml` creates the DNS record and tunnel route).

The image ships without game data. To make the site load the game without the drag-and-drop step,
put the extracted files (lower-case names plus a `manifest.txt` as produced by
`scripts/extract-assets.py`) in `/opt/cyber-empires/game` on the host; it is mounted read-only at
`/game/` in the container.
