//! Player operations: building, moving, repairing, spying (CYBOR BUILD_OP.C / MOVER.C / VAL2.C).

use super::state::*;
use crate::data::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    NotOwner,
    NoCapital,
    NoSpace,
    NoMoney(i64),
    NotAdjacent,
    Full,
    NoFactory,
    AlreadyBuilt,
    CityCannotFortify,
    MaxFort,
    MaxEnhancements,
    NoBay,
}

impl Game {
    pub fn vault(&self, p: u8) -> i64 {
        self.players[p as usize].vault
    }

    // ------------------------------------------------------------------ building
    pub fn buy_capital(&mut self, p: u8, c: u8) -> Result<(), RuleError> {
        let cty = self.country(c);
        if cty.owner != p {
            return Err(RuleError::NotOwner);
        }
        if cty.has_capital() {
            return Err(RuleError::AlreadyBuilt);
        }
        if self.vault(p) < CAPITAL_COST as i64 {
            return Err(RuleError::NoMoney(CAPITAL_COST as i64));
        }
        self.countries[c as usize].slots[0] = SLOT_CAPITAL;
        self.players[p as usize].vault -= CAPITAL_COST as i64;
        Ok(())
    }

    pub fn buy_fort(&mut self, p: u8, c: u8, level: u8) -> Result<(), RuleError> {
        let cty = self.country(c);
        if cty.owner != p {
            return Err(RuleError::NotOwner);
        }
        if !cty.has_capital() {
            return Err(RuleError::NoCapital);
        }
        if cty.is_city() {
            return Err(RuleError::CityCannotFortify);
        }
        if level == 0 || level > 3 || level <= cty.fort {
            return Err(RuleError::MaxFort);
        }
        if cty.fort == 0 && cty.free_space() <= 0 {
            return Err(RuleError::NoSpace);
        }
        let cost = FORT_COST[level as usize] as i64;
        if self.vault(p) < cost {
            return Err(RuleError::NoMoney(cost));
        }
        let cty = self.country_mut(c);
        if cty.fort == 0 {
            cty.fill_build_area(SLOT_FORT);
        }
        cty.fort = level;
        self.players[p as usize].vault -= cost;
        Ok(())
    }

    pub fn buy_factory(&mut self, p: u8, c: u8, kind: usize) -> Result<(), RuleError> {
        let cty = self.country(c);
        if cty.owner != p {
            return Err(RuleError::NotOwner);
        }
        if !cty.has_capital() {
            return Err(RuleError::NoCapital);
        }
        if cty.free_space() <= 0 {
            return Err(RuleError::NoSpace);
        }
        let cost = FACTORY_COST[kind] as i64;
        if self.vault(p) < cost {
            return Err(RuleError::NoMoney(cost));
        }
        let cty = self.country_mut(c);
        cty.fill_build_area([3u8, 4, 6][kind]);
        cty.nbays += FACTORY_BAYS[kind] as i16;
        self.players[p as usize].vault -= cost;
        Ok(())
    }

    pub fn buy_enhancement(&mut self, p: u8, c: u8) -> Result<(), RuleError> {
        let cty = self.country(c);
        if cty.owner != p {
            return Err(RuleError::NotOwner);
        }
        if !cty.has_capital() {
            return Err(RuleError::NoCapital);
        }
        if cty.factory_count() == 0 {
            return Err(RuleError::NoFactory);
        }
        if cty.free_space() <= 0 {
            return Err(RuleError::NoSpace);
        }
        if cty.enhancements() >= 5 {
            return Err(RuleError::MaxEnhancements);
        }
        if self.vault(p) < ENHANCEMENT_COST as i64 {
            return Err(RuleError::NoMoney(ENHANCEMENT_COST as i64));
        }
        self.country_mut(c).fill_build_area(SLOT_ENHANCEMENT);
        self.players[p as usize].vault -= ENHANCEMENT_COST as i64;
        Ok(())
    }

    /// Cost and build time of cyborg `t` in country `c` (with its enhancements).
    pub fn cyborg_price(&self, c: u8, t: usize) -> (i64, i32) {
        let enh = self.country(c).enhancements();
        ((UNIT_COST[t] as f64 * (1.0 - enh as f64 / 10.0)) as i64, (BUILD_TIME[t] - enh).max(BUILD_TIME_MIN[t]))
    }

    pub fn buy_cyborg(&mut self, p: u8, c: u8, t: usize) -> Result<(), RuleError> {
        let cty = self.country(c);
        if cty.owner != p {
            return Err(RuleError::NotOwner);
        }
        if cty.factory_count() == 0 {
            return Err(RuleError::NoFactory);
        }
        if cty.free_bays() <= 0 {
            return Err(RuleError::NoBay);
        }
        let (cost, bt) = self.cyborg_price(c, t);
        if self.vault(p) < cost {
            return Err(RuleError::NoMoney(cost));
        }
        let cty = self.country_mut(c);
        let nb = cty.nbays.max(0) as usize;
        let b = cty.bays.iter().take(nb).position(|&b| b == EMPTY_BAY).ok_or(RuleError::NoBay)?;
        cty.bays[b] = (bt * 100 + t as i32) as i16;
        self.players[p as usize].vault -= cost;
        Ok(())
    }

    // ------------------------------------------------------------------ moving
    pub fn adjacent(&self, a: u8, b: u8) -> bool {
        self.country(a).adj().contains(&b)
    }

    /// Plot unit `i` of `from` to move to `to`.
    pub fn plot_move(&mut self, p: u8, from: u8, i: usize, to: u8) -> Result<(), RuleError> {
        if self.owner(from) != p {
            return Err(RuleError::NotOwner);
        }
        if !self.adjacent(from, to) {
            return Err(RuleError::NotAdjacent);
        }
        if self.owner(from) == self.owner(to) && self.country(to).count >= 10 {
            return Err(RuleError::Full);
        }
        let u = self.countries[from as usize].units[i];
        if !u.present() || u.dest != from {
            return Err(RuleError::NoBay);
        }
        self.countries[from as usize].units[i].dest = to;
        if self.owner(from) == self.owner(to) {
            self.countries[to as usize].count += 1;
        }
        self.countries[from as usize].count -= 1;
        Ok(())
    }

    /// Take back a plotted move of unit `i` of `from` (destination `to`).
    pub fn unplot_move(&mut self, from: u8, i: usize) {
        let u = self.countries[from as usize].units[i];
        if !u.present() || u.dest == from {
            return;
        }
        let to = u.dest;
        if self.owner(from) == self.owner(to) {
            self.countries[to as usize].count -= 1;
        }
        self.countries[from as usize].count += 1;
        self.countries[from as usize].units[i].dest = from;
    }

    // ------------------------------------------------------------------ repair
    pub fn repair_cost(&self, c: u8, i: usize) -> i64 {
        let u = self.country(c).units[i];
        let decile = (u.status.max(0) / 10).min(10) as usize;
        let f = if decile >= 10 { 0.0 } else { REPAIR_FACTOR[decile] };
        (f * UNIT_COST[u.kind as usize] as f32) as i64
    }

    pub fn repair_unit(&mut self, p: u8, c: u8, i: usize) -> Result<(), RuleError> {
        if self.owner(c) != p {
            return Err(RuleError::NotOwner);
        }
        let u = self.country(c).units[i];
        if !u.present() || u.status >= 100 || u.dest != c {
            return Err(RuleError::AlreadyBuilt);
        }
        let cost = self.repair_cost(c, i);
        if self.vault(p) <= cost {
            return Err(RuleError::NoMoney(cost));
        }
        self.players[p as usize].vault -= cost;
        self.countries[c as usize].units[i].status = 100;
        Ok(())
    }

    // ------------------------------------------------------------------ spying
    pub fn spying_allowed(&self) -> bool {
        !(self.year == 2200 && self.season == 1)
    }

    /// Resolve a spy operation on `c` with `money` credits. Returns the report.
    pub fn spy(&mut self, p: u8, c: u8, money: i64) -> SpyReport {
        self.players[p as usize].vault -= money;
        let cty = self.country(c).clone();
        let fort = cty.fort as i32;
        let exact_value = cty.value as i64 * 1000;
        if money == 0 {
            return SpyReport::Nothing;
        }
        let m = (money / 1000) as i32;
        let mut acc = m + (100 - 25 * (fort + 1));
        if acc > 100 {
            acc = 100;
        }
        let estimate = |g: &mut Game, roll: i32, base: i64| -> i64 {
            if g.rand(1, 2) == 1 {
                (base as f64 * (roll as f64 / 100.0)) as i64
            } else {
                (base as f64 * (2.0 - roll as f64 / 100.0)) as i64
            }
        };
        let vague = |g: &mut Game| -> SpyReport {
            let roll = g.rand(acc, 100);
            SpyReport::Vague { value: estimate(g, roll, exact_value / 1000) * 1000, space: cty.space }
        };
        if cty.owner == NEUTRAL {
            return vague(self);
        }
        let mut s = fort * 10 - m;
        if fort == 3 {
            s += 10;
        }
        if s < 0 {
            s = 0;
        }
        if self.rand(s, s + 9) >= 10 {
            return vague(self);
        }
        let half = (acc as f64 / 2.0) as i32;
        let roll = self.rand(half, 100);
        let tonnage = cty.tonnage() as i64;
        let est_tonnage = if acc > 60 { estimate(self, roll, tonnage) } else { 0 };
        let est_value = estimate(self, roll, exact_value / 1000) * 1000;
        SpyReport::Full {
            owner: cty.owner,
            government: cty.has_capital(),
            value: est_value,
            space: cty.space,
            tonnage: est_tonnage,
            fort: if acc > 60 { Some(cty.fort) } else { None },
            factories: if acc > 70 { Some((cty.factory_count(), cty.nbays as i32)) } else { None },
            upgrades: if acc > 80 { Some(cty.enhancements()) } else { None },
        }
    }
}

#[derive(Debug, Clone)]
pub enum SpyReport {
    Nothing,
    Vague { value: i64, space: u8 },
    Full { owner: u8, government: bool, value: i64, space: u8, tonnage: i64, fort: Option<u8>, factories: Option<(i32, i32)>, upgrades: Option<i32> },
}
