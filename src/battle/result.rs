//! Applying a real-time battle's outcome to the strategy state (`_START2`, `_CHECK_PIECES`, city damage).

use super::setup::{count_city_pieces, Structures};
use super::sim::{Piece, Sim};
use crate::data::{ARMOR, NO_CYBORG};
use crate::strategy::resolve::Battle;
use crate::strategy::state::{Game, SLOT_ENHANCEMENT};

/// Map the simulation result back into the Battle (types/hits/victor/retreats) and apply
/// structure and city damage to the defending country. `cp1`/`cp3` are the city piece counts
/// before and after the pre-applied damage.
pub fn apply_result(g: &mut Game, b: &mut Battle, sim: &Sim, structures: &Structures, cp1: i32, cp3: i32) {
    let (res, aret, dret, health) = sim.result();
    for i in 0..20 {
        let m = &sim.mechs[i];
        if !m.exists {
            b.types[i] = NO_CYBORG;
            b.hits[i] = -1;
            continue;
        }
        let kind = m.kind.clamp(0, 8) as u8;
        b.types[i] = kind;
        b.temp[i] = kind;
        b.hits[i] = if health[i] > 100 { -1 } else { health[i] * ARMOR[kind as usize] / 100 };
        if m.status < 0 && m.status != -4 {
            b.hits[i] = -1;
        }
    }
    b.victor = match res {
        2 => 2,
        1 => 1,
        _ => 0,
    };
    b.attacker_retreated = aret;
    b.defender_retreated = dret;
    b.simulated = false;
    // city damage from destroyed buildings
    let def = b.defend_cty;
    if g.country(def).is_city() {
        let cp2 = count_city_pieces(&sim.map);
        let cp2 = cp3 - 3 * (cp3 - cp2);
        if cp1 != cp2 && cp1 > 0 {
            let dmg = (100.0 - cp2 as f64 / cp1 as f64 * 100.0) as i16;
            let c = g.country_mut(def);
            c.damage = dmg.clamp(c.damage.min(100), 100);
        }
    }
    check_pieces(g, b, &sim.pieces, structures);
}

/// `_CHECK_PIECES` + `_ADJUST_DATA_STRUCT`.
pub fn check_pieces(g: &mut Game, b: &mut Battle, pieces: &[Piece], st: &Structures) {
    b.bdlevel = 0;
    let def = b.defend_cty;
    if pieces.first().map_or(false, |p| !p.alive) && g.country(def).has_capital() {
        g.country_mut(def).clear_structures();
        b.bdlevel = 30;
        return;
    }
    let mut first = 1usize;
    for s in 1..7 {
        let v = st.pstruct[s];
        if v == 0 {
            continue;
        }
        let width = if v == SLOT_ENHANCEMENT { 1 } else { (v as i32 - 2).max(1) as usize };
        let mut destroyed = 0;
        for p in first..first + width {
            if p < pieces.len() && !pieces[p].alive {
                destroyed += 1;
                b.bdlevel += 1;
            }
        }
        if destroyed > 0 {
            let c = g.country_mut(def);
            if c.slots[s] == SLOT_ENHANCEMENT {
                c.slots[s] = 0;
            } else {
                if c.slots[s] as i32 - destroyed <= 2 {
                    c.slots[s] = 0;
                } else {
                    c.slots[s] -= destroyed as u8;
                }
                c.nbays = (c.nbays - destroyed as i16).max(0);
            }
        }
        first += width;
    }
}
