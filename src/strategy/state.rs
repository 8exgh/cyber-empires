//! Game state. Mirrors the original 17652-byte `_MECHRECORD` so that GAME?.DAT files are interchangeable.

use crate::data::*;
use crate::rng::Rng;
use crate::world_data::{CITY_COUNTRIES, COUNTRIES, NUM_COUNTRIES, STARTING_LOCATIONS, START_SCROLL};

pub const NEUTRAL: u8 = 5;
pub const MAX_PLAYERS: usize = 5;
pub const RECORD_SIZE: usize = 17652;
pub const COUNTRY_RECORD: usize = 236;

/// Build-slot codes (country.slots[i]).
pub const SLOT_EMPTY: u8 = 0;
pub const SLOT_CAPITAL: u8 = 1;
pub const SLOT_FORT: u8 = 2;
pub const SLOT_FACTORY_UNI: u8 = 3;
pub const SLOT_FACTORY_DUAL: u8 = 4;
pub const SLOT_FACTORY_QUAD: u8 = 6;
pub const SLOT_ENHANCEMENT: u8 = 7;
pub const EMPTY_BAY: i16 = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerType {
    None,
    Human,
    Computer,
}

impl PlayerType {
    pub fn code(self) -> i16 {
        match self {
            PlayerType::None => 0x48,
            PlayerType::Human => 0x54,
            PlayerType::Computer => 0x60,
        }
    }
    pub fn from_code(c: i16) -> PlayerType {
        match c {
            0x54 => PlayerType::Human,
            0x60 => PlayerType::Computer,
            _ => PlayerType::None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub id: u8,
    pub name: String,
    pub order_slot: i16,
    pub vault: i64,
    pub ptype: PlayerType,
    pub map_x: i16,
    pub map_y: i16,
    /// Computer skill 0 (NASTY) .. 5 (WIMPY).
    pub skill: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unit {
    /// 0..8 cyborg type, 10 = empty slot
    pub kind: u8,
    pub status: i16,
    /// destination country (== own country when not moving)
    pub dest: u8,
}

impl Unit {
    pub const EMPTY: Unit = Unit { kind: NO_CYBORG, status: 100, dest: 0 };
    pub fn present(&self) -> bool {
        self.kind != NO_CYBORG
    }
}

#[derive(Clone, Debug)]
pub struct Country {
    pub owner: u8,
    pub number: u8,
    /// 7 build slots; slot 0 is the capital/government.
    pub slots: [u8; 7],
    pub space: u8,
    pub fort: u8,
    /// UI counter of cyborgs (maintained by the move box)
    pub count: i16,
    pub units: [Unit; 10],
    /// 24 factory bays: 10 = empty, otherwise turns_left*100 + type; < 100 = finished type
    pub bays: [i16; 24],
    pub damage: i16,
    pub value: i16,
    pub nbays: i16,
    /// owner id of this round's attacker, 5 = none
    pub attacked_by: u8,
}

impl Country {
    pub fn blank(number: u8) -> Country {
        let mut units = [Unit::EMPTY; 10];
        for u in units.iter_mut() {
            u.dest = number;
        }
        Country {
            owner: NEUTRAL,
            number,
            slots: [0; 7],
            space: 0,
            fort: 0,
            count: 0,
            units,
            bays: [EMPTY_BAY; 24],
            damage: 0,
            value: 0,
            nbays: 0,
            attacked_by: NEUTRAL,
        }
    }
    pub fn name(&self) -> &'static str {
        COUNTRIES[self.number as usize].name
    }
    pub fn terrain(&self) -> u8 {
        COUNTRIES[self.number as usize].terrain
    }
    pub fn adj(&self) -> &'static [u8] {
        COUNTRIES[self.number as usize].adj
    }
    pub fn x(&self) -> i16 {
        COUNTRIES[self.number as usize].x
    }
    pub fn y(&self) -> i16 {
        COUNTRIES[self.number as usize].y
    }
    pub fn has_capital(&self) -> bool {
        self.slots[0] == SLOT_CAPITAL
    }
    pub fn num_units(&self) -> usize {
        self.units.iter().filter(|u| u.present()).count()
    }
    pub fn is_city(&self) -> bool {
        self.terrain() == 5
    }
    pub fn free_unit_slot(&self) -> Option<usize> {
        self.units.iter().position(|u| !u.present())
    }
    /// Units present and not in transit.
    pub fn resident_units(&self) -> impl Iterator<Item = (usize, &Unit)> {
        let n = self.number;
        self.units.iter().enumerate().filter(move |(_, u)| u.present() && u.dest == n)
    }
    pub fn tonnage(&self) -> i32 {
        self.resident_units().map(|(_, u)| ARMOR[u.kind as usize]).sum()
    }
    pub fn factory_count(&self) -> i32 {
        self.slots.iter().filter(|&&s| (3..=6).contains(&s)).count() as i32
    }
    pub fn enhancements(&self) -> i32 {
        self.slots.iter().filter(|&&s| s == SLOT_ENHANCEMENT).count() as i32
    }
    pub fn free_space(&self) -> i32 {
        self.slots.iter().take(self.space as usize).filter(|&&s| s == 0).count() as i32
    }
    pub fn free_bays(&self) -> i32 {
        self.bays.iter().take(self.nbays.max(0) as usize).filter(|&&b| b == EMPTY_BAY).count() as i32
    }
    pub fn has_fort_slot(&self) -> bool {
        self.slots.iter().any(|&s| s == SLOT_FORT)
    }
    /// Raze all structures (CLEAR_STRUCT).
    pub fn clear_structures(&mut self) {
        self.fort = 0;
        self.nbays = 0;
        self.bays = [EMPTY_BAY; 24];
        self.slots = [0; 7];
    }
    pub fn fill_build_area(&mut self, code: u8) -> bool {
        for i in 0..(self.space as usize).min(7) {
            if self.slots[i] == 0 {
                self.slots[i] = code;
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub players: Vec<Player>,
    /// index 0 unused; countries 1..=72
    pub countries: Vec<Country>,
    /// 1 poor, 2 average, 3 rich
    pub planet: i16,
    pub season: i16,
    pub year: i16,
    pub cur_player: i16,
    pub prev_player: i16,
    /// 1 = complete campaign (battles may be fought), 0 = strategy campaign (simulated)
    pub complete_campaign: bool,
    /// HAL game-stage counter 0..10
    pub stage: i16,
    pub fog: bool,
    pub rng: Rng,
    /// per-player battle tally this round (won +1 / lost -1)
    pub sounds: [i16; 5],
    pub lastrowy: [i16; 5],
    pub testrowy: [i16; 5],
    pub halctytotal: [i16; 5],
    pub aneutral: [u8; 2],
    /// messages generated during AI/round processing for the UI
    pub log: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct NewGameOptions {
    pub types: [PlayerType; 5],
    pub names: [String; 5],
    pub skills: [i16; 5],
    pub planet: i16,
    pub fog: bool,
    pub complete_campaign: bool,
}

impl Game {
    /// Template state (countries from the static world data, everything else blank).
    pub fn template(seed: u32) -> Game {
        let mut countries = Vec::with_capacity(NUM_COUNTRIES + 1);
        for c in 0..=NUM_COUNTRIES {
            countries.push(Country::blank(c as u8));
        }
        let players = (0..5)
            .map(|i| Player { id: i as u8, name: String::new(), order_slot: 0, vault: 0, ptype: PlayerType::None, map_x: 0, map_y: 0, skill: 0 })
            .collect();
        Game {
            players,
            countries,
            planet: 2,
            season: 1,
            year: 2200,
            cur_player: -1,
            prev_player: 0,
            complete_campaign: true,
            stage: 0,
            fog: false,
            rng: Rng::new(seed),
            sounds: [0; 5],
            lastrowy: [0x48; 5],
            testrowy: [0x48; 5],
            halctytotal: [0; 5],
            aneutral: [0; 2],
            log: Vec::new(),
        }
    }

    pub fn country(&self, c: u8) -> &Country {
        &self.countries[c as usize]
    }
    pub fn country_mut(&mut self, c: u8) -> &mut Country {
        &mut self.countries[c as usize]
    }
    pub fn owner(&self, c: u8) -> u8 {
        self.countries[c as usize].owner
    }
    pub fn player(&self, p: u8) -> &Player {
        &self.players[p as usize]
    }
    pub fn cur(&self) -> &Player {
        &self.players[self.cur_player.max(0) as usize]
    }
    pub fn is_computer(&self, p: u8) -> bool {
        self.players[p as usize].ptype == PlayerType::Computer
    }
    pub fn is_human(&self, p: u8) -> bool {
        self.players[p as usize].ptype == PlayerType::Human
    }
    pub fn active_players(&self) -> usize {
        self.players.iter().filter(|p| p.ptype != PlayerType::None).count()
    }
    pub fn rand(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.range(lo, hi)
    }

    /// GAME_INIT + GET_OPTIONS: a brand new game.
    pub fn new_game(opts: &NewGameOptions, seed: u32) -> Game {
        let mut g = Game::template(seed);
        g.planet = opts.planet;
        g.fog = opts.fog;
        g.complete_campaign = opts.complete_campaign;
        for p in 0..5 {
            g.players[p].ptype = opts.types[p];
            g.players[p].skill = opts.skills[p];
            g.players[p].name = if opts.types[p] == PlayerType::Computer && opts.names[p].is_empty() {
                EMPTE_NAME(p)
            } else {
                opts.names[p].clone()
            };
        }
        g.game_init();
        g
    }

    /// The original `_GAME_INIT`.
    pub fn game_init(&mut self) {
        self.season = 1;
        self.stage = 0;
        self.year = 2200;
        let worth = self.planet as i32;
        for c in 1..=NUM_COUNTRIES {
            let v = self.rand(3, 10) * worth;
            let sp = self.rand(2, 7);
            let cty = &mut self.countries[c];
            cty.value = v as i16;
            cty.space = sp as u8;
            cty.owner = NEUTRAL;
            cty.fort = 0;
            cty.nbays = 0;
            cty.damage = 0;
            cty.slots = [0; 7];
            for u in cty.units.iter_mut() {
                *u = Unit { kind: NO_CYBORG, status: 0, dest: c as u8 };
            }
            cty.bays = [EMPTY_BAY; 24];
            cty.count = 0;
            cty.attacked_by = NEUTRAL;
        }
        for &c in CITY_COUNTRIES.iter() {
            let cty = &mut self.countries[c as usize];
            cty.value = (cty.value as i32 * worth + 10) as i16;
            cty.space = 1;
        }
        let start_value = (self.rand(7, 10) * worth) as i16;
        let order = self.set_order();
        for k in 0..5 {
            let p = order[k] as usize;
            if self.players[p].ptype == PlayerType::None {
                continue;
            }
            let r = self.rand(0, 2) as usize;
            let c = STARTING_LOCATIONS[k][r];
            let cty = &mut self.countries[c as usize];
            cty.owner = p as u8;
            cty.value = start_value;
            cty.space = 7;
            cty.slots[0] = SLOT_CAPITAL;
            self.players[p].vault = 1_210_000;
            let (mut mx, mut my) = (0, 0);
            for &(sc, x, y) in START_SCROLL.iter() {
                if sc == c {
                    mx = x;
                    my = y;
                }
            }
            self.players[p].map_x = mx;
            self.players[p].map_y = my;
        }
        self.cur_player = -1;
        self.lastrowy = self.player_codes();
        self.testrowy = self.lastrowy;
    }

    pub fn player_codes(&self) -> [i16; 5] {
        let mut a = [0x48; 5];
        for p in 0..5 {
            a[p] = self.players[p].ptype.code();
        }
        a
    }

    /// `_SET_ORDER`: random permutation by largest draw first.
    pub fn set_order(&mut self) -> [u8; 5] {
        let mut draws = [0i32; 5];
        for d in draws.iter_mut() {
            *d = self.rand(1, 100);
        }
        let mut order = [0u8; 5];
        for k in 0..5 {
            let mut best = 0;
            let mut hi = 0;
            for i in 0..5 {
                if draws[i] > hi {
                    hi = draws[i];
                    best = i;
                }
            }
            draws[best] = 0;
            order[k] = best as u8;
            self.players[best].order_slot = k as i16;
        }
        order
    }

    // ---------------------------------------------------------------- original file format

    /// Serialise to the original big-endian GAME?.DAT image.
    pub fn to_dat(&self) -> Vec<u8> {
        let mut d = vec![0u8; RECORD_SIZE];
        let w16 = |d: &mut Vec<u8>, off: usize, v: i16| {
            d[off] = (v >> 8) as u8;
            d[off + 1] = v as u8;
        };
        let w32 = |d: &mut Vec<u8>, off: usize, v: i32| {
            d[off..off + 4].copy_from_slice(&v.to_be_bytes());
        };
        w16(&mut d, 0, self.active_players() as i16);
        w16(&mut d, 4, self.planet);
        w16(&mut d, 10, self.cur_player.max(0));
        for p in 0..5 {
            let b = 12 + 32 * p;
            let pl = &self.players[p];
            w16(&mut d, b, p as i16);
            let nm = pl.name.as_bytes();
            for i in 0..16 {
                d[b + 2 + i] = if i < nm.len().min(15) { nm[i] } else { 0 };
            }
            w16(&mut d, b + 18, pl.order_slot);
            w32(&mut d, b + 20, pl.vault as i32);
            w16(&mut d, b + 24, pl.ptype.code());
            w16(&mut d, b + 26, pl.map_x);
            w16(&mut d, b + 28, pl.map_y);
            w16(&mut d, b + 30, pl.skill);
        }
        for c in 1..=NUM_COUNTRIES {
            let b = c * COUNTRY_RECORD;
            let cty = &self.countries[c];
            w16(&mut d, b + 172, cty.owner as i16);
            let nm = cty.name().as_bytes();
            for i in 0..16 {
                d[b + 174 + i] = if i < nm.len().min(14) { nm[i] } else { 0 };
            }
            w16(&mut d, b + 190, c as i16);
            for i in 0..7 {
                w16(&mut d, b + 192 + 2 * i, cty.slots[i] as i16);
            }
            let adj = cty.adj();
            for i in 0..9 {
                w16(&mut d, b + 206 + 2 * i, if i < adj.len() { adj[i] as i16 } else { 0 });
            }
            w16(&mut d, b + 224, cty.space as i16);
            w16(&mut d, b + 226, cty.fort as i16);
            w16(&mut d, b + 228, cty.count);
            w16(&mut d, b + 230, cty.x());
            w16(&mut d, b + 232, cty.y());
            for i in 0..10 {
                let u = &cty.units[i];
                w16(&mut d, b + 234 + 10 * i, u.kind as i16);
                w16(&mut d, b + 236 + 10 * i, u.status);
                w16(&mut d, b + 238 + 10 * i, u.dest as i16);
            }
            for i in 0..24 {
                w16(&mut d, b + 334 + 2 * i, cty.bays[i]);
            }
            w16(&mut d, b + 384, cty.damage);
            w16(&mut d, b + 386, cty.value);
            w16(&mut d, b + 388, cty.nbays);
            w16(&mut d, b + 390, cty.terrain() as i16);
            w16(&mut d, b + 392, cty.attacked_by as i16);
        }
        w16(&mut d, 17636, self.season);
        w16(&mut d, 17638, self.year);
        w16(&mut d, 17642, self.prev_player);
        w16(&mut d, 17644, self.complete_campaign as i16);
        w16(&mut d, 17646, self.stage);
        w16(&mut d, 17648, self.fog as i16);
        d
    }

    /// Load from an original GAME?.DAT image (big-endian).
    pub fn from_dat(d: &[u8], seed: u32) -> Option<Game> {
        if d.len() < RECORD_SIZE {
            return None;
        }
        let r16 = |off: usize| -> i16 { ((d[off] as i16) << 8) | d[off + 1] as i16 };
        let r32 = |off: usize| -> i32 { i32::from_be_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]) };
        let mut g = Game::template(seed);
        g.planet = r16(4).clamp(1, 3);
        g.cur_player = r16(10);
        for p in 0..5 {
            let b = 12 + 32 * p;
            let pl = &mut g.players[p];
            pl.name = cstr(&d[b + 2..b + 18]);
            pl.order_slot = r16(b + 18);
            pl.vault = r32(b + 20) as i64;
            pl.ptype = PlayerType::from_code(r16(b + 24));
            pl.map_x = r16(b + 26);
            pl.map_y = r16(b + 28);
            pl.skill = r16(b + 30);
        }
        for c in 1..=NUM_COUNTRIES {
            let b = c * COUNTRY_RECORD;
            let cty = &mut g.countries[c];
            cty.owner = r16(b + 172).clamp(0, 5) as u8;
            for i in 0..7 {
                cty.slots[i] = r16(b + 192 + 2 * i).clamp(0, 9) as u8;
            }
            cty.space = r16(b + 224).clamp(0, 7) as u8;
            cty.fort = r16(b + 226).clamp(0, 3) as u8;
            cty.count = r16(b + 228);
            for i in 0..10 {
                let kind = r16(b + 234 + 10 * i);
                cty.units[i] = Unit {
                    kind: if (0..9).contains(&kind) { kind as u8 } else { NO_CYBORG },
                    status: r16(b + 236 + 10 * i),
                    dest: r16(b + 238 + 10 * i).clamp(0, 72) as u8,
                };
            }
            for i in 0..24 {
                cty.bays[i] = r16(b + 334 + 2 * i);
            }
            cty.damage = r16(b + 384);
            cty.value = r16(b + 386);
            cty.nbays = r16(b + 388);
            cty.attacked_by = r16(b + 392).clamp(0, 5) as u8;
        }
        g.season = r16(17636).clamp(1, 7);
        g.year = r16(17638);
        g.prev_player = r16(17642);
        g.complete_campaign = r16(17644) != 0;
        g.stage = r16(17646);
        g.fog = r16(17648) != 0;
        g.lastrowy = g.player_codes();
        g.testrowy = g.lastrowy;
        Some(g)
    }
}

fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).to_string()
}

#[allow(non_snake_case)]
fn EMPTE_NAME(p: usize) -> String {
    EMPIRE_NAMES[p].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_round_trip() {
        let opts = NewGameOptions {
            types: [PlayerType::Human, PlayerType::Computer, PlayerType::Computer, PlayerType::None, PlayerType::None],
            names: ["A".into(), "B".into(), "C".into(), String::new(), String::new()],
            skills: [0, 1, 2, 3, 4],
            planet: 3,
            fog: true,
            complete_campaign: true,
        };
        let g = Game::new_game(&opts, 42);
        let dat = g.to_dat();
        assert_eq!(dat.len(), RECORD_SIZE);
        let g2 = Game::from_dat(&dat, 1).unwrap();
        assert_eq!(g2.to_dat(), dat);
        assert_eq!(g2.planet, 3);
        assert!(g2.fog);
        assert_eq!(g2.players[0].vault, 1_210_000);
        assert_eq!((1..=72).filter(|&c| g2.countries[c].owner != NEUTRAL).count(), 3);
    }

    #[test]
    fn ai_game_runs() {
        let opts = NewGameOptions { types: [PlayerType::Computer; 5], names: Default::default(), skills: [0, 1, 2, 3, 5], planet: 2, fog: false, complete_campaign: false };
        let mut g = Game::new_game(&opts, 7);
        for _ in 0..12 {
            for p in 0..5u8 {
                g.cur_player = p as i16;
                if g.check_status() {
                    crate::strategy::hal::hals_turn(&mut g);
                }
            }
            g.run_round_simulated();
        }
        assert!(g.year >= 2201);
        assert!((1..=72).any(|c| g.countries[c].owner != NEUTRAL));
    }
}
