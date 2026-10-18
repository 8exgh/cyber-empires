//! Orchestrates a campaign battle: setup screens → real-time fight (or simulation) → result.

use super::prebattle::{prepare_defender_map, AttackSetup, DefendSetup};
use super::Sheets;
use crate::assets::Assets;
use crate::battle::result::apply_result;
use crate::battle::screen::BattleScreen;
use crate::battle::setup::{compose_battle, count_city_pieces, damage_city_pieces, DefenderPlacement, SetupParams, Structures};
use crate::gfx::{Palette, Surface};
use crate::input::Input;
use crate::rng::Rng;
use crate::strategy::resolve::{simulate_battle, Battle};
use crate::strategy::state::Game;

pub enum Stage {
    Attack(AttackSetup),
    Defend(DefendSetup),
    Fight { screen: Box<BattleScreen>, structures: Structures, cp1: i32, cp3: i32 },
    Done,
}

pub struct BattleFlow {
    pub battle: Battle,
    pub stage: Stage,
    pub adir: usize,
    pub att_goal: usize,
    pub att_agg: usize,
    pub att_control: usize,
    pub def_control: usize,
    pub att_sim: bool,
    pub def_sim: bool,
    pub human: [bool; 2],
    pub placement: DefenderPlacement,
    pub result: Option<Battle>,
}

impl BattleFlow {
    pub fn new(g: &Game, b: Battle, rng: &mut Rng) -> BattleFlow {
        let ao = b.att_owner;
        let d_o = b.def_owner;
        let human = [g.is_human(ao), g.is_human(d_o)];
        let adir = rng.range(0, 3) as usize;
        let mut flow = BattleFlow {
            battle: b,
            stage: Stage::Done,
            adir,
            att_goal: 0,
            att_agg: 2,
            att_control: 0,
            def_control: 0,
            att_sim: false,
            def_sim: false,
            human,
            placement: DefenderPlacement::Ai,
            result: None,
        };
        if human[0] {
            flow.stage = Stage::Attack(AttackSetup::new(&flow.battle));
        } else {
            // DECIDE_ON_GOAL
            let heavy = (0..10).filter(|&i| matches!(flow.battle.types[i], 0 | 1 | 4)).count();
            flow.att_goal = if heavy > 2 { 2 } else { 0 };
            flow.att_control = (0..10).rev().find(|&i| flow.battle.types[i] != crate::data::NO_CYBORG).unwrap_or(0);
            flow.stage = Stage::Done; // advanced by `advance`
        }
        flow
    }

    fn simulate_now(&self) -> bool {
        if self.human[0] && self.human[1] {
            self.att_sim && self.def_sim
        } else {
            self.att_sim || self.def_sim
        }
    }

    /// Move to the next stage once the current one is finished. Returns true when the whole flow is complete.
    pub fn advance(&mut self, g: &mut Game, assets: &mut Assets, rng: &mut Rng) -> bool {
        loop {
            match &mut self.stage {
                Stage::Attack(a) => {
                    if !a.done {
                        return false;
                    }
                    self.att_goal = a.mission;
                    self.att_agg = a.agg;
                    self.att_control = a.control_slot();
                    self.att_sim = a.simulate;
                    self.stage = Stage::Done;
                }
                Stage::Defend(d) => {
                    if !d.done {
                        return false;
                    }
                    self.def_sim = d.simulate;
                    self.def_control = d.control_slot();
                    self.placement = if d.auto { DefenderPlacement::Auto } else { DefenderPlacement::Manual(d.positions.clone()) };
                    self.stage = Stage::Done;
                    return self.start_fight(g, assets, rng);
                }
                Stage::Fight { screen, structures, cp1, cp3 } => {
                    if !screen.finished() {
                        return false;
                    }
                    let mut b = self.battle.clone();
                    apply_result(g, &mut b, &screen.sim, structures, *cp1, *cp3);
                    self.result = Some(b);
                    self.stage = Stage::Done;
                    return true;
                }
                Stage::Done => {
                    if self.result.is_some() {
                        return true;
                    }
                    // after the attacker stage: defender stage or fight
                    if self.human[1] && !matches!(self.placement, DefenderPlacement::Manual(_) | DefenderPlacement::Auto) && !self.def_sim {
                        let ttype = (g.country(self.battle.defend_cty).terrain() as usize).saturating_sub(1).min(4);
                        if let Some((map, st)) = prepare_defender_map(assets, g, &self.battle, self.adir) {
                            let show_sim = self.att_sim || !self.human[0];
                            self.stage = Stage::Defend(DefendSetup::new(map, st, self.adir, &self.battle, ttype, show_sim));
                            self.human[1] = true;
                            // mark defender stage as consumed via placement after it finishes
                            return false;
                        }
                    }
                    return self.start_fight(g, assets, rng);
                }
            }
        }
    }

    fn start_fight(&mut self, g: &mut Game, assets: &mut Assets, rng: &mut Rng) -> bool {
        if self.simulate_now() {
            let b = simulate_battle(g, self.battle.clone());
            self.result = Some(b);
            self.stage = Stage::Done;
            return true;
        }
        let params = SetupParams {
            adir: self.adir,
            att_goal: self.att_goal,
            att_agg: self.att_agg,
            att_control: self.att_control,
            def_control: self.def_control,
            human: self.human,
            placement: self.placement.clone(),
        };
        // structures / city counts for the result handling
        let def = g.country(self.battle.defend_cty).clone();
        let mut cp1 = 0;
        let mut cp3 = 0;
        let structures = match prepare_defender_map(assets, g, &self.battle, self.adir) {
            Some((mut map, st)) => {
                if def.is_city() {
                    cp1 = count_city_pieces(&map);
                    damage_city_pieces(&mut map, rng, def.damage as i32);
                    cp3 = count_city_pieces(&map);
                }
                st
            }
            None => Structures::default(),
        };
        match compose_battle(assets, g, &self.battle, rng, &params) {
            Some(init) => match BattleScreen::new(assets, &init) {
                Some(screen) => {
                    self.stage = Stage::Fight { screen: Box::new(screen), structures, cp1, cp3 };
                    false
                }
                None => {
                    let b = simulate_battle(g, self.battle.clone());
                    self.result = Some(b);
                    true
                }
            },
            None => {
                let b = simulate_battle(g, self.battle.clone());
                self.result = Some(b);
                true
            }
        }
    }

    pub fn update(&mut self, input: &Input, dt: f32) {
        match &mut self.stage {
            Stage::Attack(a) => a.update(input),
            Stage::Defend(d) => d.update(input),
            Stage::Fight { screen, .. } => screen.update(input, dt),
            Stage::Done => {}
        }
    }

    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets, g: &Game, strategy_pal: &Palette) -> Palette {
        match &self.stage {
            Stage::Attack(a) => {
                a.draw(fb, sheets, g, &self.battle);
                *strategy_pal
            }
            Stage::Defend(d) => {
                let d_o = self.battle.def_owner;
                d.draw(fb, sheets, g, &self.battle, [95u8, 2, 6, 5, 45][d_o.min(4) as usize]);
                *strategy_pal
            }
            Stage::Fight { screen, .. } => {
                screen.draw(fb);
                screen.palette()
            }
            Stage::Done => {
                fb.clear(0);
                *strategy_pal
            }
        }
    }
}
