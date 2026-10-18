//! The computer player ("HAL", module HAL.C of CYBOR.PRG).

use super::state::*;
use crate::data::*;
use crate::world_data::NUM_COUNTRIES;

/// `_BUYMATRIX`: row = game stage 0..10; columns: 0 capital, 1-3 factories, 4-12 cyborg types,
/// 13-15 forts, 16 enhancement, 17-19 unused, 20 starting level.
pub const BUYMATRIX: [[i16; 21]; 11] = [
    [8, 0, 0, 5, 0, 0, 0, 0, 0, 0, 0, 3, 6, 0, 0, 0, 2, 0, 0, 0, 8],
    [15, 0, 0, 1, 0, 0, 6, 6, 6, 8, 7, 7, 1, 0, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 0, 0, 8, 8, 9, 8, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 0, 6, 8, 8, 9, 6, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 0, 8, 8, 8, 9, 5, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 8, 8, 8, 8, 9, 6, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 10, 10, 11, 11, 13, 8, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 10, 10, 10, 10, 10, 8, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [15, 0, 0, 1, 12, 12, 13, 13, 13, 8, 4, 4, 1, 1, 0, 0, 2, 0, 0, 0, 15],
    [35, 0, 0, 1, 17, 17, 20, 12, 24, 8, 4, 4, 2, 0, 0, 0, 2, 0, 0, 0, 35],
    [35, 0, 0, 1, 24, 24, 26, 25, 26, 20, 4, 4, 2, 2, 0, 0, 2, 0, 0, 0, 35],
];
const BUILDPRI: [i32; 7] = [100, 0, 1000, 2000, 0, 5000, 200];
const CTYVAL: [i32; 7] = [30, 0, 500, 800, 1200, 1500, 200];
/// `CTYVAL[-1]` in the original reads the last entry of `_BUILDPRI` (200): reproduced on purpose.
const CTYVAL_EMPTY: i32 = 200;
const FCYB: [i32; 9] = [200, 180, 130, 90, 50, 50, 30, 20, 10];
const STAY: [[i32; 2]; 6] = [[1, 0], [1, 0], [1, 1], [2, 1], [2, 2], [4, 4]];
const FORTFACTOR: [i32; 4] = [0, 3, 6, 9];
const FACTFILL: [u8; 3] = [3, 4, 6];

struct Hal {
    cur: u8,
    skill: usize,
    row: [i16; 21],
    stage: usize,
    primove: [i32; 73],
    numunits: [i32; 73],
    halscty: Vec<u8>,
    aggression: bool,
    noeny: bool,
    nobuy: bool,
    blitz: bool,
    numforts: i32,
    numbays: i32,
    numcyborgs: i32,
    maxpurchase: i64,
    stayenemy: i32,
    flevel: u8,
    /// implicit global "COUNTRY1"
    country1: u8,
}

/// Run the whole AI turn for the current player.
pub fn hals_turn(g: &mut Game) {
    let cur = g.cur_player.max(0) as u8;
    let stage = g.stage.clamp(0, 10) as usize;
    let mut h = Hal {
        cur,
        skill: g.players[cur as usize].skill.clamp(0, 5) as usize,
        row: BUYMATRIX[stage],
        stage,
        primove: [0; 73],
        numunits: [0; 73],
        halscty: Vec::new(),
        aggression: true,
        noeny: false,
        nobuy: true,
        blitz: false,
        numforts: 0,
        numbays: 1,
        numcyborgs: 0,
        maxpurchase: 0,
        stayenemy: 0,
        flevel: 0,
        country1: 0,
    };
    add_enemy_units(g, &mut h);
    add_enemy_forts(g, &mut h);
    h.row[8] += h.numforts as i16;
    get_hals_countries(g, &mut h);
    set_front_priority(g, &mut h);
    set_move_priority(g, &mut h);
    if h.noeny {
        set_neutral_priority(g, &mut h);
    }
    hal_repairs(g, &mut h);
    let mut money = g.players[cur as usize].vault;
    if h.skill == 0 {
        money = (money as f64 * 1.5) as i64;
    } else if h.skill == 1 {
        money = (money as f64 * 1.3) as i64;
    }
    h.maxpurchase = money / h.numbays.max(1) as i64;
    if h.maxpurchase > 60000 && g.year < 2201 {
        h.maxpurchase = 50000;
    }
    if money < 300_000 {
        h.maxpurchase = 40000;
    }
    if h.numcyborgs < 15 {
        h.maxpurchase = 27000;
    }
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 2000 || money <= 0 {
            break;
        }
        let (item, cont) = hal_buys(&mut h);
        if !cont {
            break;
        }
        match item {
            Some(0) => locate_capitol(g, &mut h, &mut money),
            Some(i @ 1..=3) => locate_factory(g, &mut h, i, &mut money),
            Some(i @ 4..=12) => locate_battle_unit(g, &mut h, i - 4, &mut money),
            Some(13..=15) => locate_fortification(g, &mut h, &mut money),
            Some(16) => locate_enhancement(g, &mut h, &mut money),
            _ => {}
        }
    }
    g.players[cur as usize].vault = money;
    hal_moves(g, &mut h);
    let _ = h.stage;
}

fn add_enemy_units(g: &Game, h: &mut Hal) {
    for c in 1..=NUM_COUNTRIES {
        h.numunits[c] = 0;
        if g.countries[c].owner == h.cur {
            continue;
        }
        for &a in g.countries[c].adj() {
            if g.owner(a) != h.cur {
                continue;
            }
            h.numunits[c] += g.country(a).num_units() as i32;
        }
    }
}

fn add_enemy_forts(g: &Game, h: &mut Hal) {
    h.numforts = 0;
    h.numbays = 0;
    h.numcyborgs = 0;
    for c in 1..=NUM_COUNTRIES {
        let cty = &g.countries[c];
        if cty.has_capital() && cty.owner != h.cur && cty.fort != 0 {
            h.numforts += 1;
        }
        if cty.owner != h.cur {
            continue;
        }
        for &s in cty.slots.iter() {
            if s > 2 && s < 7 {
                h.numbays += s as i32 - 2;
            }
        }
        h.numcyborgs += cty.num_units() as i32;
    }
    if h.numforts > 5 {
        h.numforts = 5;
    }
    if h.numbays == 0 {
        h.numbays = 1;
    }
}

fn get_hals_countries(g: &mut Game, h: &mut Hal) {
    h.halscty = (1..=NUM_COUNTRIES as u8).filter(|&c| g.owner(c) == h.cur).collect();
    let n = h.halscty.len() as i16;
    h.aggression = !(g.halctytotal[h.cur as usize] - 2 > n);
    g.halctytotal[h.cur as usize] = n;
}

fn set_front_priority(g: &Game, h: &mut Hal) {
    for k in 0..h.halscty.len() {
        let c = h.halscty[k];
        let mut enemy = false;
        let mut neutral = false;
        for &a in g.country(c).adj() {
            let o = g.owner(a);
            if o != NEUTRAL && o != h.cur {
                enemy = true;
                h.noeny = true;
                break;
            }
            if o == NEUTRAL {
                neutral = true;
            }
        }
        if enemy {
            h.primove[c as usize] = 3000;
            build_front(g, h, c, 2000, 0);
        } else if neutral {
            h.primove[c as usize] = 100;
            build_front(g, h, c, 50, 0);
        }
    }
}

fn build_front(g: &Game, h: &mut Hal, c: u8, pri: i32, depth: u32) {
    if depth > 16 {
        return;
    }
    let adj = g.country(c).adj();
    for &a in adj {
        let o = g.owner(a);
        if h.primove[a as usize] < pri && (o == h.cur || o == NEUTRAL) {
            h.primove[a as usize] = pri;
        }
    }
    if adj.is_empty() {
        return;
    }
    let d = adj[0];
    for &b in g.country(d).adj() {
        let o = g.owner(b);
        if h.primove[b as usize] < pri && (o == h.cur || o == NEUTRAL) && pri >= 8 {
            build_front(g, h, d, pri / 2, depth + 1);
        }
    }
}

fn set_move_priority(g: &Game, h: &mut Hal) {
    let sign = if h.aggression { -1 } else { 1 };
    for c in 1..=NUM_COUNTRIES {
        let cty = &g.countries[c];
        if cty.owner == h.cur {
            continue;
        }
        h.primove[c] = cty.value as i32 / g.planet.max(1) as i32;
        if cty.owner != NEUTRAL {
            h.primove[c] += 4000;
        } else {
            h.primove[c] += 100;
        }
        for &s in cty.slots.iter() {
            if s != 0 {
                h.primove[c] += BUILDPRI[(s as usize - 1).min(6)];
            }
        }
        let mut sum = 0;
        for u in cty.units.iter() {
            if u.present() {
                sum += FCYB[u.kind as usize] * sign;
            }
        }
        h.primove[c] += sum;
    }
}

fn set_neutral_priority(g: &mut Game, h: &mut Hal) {
    let mut pri = 5000;
    for k in 0..h.halscty.len() {
        let c = h.halscty[k];
        for &a in g.country(c).adj() {
            if g.owner(a) != NEUTRAL {
                continue;
            }
            if pri <= 2000 {
                return;
            }
            if pri == 5000 {
                g.aneutral[0] = a;
                h.primove[a as usize] = pri;
                pri -= 10;
            } else if a != g.aneutral[0] {
                g.aneutral[1] = a;
                h.primove[a as usize] = pri;
                pri -= 4000;
            }
        }
    }
}

fn hal_repairs(g: &mut Game, h: &mut Hal) {
    for k in 0..h.halscty.len() {
        let c = h.halscty[k];
        for u in 0..10 {
            let unit = g.countries[c as usize].units[u];
            if !unit.present() || unit.status >= 100 {
                continue;
            }
            let cost = (UNIT_COST[unit.kind as usize] as f32 * REPAIR_FACTOR[(unit.status.max(0) / 10).min(9) as usize]) as i64;
            if g.players[h.cur as usize].vault > cost {
                g.players[h.cur as usize].vault -= cost;
                g.countries[c as usize].units[u].status = 100;
            }
        }
    }
}

/// Returns (item, continue).
fn hal_buys(h: &mut Hal) -> (Option<usize>, bool) {
    let level = h.row[20];
    let mut item = None;
    let mut i = 0;
    while i < 17 {
        if h.row[i] == level {
            item = Some(i);
            h.row[i] -= 1;
            break;
        }
        i += 1;
    }
    if i >= 16 {
        h.row[20] -= 1;
        if h.row[20] < 1 {
            return (item, false);
        }
    }
    (item, true)
}

fn locate_capitol(g: &mut Game, h: &mut Hal, money: &mut i64) {
    let mut best = 0;
    let mut pick = 0u8;
    for &c in &h.halscty {
        let cty = g.country(c);
        if cty.value as i32 > best && cty.slots[0] == 0 {
            best = cty.value as i32;
            pick = c;
        }
    }
    if pick == 0 {
        return;
    }
    if *money > CAPITAL_COST as i64 {
        g.countries[pick as usize].slots[0] = SLOT_CAPITAL;
        *money -= CAPITAL_COST as i64;
        h.nobuy = false;
    } else if h.nobuy {
        h.row[20] = -1;
    }
}

fn locate_factory(g: &mut Game, h: &mut Hal, f: usize, money: &mut i64) {
    let mut facplace: Vec<u8> = Vec::new();
    let mut last = 0u8;
    for &c in &h.halscty {
        let cty = g.country(c);
        if cty.free_space() > 0 && cty.has_capital() {
            if facplace.len() < 20 && cty.adj().iter().all(|&a| g.owner(a) == h.cur) {
                facplace.push(c);
            }
            last = c;
        }
    }
    if last == 0 {
        return;
    }
    let cty = if !facplace.is_empty() { facplace[g.rand(0, facplace.len() as i32 - 1) as usize] } else { last };
    h.country1 = cty;
    if *money <= FACTORY_COST[f - 1] as i64 {
        return;
    }
    let c = g.country_mut(cty);
    c.fill_build_area(FACTFILL[f - 1]);
    c.nbays += FACTORY_BAYS[f - 1] as i16;
    *money -= FACTORY_COST[f - 1] as i64;
}

fn locate_battle_unit(g: &mut Game, h: &mut Hal, t: usize, money: &mut i64) {
    let mut pick = 0u8;
    for &c in &h.halscty {
        if g.country(c).free_bays() > 0 {
            pick = c;
            break;
        }
    }
    if pick == 0 {
        return;
    }
    h.country1 = pick;
    let enh = g.country(pick).enhancements();
    let cost = (UNIT_COST[t] as f64 * (1.0 - enh as f64 / 10.0)) as i64;
    let bt = (BUILD_TIME[t] - enh).max(BUILD_TIME_MIN[t]);
    if *money > cost && cost <= h.maxpurchase {
        let c = g.country_mut(pick);
        let nb = c.nbays.max(0) as usize;
        if let Some(b) = c.bays.iter().take(nb).position(|&b| b == EMPTY_BAY) {
            c.bays[b] = (bt * 100 + t as i32) as i16;
            *money -= cost;
        }
    }
}

fn calculate_country_value(g: &Game, cur: u8, c: u8) -> i32 {
    let cty = g.country(c);
    let mut v = 0;
    for &s in cty.slots.iter() {
        v += if s == 0 { CTYVAL_EMPTY } else { CTYVAL[(s as usize - 1).min(6)] };
    }
    if cty.adj().iter().any(|&a| g.owner(a) != NEUTRAL && g.owner(a) != cur) {
        v += 5000;
    }
    v
}

fn locate_fortification(g: &mut Game, h: &mut Hal, money: &mut i64) {
    let mut best = 0;
    let mut pick = 0u8;
    h.flevel = 0;
    for &c in &h.halscty {
        let cty = g.country(c);
        if cty.free_space() <= 0 || !cty.has_capital() {
            continue;
        }
        let v = calculate_country_value(g, h.cur, c);
        if v <= best {
            continue;
        }
        if !cty.units[0].present() {
            continue;
        }
        best = v;
        if v > 500 && v <= 800 {
            if cty.fort >= 1 {
                best = 0;
            } else {
                pick = c;
                h.flevel = 1;
            }
        } else if best > 800 && best <= 1200 {
            if cty.fort >= 2 {
                best = 0;
            } else {
                pick = c;
                h.flevel = 2;
            }
        }
        if best > 1200 {
            if cty.fort == 3 {
                best = 0;
            } else {
                pick = c;
                h.flevel = 3;
            }
        }
    }
    if pick == 0 {
        return;
    }
    h.country1 = pick;
    let mut done = false;
    for _try in 0..3 {
        if h.flevel == 0 {
            break;
        }
        let cost = FORT_COST[h.flevel as usize] as i64;
        if *money > cost && !done {
            let c = g.country_mut(pick);
            if c.fort == 0 {
                c.fill_build_area(SLOT_FORT);
            }
            c.fort = h.flevel;
            *money -= cost;
            done = true;
        } else if h.flevel > 0 && !done {
            h.flevel -= 1;
        }
    }
}

fn locate_enhancement(g: &mut Game, h: &mut Hal, money: &mut i64) {
    let mut pick = 0u8;
    for &c in &h.halscty {
        let cty = g.country(c);
        if cty.free_space() > 0 && cty.has_capital() && cty.enhancements() < 2 && cty.nbays >= 4 {
            pick = c;
        }
    }
    if pick == 0 {
        return;
    }
    h.country1 = pick;
    if *money > ENHANCEMENT_COST as i64 {
        g.country_mut(pick).fill_build_area(SLOT_ENHANCEMENT);
        *money -= ENHANCEMENT_COST as i64;
    }
}

fn hal_moves(g: &mut Game, h: &mut Hal) {
    for k in 0..h.halscty.len() {
        let c = h.halscty[k];
        let movers = check_movement(g, h, c);
        if !movers.is_empty() {
            battle_units_move(g, h, c, &movers);
        }
    }
}

fn find_enemy(g: &Game, cur: u8, c: u8) -> bool {
    g.country(c).adj().iter().any(|&a| g.owner(a) != NEUTRAL && g.owner(a) != cur)
}

fn get_destination(g: &Game, h: &Hal, c: u8) -> u8 {
    let adj = g.country(c).adj();
    let mut dest = adj[0];
    let mut best = h.primove[dest as usize];
    for &a in &adj[1..] {
        if h.primove[a as usize] > best {
            best = h.primove[a as usize];
            dest = a;
        }
    }
    dest
}

fn check_movement(g: &Game, h: &mut Hal, c: u8) -> Vec<usize> {
    let cty = g.country(c);
    let enemy = find_enemy(g, h.cur, c);
    let mut stayhome;
    let mut dest = 0u8;
    if !cty.has_capital() && !enemy {
        stayhome = STAY[h.skill][0];
    } else if cty.has_capital() && !enemy {
        stayhome = STAY[h.skill][1];
    } else {
        dest = get_destination(g, h, c);
        let n = g.country(dest).num_units() as i32;
        h.stayenemy = (FORTFACTOR[g.country(dest).fort as usize] + n).min(9);
        stayhome = h.stayenemy;
    }
    let n = cty.num_units() as i32;
    if enemy && stayhome - 1 <= n && h.skill < 3 {
        if stayhome >= 4 {
            h.blitz = true;
        }
        stayhome = 0;
    } else if enemy && dest != 0 && h.numunits[dest as usize] >= stayhome + 2 && h.skill < 3 {
        h.blitz = true;
        stayhome = 0;
    }
    let mut movers = Vec::new();
    let mut cnt = 0;
    for u in 0..10 {
        if cty.fort != 0 && u == 0 {
            continue;
        }
        if !cty.units[u].present() {
            continue;
        }
        cnt += 1;
        if cnt > stayhome {
            movers.push(u);
        }
    }
    movers
}

fn battle_units_move(g: &mut Game, h: &mut Hal, c: u8, movers: &[usize]) {
    let mut dest = get_destination(g, h, c);
    for &u in movers {
        let o = g.owner(dest);
        if !(h.blitz && o != h.cur && o != NEUTRAL) {
            dest = get_destination(g, h, c);
            if dest == g.aneutral[0] || dest == g.aneutral[1] {
                h.primove[dest as usize] = 0;
            }
        }
        let kind = g.countries[c as usize].units[u].kind;
        if kind != NO_CYBORG {
            h.primove[dest as usize] -= FCYB[kind as usize];
            g.countries[c as usize].units[u].dest = dest;
        }
    }
    h.blitz = false;
}
