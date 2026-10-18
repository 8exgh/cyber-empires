//! Native helper for development: decode/verify GIFs, render screens headlessly.
use cyber_empires::app::App;
use std::fs;
use std::path::Path;

fn load_assets(app: &mut App, dir: &str) {
    for e in fs::read_dir(dir).expect("asset dir") {
        let e = e.unwrap();
        let p = e.path();
        if p.is_file() {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            app.add_asset(&name, fs::read(&p).unwrap());
        }
    }
}

fn write_ppm(app: &App, path: &str) {
    let mut out = format!("P6\n{} {}\n255\n", 320, 200).into_bytes();
    for i in 0..320 * 200 {
        out.extend_from_slice(&app.rgba[i * 4..i * 4 + 3]);
    }
    fs::write(path, out).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage:\n  ce-tool gifcheck <gif> <ref.idx> <ref.pal>\n  ce-tool render <assets_dir> <script-file> [outdir]\n    script lines: frame <seconds> | click <x> <y> | move <x> <y> | key <code> | type <text> | dump <name> | seed <n>");
        return;
    }
    match args[1].as_str() {
        "gifcheck" => {
            let data = fs::read(&args[2]).unwrap();
            let g = cyber_empires::gif::decode(&data).unwrap();
            let idx = fs::read(&args[3]).unwrap();
            let pal = fs::read(&args[4]).unwrap();
            let bad = g.pixels.iter().zip(idx.iter()).filter(|(a, b)| a != b).count();
            let mut palbad = 0;
            for i in 0..(1usize << g.palette_bits) {
                if g.palette[i] != [pal[i * 3], pal[i * 3 + 1], pal[i * 3 + 2]] {
                    palbad += 1;
                }
            }
            println!("{} {}x{} bits={} pixel_mismatch={} pal_mismatch={}", args[2], g.width, g.height, g.palette_bits, bad, palbad);
        }
        "render" => {
            let mut app = App::new();
            load_assets(&mut app, &args[2]);
            let script = fs::read_to_string(&args[3]).unwrap();
            let outdir = args.get(4).cloned().unwrap_or_else(|| ".".to_string());
            fs::create_dir_all(&outdir).ok();
            let mut seed = 1u32;
            let mut started = false;
            for line in script.lines() {
                let t: Vec<&str> = line.split_whitespace().collect();
                if t.is_empty() || t[0].starts_with('#') {
                    continue;
                }
                if !started && t[0] != "seed" {
                    if let Err(e) = app.start(seed) {
                        eprintln!("start failed: {}", e);
                        return;
                    }
                    started = true;
                }
                match t[0] {
                    "seed" => seed = t[1].parse().unwrap(),
                    "frame" => {
                        let secs: f32 = t[1].parse().unwrap();
                        let n = ((secs * 60.0).ceil() as usize).max(1);
                        for _ in 0..n {
                            app.frame(secs / n as f32);
                        }
                    }
                    "move" => {
                        app.input.mouse_x = t[1].parse().unwrap();
                        app.input.mouse_y = t[2].parse().unwrap();
                        app.frame(1.0 / 60.0);
                    }
                    "click" => {
                        app.input.mouse_x = t[1].parse().unwrap();
                        app.input.mouse_y = t[2].parse().unwrap();
                        app.frame(1.0 / 60.0);
                        app.input.mouse_down = true;
                        app.frame(1.0 / 60.0);
                        app.input.mouse_down = false;
                        app.frame(1.0 / 60.0);
                    }
                    "key" => {
                        let code: u8 = t[1].parse().unwrap();
                        app.input.keys[code as usize] = true;
                        app.frame(1.0 / 60.0);
                        app.input.keys[code as usize] = false;
                        app.frame(1.0 / 60.0);
                    }
                    "type" => {
                        for c in t[1..].join(" ").bytes() {
                            app.input.typed.push(c);
                            app.frame(1.0 / 60.0);
                        }
                    }
                    "dump" => {
                        let p = Path::new(&outdir).join(format!("{}.ppm", t[1]));
                        write_ppm(&app, p.to_str().unwrap());
                        println!("wrote {} [{}]", p.display(), app.debug_state());
                    }
                    "testgame" => {
                        // Human (player 0) vs Computer (player 1): human units already plotted to attack the computer's country.
                        use cyber_empires::strategy::state::{Game, NewGameOptions, PlayerType, Unit, SLOT_CAPITAL, SLOT_FORT, SLOT_FACTORY_DUAL};
                        use cyber_empires::ui::strategy::StrategyScreen;
                        let complete = t.get(1).map_or(true, |v| *v != "strategy");
                        let opts = NewGameOptions { types: [PlayerType::Human, PlayerType::Computer, PlayerType::None, PlayerType::None, PlayerType::None], names: ["SEAN".into(), "DESTROYER".into(), String::new(), String::new(), String::new()], skills: [2; 5], planet: 2, fog: false, complete_campaign: complete };
                        let mut g = Game::new_game(&opts, seed);
                        let hc = (1..=72u8).find(|&c| g.countries[c as usize].owner == 0).unwrap();
                        let ec = g.countries[hc as usize].adj()[0];
                        g.countries[ec as usize].owner = 1;
                        g.countries[ec as usize].slots = [SLOT_CAPITAL, SLOT_FORT, SLOT_FACTORY_DUAL, 0, 0, 0, 0];
                        g.countries[ec as usize].space = 7;
                        g.countries[ec as usize].fort = 1;
                        for (i, k) in [5u8, 7, 8].iter().enumerate() { g.countries[ec as usize].units[i] = Unit { kind: *k, status: 100, dest: ec }; }
                        for (i, k) in [0u8, 1, 3, 6].iter().enumerate() { g.countries[hc as usize].units[i] = Unit { kind: *k, status: 100, dest: ec }; }
                        g.countries[hc as usize].count = 0;
                        g.cur_player = 0;
                        let mut scr = StrategyScreen::new(g, complete);
                        scr.doors_open = true;
                        app.screen = cyber_empires::app::Screen::Strategy(Box::new(scr));
                        started = true;
                        println!("testgame: human country {} attacks {}", hc, ec);
                    }
                    "log" => {
                        for l in app.log.drain(..) {
                            println!("LOG {}", l);
                        }
                    }
                    _ => eprintln!("unknown script command {}", t[0]),
                }
            }
            for l in app.log.drain(..) {
                println!("LOG {}", l);
            }
        }
        "mapview" => {
            // ce-tool mapview <assets_dir> <terrain> <country> <out.ppm> [scale_div]
            use cyber_empires::battle::map::{BattleMap, TileSet, MAP_H, MAP_W, TILE};
            use cyber_empires::gfx::{Rect, Surface};
            let mut app = App::new();
            load_assets(&mut app, &args[2]);
            let terrain: u8 = args[3].parse().unwrap();
            let country: u8 = args[4].parse().unwrap();
            let div: i32 = args.get(6).map(|s| s.parse().unwrap()).unwrap_or(4);
            let map = BattleMap::load(&mut app.assets, terrain, country).expect("map");
            let base = [[0u8; 3]; 256];
            let ts = TileSet::load(&mut app.assets, terrain, &base, 1).expect("tileset");
            let full_w = MAP_W as i32 * TILE;
            let full_h = MAP_H as i32 * TILE;
            let mut big = Surface::new(full_w as usize, full_h as usize);
            ts.draw_map(&mut big, &map, Rect::new(0, 0, full_w, full_h), 0, 0);
            let (ow, oh) = ((full_w / div) as usize, (full_h / div) as usize);
            let mut out = format!("P6\n{} {}\n255\n", ow, oh).into_bytes();
            for y in 0..oh {
                for x in 0..ow {
                    let c = ts.pal[big.px[(y as i32 * div) as usize * big.w + (x as i32 * div) as usize] as usize];
                    out.extend_from_slice(&c);
                }
            }
            fs::write(&args[5], out).unwrap();
            println!("wrote {} ({}x{}), {} colours", args[5], ow, oh, ts.ncolors);
        }
        "simgame" => {
            // ce-tool simgame <seed> <rounds>
            use cyber_empires::strategy::state::{Game, NewGameOptions, PlayerType};
            let seed: u32 = args[2].parse().unwrap();
            let rounds: usize = args[3].parse().unwrap();
            let opts = NewGameOptions {
                types: [PlayerType::Computer; 5],
                names: Default::default(),
                skills: [0, 1, 2, 3, 5],
                planet: 2,
                fog: false,
                complete_campaign: false,
            };
            let mut g = Game::new_game(&opts, seed);
            for r in 0..rounds {
                for p in 0..5u8 {
                    if g.players[p as usize].ptype == PlayerType::None { continue; }
                    g.cur_player = p as i16;
                    if !g.check_status() { continue; }
                    cyber_empires::strategy::hal::hals_turn(&mut g);
                }
                g.run_round_simulated();
                let owned: Vec<usize> = (0..5).map(|p| g.countries_owned(p as u8)).collect();
                let vaults: Vec<i64> = g.players.iter().map(|p| p.vault).collect();
                let units: usize = (1..=72).map(|c| g.countries[c].num_units()).sum();
                let forts: usize = (1..=72).filter(|&c| g.countries[c].fort > 0).count();
                let facts: i32 = (1..=72).map(|c| g.countries[c].factory_count()).sum();
                println!("round {:3} y{} s{} stage {} owned {:?} units {} forts {} factories {} vaults {:?}", r + 1, g.year, g.season, g.stage, owned, units, forts, facts, vaults);
            }
            // round-trip the save format
            let dat = g.to_dat();
            let g2 = Game::from_dat(&dat, 1).unwrap();
            assert_eq!(g2.to_dat(), dat);
            println!("save round-trip ok ({} bytes)", dat.len());
        }
        "battle" => {
            // ce-tool battle <assets_dir> <seed> <ticks> <outdir> [defend_cty]
            use cyber_empires::battle::screen::{BattleScreen, Phase};
            use cyber_empires::battle::setup::{compose_battle, DefenderPlacement, SetupParams};
            use cyber_empires::strategy::resolve::load_parties;
            use cyber_empires::strategy::state::{Game, NewGameOptions, PlayerType, Unit, SLOT_CAPITAL, SLOT_FORT, SLOT_FACTORY_DUAL, SLOT_ENHANCEMENT};
            let mut app = App::new();
            load_assets(&mut app, &args[2]);
            app.start(1).unwrap();
            let seed: u32 = args[3].parse().unwrap();
            let ticks: usize = args[4].parse().unwrap();
            let outdir = args[5].clone();
            let defc: u8 = args.get(6).map(|s| s.parse().unwrap()).unwrap_or(4);
            fs::create_dir_all(&outdir).ok();
            let opts = NewGameOptions { types: [PlayerType::Human, PlayerType::Computer, PlayerType::None, PlayerType::None, PlayerType::None], names: Default::default(), skills: [2; 5], planet: 2, fog: false, complete_campaign: true };
            let mut g = Game::new_game(&opts, seed);
            // defender country with structures, attacker adjacent
            let attc = g.country(defc).adj()[0];
            g.countries[defc as usize].owner = 1;
            g.countries[defc as usize].slots = [SLOT_CAPITAL, SLOT_FORT, SLOT_FACTORY_DUAL, SLOT_ENHANCEMENT, 0, 0, 0];
            g.countries[defc as usize].space = 7;
            g.countries[defc as usize].fort = 2;
            g.countries[attc as usize].owner = 0;
            for (i, k) in [0u8, 2, 5, 6, 8].iter().enumerate() { g.countries[defc as usize].units[i] = Unit { kind: *k, status: 100, dest: defc }; }
            for (i, k) in [1u8, 3, 4, 7, 8, 5].iter().enumerate() { g.countries[attc as usize].units[i] = Unit { kind: *k, status: 100, dest: defc }; }
            let b = load_parties(&mut g, attc, defc);
            let params = SetupParams { adir: (seed % 4) as usize, att_goal: 3, att_agg: 2, att_control: 0, def_control: 0, human: [false, false], placement: DefenderPlacement::Ai };
            let mut rng = cyber_empires::rng::Rng::new(seed);
            let init = compose_battle(&mut app.assets, &g, &b, &mut rng, &params).expect("compose");
            let mut bs = BattleScreen::new(&mut app.assets, &init).expect("screen");
            bs.phase = Phase::Running;
            let input = cyber_empires::input::Input::new();
            let dump = |bs: &BattleScreen, app: &mut App, name: &str| {
                bs.draw(&mut app.fb);
                app.pal = bs.palette();
                app.present();
                write_ppm(app, &format!("{}/{}.ppm", outdir, name));
            };
            dump(&bs, &mut app, "b000");
            for t in 0..ticks {
                bs.update(&input, 1.0 / 18.2);
                if (t + 1) % 90 == 0 { dump(&bs, &mut app, &format!("b{:03}", t + 1)); }
                if bs.result.is_some() { break; }
            }
            let alive: Vec<String> = bs.sim.mechs.iter().enumerate().filter(|(_, m)| m.exists).map(|(i, m)| format!("{}:{}/{}/{}", i, m.kind, m.status, m.hits)).collect();
            println!("ticks run: {} result {:?} mechs {:?} bullets alive {} events {}", bs.sim.itime, bs.result.map(|r| (r.0, r.1, r.2)), alive, bs.sim.bullets.iter().filter(|b| b.alive).count(), bs.sounds.len());
            dump(&bs, &mut app, "bend");
        }
        _ => eprintln!("unknown command"),
    }
}
