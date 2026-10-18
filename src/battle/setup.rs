//! Composing a battlefield for a strategy battle (CYBOR `_ACTION_SETUP`, `_INSERT_STRUCTURES`, `_SET_DEF_POS`...).

use super::map::{BattleMap, MAP_H, MAP_W};
use super::sim::{BattleInit, MechInit, Piece};
use super::tables::*;
use crate::assets::Assets;
use crate::data::{ARMOR, NO_CYBORG};
use crate::rng::Rng;
use crate::strategy::resolve::Battle;
use crate::strategy::state::Game;

/// How the defender's cyborgs are placed.
#[derive(Clone, Debug)]
pub enum DefenderPlacement {
    /// computer defender: AP positions, light units fall back behind the wall
    Ai,
    /// AUTO button: AP positions in arrangement order
    Auto,
    /// manual UI positions (minimap pixels) per occupied defender slot, in slot order
    Manual(Vec<(i32, i32)>),
}

#[derive(Clone, Debug)]
pub struct SetupParams {
    pub adir: usize,
    pub att_goal: usize,
    pub att_agg: usize,
    pub att_control: usize,
    pub def_control: usize,
    pub human: [bool; 2],
    pub placement: DefenderPlacement,
}

/// Result of structure insertion: pieces, which build slot each piece group belongs to, turret tiles.
#[derive(Clone, Debug, Default)]
pub struct Structures {
    pub pieces: Vec<Piece>,
    pub pstruct: [u8; 7],
    pub turrets: Vec<(i32, i32)>,
    pub turrett: u8,
}

fn buildpos(k: usize) -> (i32, i32) {
    let v = BUILDPOS[k] as u16;
    ((v >> 8) as i32, (v & 0xFF) as i32) // (col, row)
}

/// `_INSERT_STRUCTURES`: stamp the defender's structures into the map.
pub fn insert_structures(map: &mut BattleMap, slots: &[u8; 7], owner: usize, fort_level: u8, adir: usize, is_city: bool) -> Structures {
    let mut st = Structures::default();
    let own = owner.min(4);
    let (mut xo, mut yo) = (BO[adir * 2] as i32, BO[adir * 2 + 1] as i32);
    let notcity = !is_city;
    if is_city {
        xo = -1;
        yo = 1;
    }
    for k in 0..7 {
        st.pstruct[k] = 0;
        let v = slots[k];
        let (bx, by) = buildpos(k);
        let x0 = bx + xo;
        let y0 = by + yo;
        match v {
            1 => {
                let mut i = 0;
                for y in y0..y0 + 7 {
                    for x in 0..11 {
                        let t = CAPBUILD[own * 77 + i] as u8;
                        i += 1;
                        if notcity || t < 120 {
                            map.set(x0 + x, y, t);
                        }
                        if (75..=79).contains(&t) {
                            st.pieces.push(Piece { x: (x0 + x) * 32, y: y * 32, alive: true });
                        }
                    }
                }
            }
            3..=6 => {
                let f = (v - 3) as usize;
                let cols = FACP[f * 4] as i32;
                let stride = FACP[f * 4 + 1] as usize;
                let rows = FACP[f * 4 + 2] as i32;
                let base = FACP[f * 4 + 3] as usize + own * stride;
                let mut i = 0;
                for y in y0..y0 + rows {
                    for x in 0..cols {
                        let idx = base + i;
                        i += 1;
                        if idx >= FAC.len() {
                            continue;
                        }
                        let t = FAC[idx] as u8;
                        map.set(x0 + x, y, t);
                        if v == 5 && FAC3[own * 2] as usize == idx {
                            map.set(x0 + x, y, FAC3[own * 2 + 1] as u8);
                        }
                        if (71..=74).contains(&t) {
                            st.pieces.push(Piece { x: (x0 + x) * 32, y: y * 32, alive: true });
                            st.pstruct[k] = v;
                        }
                    }
                }
            }
            7 => {
                let mut i = 0;
                for y in y0..y0 + 7 {
                    for x in 0..7 {
                        let t = ENHANCE[own * 49 + i] as u8;
                        i += 1;
                        map.set(x0 + x, y, t);
                        if t == 70 {
                            st.pieces.push(Piece { x: (x0 + x) * 32, y: y * 32, alive: true });
                            st.pstruct[k] = 7;
                        }
                    }
                }
            }
            2 => {
                let l = fort_level.clamp(1, 3) as usize;
                let off = (l - 1) * 22;
                st.turrett = fort_level;
                let hw = FORT[off] as u8;
                let vw = FORT[off + 1] as u8;
                for x in 24..58 {
                    if !(37..=43).contains(&x) {
                        map.set(x + xo, yo + 24, hw);
                        map.set(x + xo, yo + 52, hw);
                    }
                    if x > 34 && x < 46 {
                        map.set(x + xo, yo + 22, hw);
                        map.set(x + xo, yo + 54, hw);
                    }
                }
                for y in 25..52 {
                    if !(35..=40).contains(&y) {
                        map.set(xo + 58, y + yo, vw);
                        map.set(xo + 23, y + yo, vw);
                    }
                    if y > 32 && y < 43 {
                        map.set(xo + 60, y + yo, vw);
                        map.set(xo + 21, y + yo, vw);
                    }
                }
                for j in 0..20 {
                    let p = FORTPOS[j] as i32;
                    let (px, py) = (p % 80, p / 80);
                    map.set(px + xo, py + yo, FORT[off + 2 + j] as u8);
                }
                for j in 0..12 {
                    let tx = TURCORDS[adir * 24 + j * 2] as i32 + xo;
                    let ty = TURCORDS[adir * 24 + j * 2 + 1] as i32 + yo;
                    st.turrets.push((tx, ty));
                }
            }
            _ => {}
        }
    }
    st
}

/// Count intact city building tiles (32..59) inside the playfield.
pub fn count_city_pieces(map: &BattleMap) -> i32 {
    let mut n = 0;
    for x in 3..77 {
        for y in 3..75 {
            let t = map.get(x, y);
            if (32..=59).contains(&t) {
                n += 1;
            }
        }
    }
    n
}

/// Pre-apply the city's damage percentage to the map (`_DAMAGE_CITY_PIECES`).
pub fn damage_city_pieces(map: &mut BattleMap, rng: &mut Rng, damage: i32) {
    if damage <= 0 {
        return;
    }
    let count = count_city_pieces(map);
    let mut n = (damage as f64 * 0.01 * count as f64) as i32;
    let mut guard = 0;
    while n > 0 && guard < 20 {
        guard += 1;
        for x in 3..77 {
            for y in 3..75 {
                let t = map.get(x, y);
                if (32..=59).contains(&t) && n > 0 && rng.range(0, 99) < damage {
                    map.set(x, y, WALLCITY_STRAT[t as usize]);
                    n -= 1;
                } else if (3..=31).contains(&t) && n > 0 && rng.range(0, 99) < damage {
                    map.set(x, y, WALLCITY_STRAT[t as usize]);
                }
            }
        }
    }
}

/// `_SET_POSITIONS`: 10 tile positions along the edge for one side.
pub fn edge_positions(adir: usize, dir: usize, use_dir: bool) -> Vec<(i32, i32)> {
    let mut off = (AOFF[adir * 2] as i32, AOFF[adir * 2 + 1] as i32);
    let d = if use_dir { dir } else { 0 };
    if use_dir {
        off = (AOFF[dir * 2] as i32, AOFF[dir * 2 + 1] as i32);
    }
    let bi = (adir + d).min(3);
    let base = (APOS[bi * 2] as i32, APOS[bi * 2 + 1] as i32);
    let step0 = (AOFF[bi * 2] as i32, AOFF[bi * 2 + 1] as i32);
    let mut out = Vec::new();
    for k in 0..10 {
        let p = if k == 0 {
            base
        } else if off.0 > 0 || off.1 > 0 {
            let p = (base.0 + off.0, base.1 + off.1);
            off = (-off.0, -off.1);
            p
        } else {
            let p = (base.0 + off.0, base.1 + off.1);
            off = (-(off.0 - step0.0), -(off.1 - step0.1));
            p
        };
        out.push(p);
    }
    out
}

fn passable(t: u8, ttype: usize, special: i32) -> bool {
    let lo = LP[ttype * 2] as i32;
    let hi = LP[ttype * 2 + 1] as i32;
    let t = t as i32;
    (lo < t && t < hi) || t == 0 || t == special
}

/// `_SET_DEF_POS`: convert defender UI positions (minimap pixels) to free tiles.
pub fn resolve_defender_tiles(map: &BattleMap, ui: &[(i32, i32)], adir: usize, ttype: usize) -> Vec<(i32, i32)> {
    let (mut xo, mut yo) = (BO[adir * 2] as i32, BO[adir * 2 + 1] as i32);
    let mut special = 0;
    if ttype == 4 {
        xo = 0;
        yo = 0;
        special = 118;
    }
    let mut placed: Vec<(i32, i32)> = Vec::new();
    for &(ux, uy) in ui {
        let mut x = (ux - 17) / 2 + xo + 10;
        let mut y = (uy - 53) / 2 + yo + 9;
        let clear_at = |x: i32, y: i32, placed: &Vec<(i32, i32)>| placed.iter().all(|&(ex, ey)| (x - ex).abs() > 1 || (y - ey).abs() > 1);
        let mut clear = clear_at(x, y, &placed);
        let mut t = map.get(x, y);
        let mut k = 0usize;
        let mut r = 1;
        let mut guard = 0;
        while (!clear || !passable(t, ttype, special)) && guard < 2000 {
            guard += 1;
            let sx = SW[k * 2] as i32 * r;
            let sy = SW[k * 2 + 1] as i32 * r;
            x += sx;
            y += sy;
            clear = clear_at(x, y, &placed);
            t = map.get(x, y);
            if !passable(t, ttype, special) || x < 3 || x > 76 || y < 3 || y > 74 {
                x -= sx;
                y -= sy;
                clear = false;
            }
            t = map.get(x, y);
            k += 1;
            if k > 7 {
                r += 1;
                k = 0;
            }
        }
        placed.push((x, y));
    }
    placed
}

/// Compose everything for a strategy battle.
pub fn compose_battle(assets: &mut Assets, game: &Game, b: &Battle, rng: &mut Rng, params: &SetupParams) -> Option<BattleInit> {
    let def = game.country(b.defend_cty);
    let terrain = def.terrain();
    let ttype = (terrain as usize).saturating_sub(1).min(4);
    let mut map = BattleMap::load(assets, terrain, b.defend_cty)?;
    let adir = params.adir.min(3);
    let st = insert_structures(&mut map, &def.slots, def.owner.min(4) as usize, def.fort, adir, terrain == 5);
    if terrain == 5 {
        damage_city_pieces(&mut map, rng, def.damage as i32);
    }
    let mut mechs = [MechInit { kind: -1, ..Default::default() }; 36];
    let att_pos = edge_positions(adir, 0, false);
    for i in 0..10 {
        if b.types[i] != NO_CYBORG && b.hits[i] > 0 {
            let kind = b.types[i] as i32;
            mechs[i] = MechInit {
                x: att_pos[i].0 * 32 + 16,
                y: att_pos[i].1 * 32 + 16,
                kind,
                health: (b.hits[i] * 100 / ARMOR[kind as usize]).clamp(1, 100),
                dir: ((adir * 2 + 4) % 8) as i32,
                side: 0,
            };
        }
    }
    // defender UI positions
    let occupied: Vec<usize> = (10..20).filter(|&i| b.types[i] != NO_CYBORG && b.hits[i] > 0).collect();
    let mut ui: Vec<(i32, i32)> = Vec::new();
    match &params.placement {
        DefenderPlacement::Ai => {
            for &i in &occupied {
                let j = i - 10;
                let mut p = (AP[adir * 20 + j * 2] as i32, AP[adir * 20 + j * 2 + 1] as i32);
                let k = b.types[i];
                if st.turrett != 0 && k != 0 && k != 1 && k != 4 {
                    p.0 += BACKUP[adir * 2] as i32;
                    p.1 += BACKUP[adir * 2 + 1] as i32;
                }
                ui.push(p);
            }
        }
        DefenderPlacement::Auto => {
            for (j, _) in occupied.iter().enumerate() {
                ui.push((AP[adir * 20 + j * 2] as i32, AP[adir * 20 + j * 2 + 1] as i32));
            }
        }
        DefenderPlacement::Manual(v) => {
            ui = v.clone();
            while ui.len() < occupied.len() {
                let j = ui.len();
                ui.push((AP[adir * 20 + j * 2] as i32, AP[adir * 20 + j * 2 + 1] as i32));
            }
        }
    }
    let tiles = resolve_defender_tiles(&map, &ui, adir, ttype);
    for (n, &i) in occupied.iter().enumerate() {
        let kind = b.types[i] as i32;
        mechs[i] = MechInit {
            x: tiles[n].0 * 32 + 16,
            y: tiles[n].1 * 32 + 16,
            kind,
            health: (b.hits[i] * 100 / ARMOR[kind as usize]).clamp(1, 100),
            dir: (adir * 2) as i32,
            side: 1,
        };
    }
    // turrets
    for (j, &(tx, ty)) in st.turrets.iter().enumerate().take(12) {
        let t = TURT[(st.turrett as usize).min(3) * 12 + j];
        if t < 0 {
            continue;
        }
        mechs[20 + j] = MechInit { x: tx * 32 + 16, y: ty * 32 + 16, kind: t as i32, health: 100, dir: (adir * 2) as i32, side: 1 };
    }
    let mut pieces = st.pieces.clone();
    pieces.truncate(26);
    let att_cur = if mechs[params.att_control.min(9)].kind >= 0 { params.att_control.min(9) } else { (0..10).find(|&i| mechs[i].kind >= 0).unwrap_or(0) };
    let dc = 10 + params.def_control.min(9);
    let def_cur = if mechs[dc].kind >= 0 { dc } else { (10..20).find(|&i| mechs[i].kind >= 0).unwrap_or(10) };
    Some(BattleInit {
        map,
        ttype,
        mechs,
        pieces,
        att_goal: params.att_goal,
        att_agg: params.att_agg,
        human: params.human,
        cur: [att_cur, def_cur],
        pc: [b.att_owner.min(4), b.def_owner.min(4)],
    })
}

/// Compose a battle-practice battle: bare terrain map of `country`, attackers along the top, defenders along the bottom.
pub fn compose_practice(assets: &mut Assets, terrain: u8, country: u8, att: &[(u8, i32)], def: &[(u8, i32)], human: [bool; 2], pc: [u8; 2]) -> Option<BattleInit> {
    let map = BattleMap::load(assets, terrain, country)?;
    let ttype = (terrain as usize).saturating_sub(1).min(4);
    let mut mechs = [MechInit { kind: -1, ..Default::default() }; 36];
    let ap = edge_positions(0, 0, true);
    let dp = edge_positions(0, 2, true);
    for (i, &(k, h)) in att.iter().enumerate().take(10) {
        mechs[i] = MechInit { x: ap[i].0 * 32 + 16, y: ap[i].1 * 32 + 16, kind: k as i32, health: h, dir: 4, side: 0 };
    }
    for (i, &(k, h)) in def.iter().enumerate().take(10) {
        mechs[10 + i] = MechInit { x: dp[i].0 * 32 + 16, y: dp[i].1 * 32 + 16, kind: k as i32, health: h, dir: 0, side: 1 };
    }
    Some(BattleInit { map, ttype, mechs, pieces: Vec::new(), att_goal: 0, att_agg: 2, human, cur: [0, 10], pc })
}

pub const _MAP_DIMS: (usize, usize) = (MAP_W, MAP_H);
