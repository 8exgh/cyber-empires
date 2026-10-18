//! End-of-round movement and battle resolution (the original `_SET_ACTION_PHASE` and friends).
//! The phase is a resumable state machine so that the UI can run interactive battles in between.

use super::state::*;
use crate::data::*;
use crate::world_data::NUM_COUNTRIES;

/// Per-cyborg simulation stats (CYBOR `_ST`): long-range fire, short-range fire, range, heat, armour, speed.
pub const ST: [[i32; 8]; 9] = [
    [8, 18, 80, 15, 750, 0, 10, 6],
    [8, 24, 68, 23, 600, 0, 6, 5],
    [0, 19, 75, 23, 390, 0, 4, 7],
    [0, 18, 69, 21, 270, 10, 3, 4],
    [16, 2, 68, 10, 150, 10, 3, 6],
    [0, 8, 60, 7, 150, 10, 3, 2],
    [0, 1, 23, 21, 90, 20, 2, 3],
    [0, 4, 28, 3, 90, 20, 2, 1],
    [0, 1, 28, 3, 30, 20, 2, 2],
];

/// A battle between the units of `attack_cty` plotted into `defend_cty` and everything in `defend_cty`.
#[derive(Clone, Debug)]
pub struct Battle {
    pub attack_cty: u8,
    pub defend_cty: u8,
    /// [0..10] attacker types, [10..20] defender types (10 = none), compacted to the front
    pub types: [u8; 20],
    /// armour tons left, -1 = none
    pub hits: [i32; 20],
    /// copy of the types made before the fight (survives destruction)
    pub temp: [u8; 20],
    /// -1 none/draw, 1 defender won, 2 attacker won
    pub victor: i8,
    pub attacker_retreated: bool,
    pub defender_retreated: bool,
    pub att_loss: i32,
    pub def_loss: i32,
    /// structures destroyed count (30 = capital)
    pub bdlevel: i32,
    pub old_city_damage: i16,
    pub simulated: bool,
    /// owners at the time the battle was loaded (ownership may change afterwards)
    pub att_owner: u8,
    pub def_owner: u8,
}

impl Battle {
    pub fn attacker_owner(&self, _g: &Game) -> u8 {
        self.att_owner
    }
    pub fn defender_owner(&self, _g: &Game) -> u8 {
        self.def_owner
    }
    pub fn attackers(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        (0..10).filter(|&i| self.types[i] != NO_CYBORG).map(move |i| (i, self.types[i]))
    }
    pub fn defenders(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        (10..20).filter(|&i| self.types[i] != NO_CYBORG).map(move |i| (i, self.types[i]))
    }
    pub fn count_lost(&self) -> (i32, i32) {
        let mut a = 0;
        let mut d = 0;
        for i in 0..20 {
            if self.hits[i] < 0 && self.temp[i] != NO_CYBORG {
                if i < 10 {
                    a += 1;
                } else {
                    d += 1;
                }
            }
        }
        (a, d)
    }
    /// Verb for the summary ("HAS DEFEATED" / "HAS CRUSHED" / "HAS DECIMATED").
    pub fn summary_verb(&self) -> &'static str {
        let (a, d) = self.count_lost();
        let delta = if self.victor == 2 { d - a } else { a - d };
        if delta < 3 {
            " HAS DEFEATED "
        } else if delta <= 5 {
            " HAS CRUSHED "
        } else {
            " HAS DECIMATED "
        }
    }
}

/// hits -> status % (CALCULATE_DAMAGE)
pub fn calculate_damage(hits: i32, kind: u8) -> i16 {
    let hl = ARMOR[kind as usize];
    if hl - 2 < hits {
        100
    } else {
        ((hits as f32) / (hl as f32 / 100.0)) as i16
    }
}

fn hits_left(cty: &Country, i: usize) -> i32 {
    let u = &cty.units[i];
    if !u.present() {
        return 0;
    }
    ((ARMOR[u.kind as usize] as f64) * (u.status as f64 / 100.0)) as i32
}

/// Events the UI must handle while the phase runs.
#[derive(Debug)]
pub enum PhaseEvent {
    /// A battle involving a human (complete campaign): the UI decides SIM or real-time, then calls `finish_battle`.
    Battle(Battle),
    /// A simulated battle just happened (for sound/summary purposes).
    Simulated(Battle),
    Done,
}

#[derive(Clone, Debug)]
pub struct ActionPhase {
    alist: Vec<(u8, u8, bool)>,
    clist: Vec<u8>,
    moved: bool,
    battled: bool,
    cur_src: u8,
    cur_idx: usize,
    started: bool,
    finished: bool,
    pub any_victor: bool,
}

impl ActionPhase {
    pub fn new() -> ActionPhase {
        ActionPhase { alist: Vec::new(), clist: Vec::new(), moved: true, battled: true, cur_src: 0, cur_idx: 0, started: false, finished: false, any_victor: false }
    }

    /// Advance the phase. Returns the next event; `Done` when the whole phase (including SET_OWNERS) is complete.
    pub fn step(&mut self, g: &mut Game) -> PhaseEvent {
        if self.finished {
            return PhaseEvent::Done;
        }
        if !self.started {
            self.started = true;
            for c in 1..=NUM_COUNTRIES {
                g.countries[c].attacked_by = NEUTRAL;
            }
            g.sounds = [0; 5];
            g.lastrowy = g.testrowy;
        }
        loop {
            if self.clist.is_empty() {
                if !(self.moved || self.battled) {
                    // phase complete
                    return_stranded(g);
                    set_owners(g);
                    check_players(g);
                    self.finished = true;
                    return PhaseEvent::Done;
                }
                self.moved = false;
                self.battled = false;
                self.peaceful_moves(g);
                self.load_alist(g);
                if self.clist.is_empty() {
                    continue;
                }
            }
            // pick a random attacking country and its first live record
            let pick = g.rand(0, self.clist.len() as i32 - 1) as usize;
            let src = self.clist[pick];
            self.cur_src = src;
            let idx = match self.alist.iter().position(|r| !r.2 && r.0 == src) {
                Some(i) => i,
                None => {
                    self.update_clist(src);
                    continue;
                }
            };
            self.cur_idx = idx;
            let (s, t, _) = self.alist[idx];
            let real = battles_confirmed(g, s, t) && g.owner(s) != g.owner(t);
            if !real {
                self.mark_done(g);
                continue;
            }
            let battle = load_parties(g, s, t);
            let att_c = g.is_computer(g.owner(s));
            let def_c = g.is_computer(g.owner(t));
            if !g.complete_campaign || (att_c && def_c) {
                let b = simulate_battle(g, battle);
                let b = self.apply_outcome(g, b);
                self.mark_done(g);
                return PhaseEvent::Simulated(b);
            }
            return PhaseEvent::Battle(battle);
        }
    }

    /// Called by the UI with the fought (or SIM-resolved) battle.
    pub fn finish_battle(&mut self, g: &mut Game, b: Battle) -> Battle {
        let b = self.apply_outcome(g, b);
        self.mark_done(g);
        b
    }

    fn mark_done(&mut self, _g: &mut Game) {
        if self.cur_idx < self.alist.len() {
            self.alist[self.cur_idx] = (0, 0, true);
        }
        let src = self.cur_src;
        self.update_clist(src);
        self.battled = true;
    }

    fn update_clist(&mut self, src: u8) {
        let live = self.alist.iter().any(|r| !r.2 && r.0 == src);
        if !live {
            if let Some(i) = self.clist.iter().position(|&c| c == src) {
                self.clist.swap_remove(i);
            }
        }
    }

    fn apply_outcome(&mut self, g: &mut Game, mut b: Battle) -> Battle {
        let (s, t) = (b.attack_cty, b.defend_cty);
        let ao = g.owner(s);
        let d_o = g.owner(t);
        match b.victor {
            2 => {
                if b.defender_retreated {
                    defender_retreat(g, &b);
                }
                place_victor(g, &b, true);
                if d_o < 5 {
                    g.sounds[d_o as usize] -= 1;
                }
                if ao < 5 {
                    g.sounds[ao as usize] += 1;
                }
                g.countries[t as usize].owner = ao;
                g.countries[t as usize].attacked_by = ao;
                self.any_victor = true;
            }
            1 => {
                if b.attacker_retreated {
                    attacker_retreat(g, &b);
                }
                g.countries[t as usize].attacked_by = ao;
                if ao < 5 {
                    g.sounds[ao as usize] -= 1;
                }
                place_victor(g, &b, false);
                self.any_victor = true;
            }
            _ => {
                if b.attacker_retreated {
                    attacker_retreat(g, &b);
                }
                if b.defender_retreated {
                    defender_retreat(g, &b);
                }
                reset_destin(g, s, t);
            }
        }
        b.simulated |= false;
        b
    }

    /// 8 passes of peaceful moves (same owner or into neutral countries).
    fn peaceful_moves(&mut self, g: &mut Game) {
        for _pass in 0..8 {
            for c in 1..=NUM_COUNTRIES as u8 {
                if g.owner(c) == NEUTRAL {
                    continue;
                }
                for i in 0..10 {
                    let u = g.countries[c as usize].units[i];
                    if u.dest == c || u.dest == 0 {
                        continue;
                    }
                    let d = u.dest;
                    if g.owner(d) == g.owner(c) || g.owner(d) == NEUTRAL {
                        if friendly_country(g, d) && unit_moves(g, c, i) {
                            self.moved = true;
                        }
                    }
                }
            }
        }
    }

    fn load_alist(&mut self, g: &mut Game) {
        self.alist.clear();
        self.clist.clear();
        for c in 1..=NUM_COUNTRIES as u8 {
            let mut tlist: Vec<u8> = Vec::new();
            let mut any = false;
            for i in 0..10 {
                let u = g.countries[c as usize].units[i];
                if u.dest == c || u.dest == 0 {
                    continue;
                }
                let d = u.dest;
                if g.owner(d) == g.owner(c) {
                    continue;
                }
                if !u.present() {
                    g.countries[c as usize].units[i].dest = c;
                    continue;
                }
                any = true;
                if !tlist.contains(&d) {
                    tlist.push(d);
                }
            }
            for &t in &tlist {
                self.alist.push((c, t, false));
            }
            if any {
                self.clist.push(c);
            }
        }
    }
}

impl Default for ActionPhase {
    fn default() -> Self {
        ActionPhase::new()
    }
}

/// No unit of `d` is leaving towards a country of another owner.
pub fn friendly_country(g: &Game, d: u8) -> bool {
    let cty = g.country(d);
    for u in cty.units.iter() {
        let t = u.dest;
        if t != d && t != 0 && g.owner(t) != cty.owner {
            return false;
        }
    }
    true
}

/// Execute one plotted peaceful move (also captures a neutral country without a fight).
pub fn unit_moves(g: &mut Game, c: u8, i: usize) -> bool {
    let u = g.countries[c as usize].units[i];
    let d = u.dest;
    let owner = g.owner(c);
    g.countries[d as usize].owner = owner;
    if let Some(k) = g.countries[d as usize].free_unit_slot() {
        g.countries[d as usize].units[k] = Unit { kind: u.kind, status: u.status, dest: d };
        g.countries[c as usize].units[i] = Unit { kind: NO_CYBORG, status: 100, dest: c };
        true
    } else {
        false
    }
}

/// Undefended target → captured and razed. Returns true when a real battle must be fought.
pub fn battles_confirmed(g: &mut Game, s: u8, t: u8) -> bool {
    let mut attackers = false;
    let mut defenders = false;
    for i in 0..10 {
        let su = g.countries[s as usize].units[i];
        if su.present() && su.dest == t {
            attackers = true;
        }
        if g.countries[t as usize].units[i].present() {
            defenders = true;
        }
    }
    if !defenders && attackers {
        let owner = g.owner(s);
        g.countries[t as usize].clear_structures();
        g.countries[t as usize].owner = owner;
        for i in 0..10 {
            let su = g.countries[s as usize].units[i];
            if su.dest == t && su.present() {
                g.countries[t as usize].units[i].kind = su.kind;
                g.countries[t as usize].units[i].status = su.status;
                g.countries[s as usize].units[i].kind = NO_CYBORG;
            }
        }
    }
    attackers && defenders
}

/// LOAD_ATTACK + LOAD_DEFEND + SORT_ACTIONLIST: removes the units from the map into a Battle.
pub fn load_parties(g: &mut Game, s: u8, t: u8) -> Battle {
    let mut b = Battle {
        attack_cty: s,
        defend_cty: t,
        types: [NO_CYBORG; 20],
        hits: [0; 20],
        temp: [NO_CYBORG; 20],
        victor: -1,
        attacker_retreated: false,
        defender_retreated: false,
        att_loss: 0,
        def_loss: 0,
        bdlevel: 0,
        old_city_damage: g.country(t).damage,
        simulated: false,
        att_owner: g.owner(s),
        def_owner: g.owner(t),
    };
    for i in 0..10 {
        let su = g.countries[s as usize].units[i];
        if su.present() && su.dest == t {
            b.types[i] = su.kind;
            b.hits[i] = hits_left(&g.countries[s as usize], i);
            g.countries[s as usize].units[i] = Unit { kind: NO_CYBORG, status: 100, dest: s };
        }
    }
    for i in 0..10 {
        let du = g.countries[t as usize].units[i];
        if du.present() {
            b.types[10 + i] = du.kind;
            b.hits[10 + i] = hits_left(&g.countries[t as usize], i);
        }
        g.countries[t as usize].units[i].kind = NO_CYBORG;
        g.countries[t as usize].units[i].status = 100;
    }
    sort_actionlist(&mut b, 0, 10);
    sort_actionlist(&mut b, 10, 20);
    b
}

fn sort_actionlist(b: &mut Battle, lo: usize, hi: usize) {
    for i in lo..hi {
        if b.types[i] == NO_CYBORG {
            if let Some(j) = (i + 1..hi).find(|&j| b.types[j] != NO_CYBORG) {
                b.types[i] = b.types[j];
                b.hits[i] = b.hits[j];
                b.types[j] = NO_CYBORG;
            }
        }
        b.temp[i] = b.types[i];
    }
    for i in lo..hi {
        if b.types[i] == NO_CYBORG {
            b.hits[i] = -1;
        }
    }
}

pub fn heat_factor(terrain: u8) -> i32 {
    match terrain {
        1 => 1,
        2 | 5 => 2,
        _ => 3,
    }
}

/// The original strategic battle simulation.
pub fn simulate_battle(g: &mut Game, mut b: Battle) -> Battle {
    b.victor = -1;
    b.simulated = true;
    let terrain = g.country(b.defend_cty).terrain();
    let hf = heat_factor(terrain);
    let close = terrain == 2 || terrain == 5;
    let mut het = [0i32; 2];
    let mut pow = [0i32; 2];
    let mut ran = [0i32; 2];
    let mut spd = [0i32; 2];
    let mut ton = [0i32; 2];
    let mut n = [0i32; 2];
    for i in 0..20 {
        let t = b.types[i];
        if t == NO_CYBORG {
            continue;
        }
        let side = if i < 10 { 0 } else { 1 };
        let st = &ST[t as usize];
        het[side] += st[3];
        pow[side] += if close { st[1] } else { st[0] + st[1] };
        ran[side] += st[2];
        spd[side] += st[5];
        ton[side] += b.hits[i];
        n[side] += 1;
    }
    for s in 0..2 {
        het[s] *= hf;
        pow[s] = (pow[s] + het[s]) / 4;
        if n[s] > 0 {
            ran[s] /= n[s];
            spd[s] /= n[s];
        }
    }
    let (att0, def0) = (ton[0], ton[1]);
    match g.country(b.defend_cty).fort {
        1 => pow[1] = (pow[1] as f64 * 1.5) as i32,
        2 => pow[1] *= 2,
        3 => pow[1] *= 4,
        _ => {}
    }
    let (mut att_ton, mut def_ton) = (att0, def0);
    let mut guard = 0;
    while att_ton > 0 && def_ton > 0 && guard < 100_000 {
        guard += 1;
        if g.rand(1, 100) < ran[0] - spd[1] {
            def_ton -= g.rand(pow[0] / 4, pow[0]);
        }
        if g.rand(1, 100) < ran[1] - spd[0] {
            att_ton -= g.rand(pow[1] / 4, pow[1]);
        }
        if pow[0] <= 0 && pow[1] <= 0 {
            break;
        }
    }
    if terrain == 5 {
        // DAMAGE_CITY
        let mut shots = 30;
        let mut dmg = 0i64;
        let p = pow[0] + pow[1];
        while shots > 0 {
            dmg += g.rand(p / 4, p) as i64;
            shots -= 1;
        }
        let cty = g.country_mut(b.defend_cty);
        cty.damage += ((dmg as f64) / 20000.0 * 100.0) as i16;
        if cty.damage > 100 {
            cty.damage = 100;
        }
    }
    if att_ton <= 0 && def_ton <= 0 {
        // mutual annihilation
        for i in 0..20 {
            b.hits[i] = -1;
        }
        return b;
    }
    if att_ton > 0 {
        b.victor = 2;
        let mut loss = att0 - att_ton;
        if loss + 6 > att0 {
            loss -= 6;
        }
        b.att_loss = loss;
        deduct_damage(g, &mut b, loss, 0, 10);
    } else {
        b.victor = 1;
        let mut loss = def0 - def_ton;
        if loss + 6 > def0 {
            loss -= 6;
        }
        b.def_loss = loss;
        deduct_damage(g, &mut b, loss, 10, 0);
    }
    b
}

fn deduct_damage(g: &mut Game, b: &mut Battle, mut loss: i32, win_base: usize, lose_base: usize) {
    let mut guard = 0;
    while loss > 0 && guard < 10_000 {
        guard += 1;
        let alive = (win_base..win_base + 10).filter(|&u| b.types[u] != NO_CYBORG && b.hits[u] > 0).count();
        if alive == 0 {
            break;
        }
        let u = g.rand(win_base as i32, win_base as i32 + 9) as usize;
        if b.types[u] == NO_CYBORG || b.hits[u] <= 0 {
            continue;
        }
        let mut tries = 15;
        while tries > 0 && loss > 0 {
            let d = g.rand(3, 6);
            tries -= 1;
            b.hits[u] -= d;
            loss -= d;
            if b.hits[u] < 0 {
                b.types[u] = NO_CYBORG;
                tries = 0;
            }
        }
    }
    for u in lose_base..lose_base + 10 {
        b.hits[u] = -1;
    }
}

pub fn place_victor(g: &mut Game, b: &Battle, attacker_won: bool) {
    let t = b.defend_cty;
    if attacker_won {
        g.countries[t as usize].clear_structures();
        for i in 0..10 {
            if b.hits[i] > 0 && b.temp[i] != NO_CYBORG {
                let u = &mut g.countries[t as usize].units[i];
                u.kind = b.temp[i];
                u.dest = t;
                u.status = calculate_damage(b.hits[i], b.temp[i]);
            }
        }
    } else {
        for i in 10..20 {
            if b.hits[i] > 0 && b.temp[i] != NO_CYBORG {
                let u = &mut g.countries[t as usize].units[i - 10];
                u.kind = b.temp[i];
                u.status = calculate_damage(b.hits[i], b.temp[i]);
            }
        }
    }
    reset_destin(g, b.attack_cty, t);
}

fn reset_destin(g: &mut Game, s: u8, t: u8) {
    for i in 0..10 {
        if !g.countries[t as usize].units[i].present() {
            g.countries[t as usize].units[i].dest = t;
        }
        if !g.countries[s as usize].units[i].present() {
            g.countries[s as usize].units[i].dest = s;
        }
    }
}

pub fn attacker_retreat(g: &mut Game, b: &Battle) {
    let s = b.attack_cty;
    for u in 0..10 {
        if b.hits[u] > 0 && b.temp[u] != NO_CYBORG {
            if let Some(i) = g.countries[s as usize].free_unit_slot() {
                g.countries[s as usize].units[i] = Unit { kind: b.temp[u], status: calculate_damage(b.hits[u], b.temp[u]), dest: s };
            }
        }
    }
}

pub fn defender_retreat(g: &mut Game, b: &Battle) {
    let t = b.defend_cty;
    let owner = g.owner(t);
    for u in 10..20 {
        if b.hits[u] > 0 && b.temp[u] != NO_CYBORG {
            for &a in g.country(t).adj() {
                if g.owner(a) == owner {
                    if let Some(i) = g.countries[a as usize].free_unit_slot() {
                        g.countries[a as usize].units[i] = Unit { kind: b.temp[u], status: calculate_damage(b.hits[u], b.temp[u]), dest: a };
                        break;
                    }
                }
            }
        }
    }
}

pub fn return_stranded(g: &mut Game) {
    for c in 1..=NUM_COUNTRIES {
        for u in g.countries[c].units.iter_mut() {
            u.dest = c as u8;
        }
    }
}

/// Countries without government and without units revert to neutral and are razed.
pub fn set_owners(g: &mut Game) {
    for c in 1..=NUM_COUNTRIES {
        if g.countries[c].has_capital() {
            continue;
        }
        if g.countries[c].num_units() == 0 {
            g.countries[c].owner = NEUTRAL;
            g.countries[c].clear_structures();
        }
    }
}

/// `_CHECK_PLAYERS`: testrowy[p] = player type if p owns a country, else NONE.
pub fn check_players(g: &mut Game) {
    let mut t = [0x48i16; 5];
    for c in 1..=NUM_COUNTRIES {
        let o = g.countries[c].owner;
        if o < 5 {
            t[o as usize] = g.players[o as usize].ptype.code();
        }
    }
    g.testrowy = t;
}
