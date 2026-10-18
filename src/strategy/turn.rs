//! Turn flow: player advancement, end-of-round processing, eliminations.

use super::resolve::{check_players, ActionPhase};
use super::state::*;
use crate::world_data::NUM_COUNTRIES;

impl Game {
    /// `_INC_DATE` part 1: advance the calendar (before the battle phase).
    pub fn advance_date(&mut self) {
        self.season += 1;
        if self.season > 7 {
            self.season = 1;
            self.year += 1;
        }
    }

    /// HAL stage counter update done at the end of the action phase.
    pub fn advance_stage(&mut self) {
        if self.stage < 10 && (self.season == 1 || self.season == 3 || self.season == 5) {
            self.stage += 1;
        }
        if self.stage > 8 {
            self.stage = if self.stage == 9 { 10 } else { 9 };
        }
    }

    /// `_INC_DATE` part 2: city repair, production countdown, income and deployment.
    pub fn end_of_round_economy(&mut self) {
        for c in 1..=NUM_COUNTRIES {
            let cty = &mut self.countries[c];
            if cty.damage > 0 {
                cty.damage -= 2;
                if cty.damage < 0 {
                    cty.damage = 0;
                }
            }
            if cty.owner == NEUTRAL {
                continue;
            }
            for b in cty.bays.iter_mut() {
                if *b >= 100 {
                    *b -= 100;
                }
            }
            if cty.has_capital() && cty.damage != 100 {
                let income = (cty.value as f64 * ((100.0 - cty.damage as f64) / 100.0) * 1000.0) as i64;
                self.players[cty.owner as usize].vault += income;
            }
            let mut count = cty.num_units() as i16;
            cty.count = count;
            for b in 0..24 {
                if count >= 10 {
                    break;
                }
                let v = cty.bays[b];
                if v < 100 && v != EMPTY_BAY && (0..9).contains(&v) {
                    if let Some(s) = cty.free_unit_slot() {
                        cty.units[s] = Unit { kind: v as u8, status: 100, dest: c as u8 };
                        cty.bays[b] = EMPTY_BAY;
                        count += 1;
                        cty.count = count;
                    }
                }
            }
        }
    }

    /// Income a player would receive next round (for the statistics screen).
    pub fn income(&self, p: u8) -> i64 {
        let mut s = 0i64;
        for c in 1..=NUM_COUNTRIES {
            let cty = &self.countries[c];
            if cty.owner == p && cty.has_capital() && cty.damage != 100 {
                s += (cty.value as f64 * ((100.0 - cty.damage as f64) / 100.0) * 1000.0) as i64;
            }
        }
        s
    }

    pub fn owns_any(&self, p: u8) -> bool {
        (1..=NUM_COUNTRIES).any(|c| self.countries[c].owner == p)
    }
    pub fn countries_owned(&self, p: u8) -> usize {
        (1..=NUM_COUNTRIES).filter(|&c| self.countries[c].owner == p).count()
    }
    pub fn players_alive(&self) -> usize {
        self.players.iter().filter(|p| p.ptype != PlayerType::None).count()
    }
    pub fn humans_left(&self) -> bool {
        self.players.iter().any(|p| p.ptype == PlayerType::Human)
    }

    /// `_CHECK_STATUS`: eliminate the current player if he owns nothing. Returns whether alive.
    pub fn check_status(&mut self) -> bool {
        let p = self.cur_player.max(0) as u8;
        let alive = self.owns_any(p);
        if !alive {
            self.players[p as usize].ptype = PlayerType::None;
        }
        alive
    }

    /// `_CHECK_ELIMINATION`: players whose status changed since the last check (annihilated).
    pub fn check_elimination(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..5 {
            if i as i16 != self.cur_player && self.testrowy[i] != self.lastrowy[i] {
                out.push(i as u8);
            }
        }
        out
    }

    /// Next player index after `from` (wrapping not allowed): None when the round is over.
    pub fn next_player_index(&self, from: i16) -> Option<i16> {
        let mut p = from + 1;
        while p < 5 {
            if self.players[p as usize].ptype != PlayerType::None {
                return Some(p);
            }
            p += 1;
        }
        None
    }

    pub fn first_player_index(&self) -> i16 {
        self.next_player_index(-1).unwrap_or(0)
    }
}

/// Round state machine used by the UI: between turns the game may need to run the battle phase.
#[derive(Debug)]
pub enum TurnStep {
    /// A new player's turn begins (player index).
    Turn(u8),
    /// The round ended: the action phase must be run (via `ActionPhase`), then call `after_action_phase`.
    RoundEnd,
    /// Only one player remains / no humans left.
    GameOver,
}

impl Game {
    /// `_GET_PLAYER`: advance to the next player or signal the end of the round.
    pub fn get_player(&mut self) -> TurnStep {
        self.prev_player = self.cur_player;
        match self.next_player_index(self.cur_player) {
            Some(p) => {
                self.cur_player = p;
                if self.prev_player == self.cur_player {
                    TurnStep::GameOver
                } else {
                    TurnStep::Turn(p as u8)
                }
            }
            None => TurnStep::RoundEnd,
        }
    }

    /// Call after the action phase finished: economy, then the first player of the new round.
    pub fn after_action_phase(&mut self) -> TurnStep {
        self.advance_stage();
        check_players(self);
        self.end_of_round_economy();
        self.cur_player = self.first_player_index();
        if self.prev_player == self.cur_player && self.players_alive() <= 1 {
            return TurnStep::GameOver;
        }
        TurnStep::Turn(self.cur_player as u8)
    }

    /// Convenience for headless simulation: run a complete round with all battles simulated.
    pub fn run_round_simulated(&mut self) {
        self.advance_date();
        let mut phase = ActionPhase::new();
        loop {
            match phase.step(self) {
                super::resolve::PhaseEvent::Battle(b) => {
                    let b = super::resolve::simulate_battle(self, b);
                    phase.finish_battle(self, b);
                }
                super::resolve::PhaseEvent::Simulated(_) => {}
                super::resolve::PhaseEvent::Done => break,
            }
        }
        self.advance_stage();
        self.end_of_round_economy();
    }
}
