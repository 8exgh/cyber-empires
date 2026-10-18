//! The strategy control screen (CYBOR.PRG main loop): map, button bar, message box and all dialogs.

use super::art;
use super::{fmt_money, wrap, Sheets};
use crate::data::*;
use crate::gfx::{Rect, Surface};
use crate::input::{key, Input};
use crate::strategy::hal::hals_turn;
use crate::strategy::resolve::{simulate_battle, ActionPhase, Battle, PhaseEvent};
use crate::strategy::rules::{RuleError, SpyReport};
use crate::strategy::state::*;
use crate::strategy::turn::TurnStep;
use crate::text::Font;
use crate::world_data::NUM_COUNTRIES;

const MAP_X_MAX: i32 = 0x15E;
const MAP_Y_MAX: i32 = 0x10E;
const SCROLL_STEP: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Move,
    Repair,
    Spy,
    Cyborgs,
    Capital,
    Factory,
    Enhance,
    Fort,
}

impl Op {
    fn label(self) -> &'static str {
        match self {
            Op::Move => "MOVE OPTION:",
            Op::Repair => "REPAIR OPTION",
            Op::Spy => "SPY OPTION",
            Op::Cyborgs => "BUILD CYBORGS",
            Op::Capital => "BUILD CAPITAL",
            Op::Factory => "BUILD FACTORY",
            Op::Enhance => "BUILD ENHANCEMENT",
            Op::Fort => "BUILD FORTIFICATION",
        }
    }
    fn prompt(self) -> [&'static str; 2] {
        match self {
            Op::Move => ["SELECT A COUNTRY", "TO MOVE FROM."],
            Op::Repair => ["SELECT THE COUNTRY", "YOU WISH TO REPAIR"],
            Op::Spy => ["SELECT THE COUNTRY", "YOU WOULD LIKE TO INVESTIGATE"],
            _ => ["SELECT THE COUNTRY", "YOU WANT TO BUILD ON"],
        }
    }
    fn button(self) -> (Rect, Rect) {
        match self {
            Op::Move => art::BTN_MOVE,
            Op::Repair => art::BTN_REPAIR,
            Op::Spy => art::BTN_SPY,
            Op::Cyborgs => art::BTN_CYBORGS,
            Op::Capital => art::BTN_CAPITAL,
            Op::Factory => art::BTN_FACTORY,
            Op::Enhance => art::BTN_ENHANCE,
            Op::Fort => art::BTN_FORT,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    /// "END OF TURN. RE-CLICK BUTTON TO CONFIRM."
    ConfirmEndTurn,
    /// Waiting for a country click for an operation.
    Op(Op),
    MoveDest { src: u8 },
    MoveBox { src: u8, dst: u8 },
    RepairBox { cty: u8 },
    FortMenu { cty: u8, chosen: i32 },
    FactMenu { cty: u8, counts: [i32; 3] },
    CybMenu { cty: u8, bought: Vec<usize>, total_cost: i64 },
    CybInfo { cty: u8, kind: usize, weapon: usize, bought: Vec<usize>, total_cost: i64 },
    SpyMoney { cty: u8, amount: i64 },
    SpyReport { report: SpyReport },
    CountryStatus { cty: u8, sub: u8 },
    Options { confirm_quit: bool },
    SaveBox,
    LoadBox,
    Annihilated { list: Vec<u8> },
    HalTurn { timer: f32 },
    /// End of round: the battle phase is running.
    Resolving,
    Summary { battle: Battle },
    Victory { timer: f32 },
    Crushed { timer: f32 },
    /// A battle must be fought interactively (complete campaign): handed to the App.
    PendingBattle { battle: Battle },
}

pub enum Request {
    None,
    /// Run the pre-battle / real-time battle for this battle; call `finish_battle` with the result.
    Battle(Battle),
    /// Store/load a save slot (1..=5) in the host.
    Save(usize),
    Load(usize),
    /// Return to the main menu.
    Exit,
}

pub struct StrategyScreen {
    pub game: Game,
    pub mode: Mode,
    pub map_x: i32,
    pub map_y: i32,
    pub doors_open: bool,
    pub delay_time: i32,
    msg: Vec<String>,
    msg_timer: f32,
    hover: u8,
    phase: Option<ActionPhase>,
    pub request: Request,
    pub complete: bool,
    crushed_shown: bool,
    congrats_shown: bool,
    game_over: bool,
}

fn err_lines(e: &RuleError, cost: i64, vault: i64) -> Vec<String> {
    let _ = cost;
    match e {
        RuleError::NotOwner => vec!["YOU DO NOT OCCUPY".into(), "THIS COUNTRY.".into(), "SELECTION CANCELLED".into(), "SELECT NEW OPERATION".into()],
        RuleError::NoCapital => vec!["YOU MUST ESTABLISH".into(), "A GOVERNMENT IN".into(), "THIS COUNTRY BEFORE".into(), "YOU CAN BUILD.".into()],
        RuleError::NoSpace => vec!["THERE IS NO ROOM TO".into(), "BUILD ANYTHING IN".into(), "THIS COUNTRY.".into()],
        RuleError::NoMoney(c) => vec!["IT WILL COST".into(), fmt_money(*c), "YOU HAVE:".into(), fmt_money(vault)],
        RuleError::NotAdjacent => vec!["YOU CAN ONLY MOVE TO".into(), "ADJACENT COUNTRIES.".into(), "SELECTION CANCELLED.".into(), "SELECT NEW OPERATION".into()],
        RuleError::Full => vec!["MAX".into()],
        RuleError::NoFactory => vec!["YOU CAN ONLY".into(), "BUILD CYBORGS".into(), "IN A COUNTRY WITH".into(), "A FACTORY.".into()],
        RuleError::AlreadyBuilt => vec!["YOU ALREADY".into(), "ESTABLISHED".into(), "GOVERNMENT CONTROL".into(), "SELECTION CANCELLED.".into()],
        RuleError::CityCannotFortify => vec!["THERE IS NO ROOM TO".into(), "BUILD ANYTHING IN".into(), "THIS COUNTRY.".into()],
        RuleError::MaxFort => vec!["THIS COUNTRY IS".into(), "HEAVILY FORTIFIED.".into(), "MORE FORTIFICATION".into(), "IS NOT POSSIBLE.".into()],
        RuleError::MaxEnhancements => vec!["THERE IS NO ROOM TO".into(), "BUILD ANYTHING IN".into(), "THIS COUNTRY.".into()],
        RuleError::NoBay => vec!["ALL FACTORY BAYS".into(), "UTILIZED.".into()],
    }
}

impl StrategyScreen {
    pub fn new(game: Game, complete: bool) -> StrategyScreen {
        let mut s = StrategyScreen {
            game,
            mode: Mode::Normal,
            map_x: 0,
            map_y: 0,
            doors_open: false,
            delay_time: 2,
            msg: Vec::new(),
            msg_timer: 0.0,
            hover: 0,
            phase: None,
            request: Request::None,
            complete,
            crushed_shown: false,
            congrats_shown: false,
            game_over: false,
        };
        // first player
        if s.game.cur_player < 0 {
            s.game.cur_player = s.game.first_player_index();
        }
        s.load_player_view();
        s.begin_player_turn_sequence();
        s
    }

    fn load_player_view(&mut self) {
        let p = self.game.cur().clone();
        self.map_x = (p.map_x as i32).clamp(0, MAP_X_MAX);
        self.map_y = (p.map_y as i32).clamp(0, MAP_Y_MAX);
    }
    fn save_player_view(&mut self) {
        let cp = self.game.cur_player.max(0) as usize;
        self.game.players[cp].map_x = self.map_x as i16;
        self.game.players[cp].map_y = self.map_y as i16;
    }

    /// Entering a player's turn: if computer, run HAL; otherwise show the prompt.
    fn begin_player_turn_sequence(&mut self) {
        let p = self.game.cur_player.max(0) as u8;
        if self.game.is_computer(p) {
            self.mode = Mode::HalTurn { timer: 0.0 };
        } else {
            self.mode = Mode::Normal;
            self.doors_open = false;
            self.load_player_view();
        }
    }

    fn set_msg(&mut self, lines: Vec<String>) {
        self.msg = lines;
        self.msg_timer = self.delay_time as f32;
    }

    fn cur(&self) -> u8 {
        self.game.cur_player.max(0) as u8
    }

    fn country_at(&self, mx: i32, my: i32) -> u8 {
        if !art::MAP_WIN.contains(mx, my) {
            return 0;
        }
        let wx = mx + self.map_x - 16;
        let wy = my + self.map_y - 16;
        let mut found = 0;
        for c in 1..=NUM_COUNTRIES as u8 {
            let cty = self.game.country(c);
            let (x, y) = (cty.x() as i32, cty.y() as i32);
            if wx >= x - 8 && wx <= x + 25 && wy >= y - 8 && wy <= y + 25 {
                found = c;
            }
        }
        found
    }

    fn scroll(&mut self, input: &Input) {
        if !input.mouse_down || !self.doors_open {
            return;
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if art::ARROW_LEFT.contains(mx, my) {
            self.map_x = (self.map_x - SCROLL_STEP).max(0);
        }
        if art::ARROW_RIGHT.contains(mx, my) {
            self.map_x = (self.map_x + SCROLL_STEP).min(MAP_X_MAX);
        }
        if art::ARROW_UP.contains(mx, my) {
            self.map_y = (self.map_y - SCROLL_STEP).max(0);
        }
        if art::ARROW_DOWN.contains(mx, my) {
            self.map_y = (self.map_y + SCROLL_STEP).min(MAP_Y_MAX);
        }
        // keyboard scrolling as a convenience
        if input.key(key::LEFT) {
            self.map_x = (self.map_x - SCROLL_STEP).max(0);
        }
    }

    // ------------------------------------------------------------------ update

    pub fn update(&mut self, input: &Input, dt: f32) {
        if self.msg_timer > 0.0 {
            self.msg_timer -= dt;
            if self.msg_timer <= 0.0 {
                self.msg.clear();
            }
        }
        if input.key(key::LEFT) {
            self.map_x = (self.map_x - SCROLL_STEP).max(0);
        }
        if input.key(key::RIGHT) {
            self.map_x = (self.map_x + SCROLL_STEP).min(MAP_X_MAX);
        }
        if input.key(key::UP) {
            self.map_y = (self.map_y - SCROLL_STEP).max(0);
        }
        if input.key(key::DOWN) {
            self.map_y = (self.map_y + SCROLL_STEP).min(MAP_Y_MAX);
        }
        self.hover = self.country_at(input.mouse_x, input.mouse_y);
        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        self.mode = match mode {
            Mode::Normal => self.update_normal(input),
            Mode::ConfirmEndTurn => self.update_confirm_end(input),
            Mode::Op(op) => self.update_op(op, input),
            Mode::MoveDest { src } => self.update_move_dest(src, input),
            Mode::MoveBox { src, dst } => self.update_move_box(src, dst, input),
            Mode::RepairBox { cty } => self.update_repair_box(cty, input),
            Mode::FortMenu { cty, chosen } => self.update_fort_menu(cty, chosen, input),
            Mode::FactMenu { cty, counts } => self.update_fact_menu(cty, counts, input),
            Mode::CybMenu { cty, bought, total_cost } => self.update_cyb_menu(cty, bought, total_cost, input),
            Mode::CybInfo { cty, kind, weapon, bought, total_cost } => self.update_cyb_info(cty, kind, weapon, bought, total_cost, input),
            Mode::SpyMoney { cty, amount } => self.update_spy_money(cty, amount, input),
            Mode::SpyReport { report } => {
                if input.clicked() || input.any_pressed() {
                    Mode::Normal
                } else {
                    Mode::SpyReport { report }
                }
            }
            Mode::CountryStatus { cty, sub } => self.update_country_status(cty, sub, input),
            Mode::Options { confirm_quit } => self.update_options(confirm_quit, input),
            Mode::SaveBox => self.update_slot_box(input, true),
            Mode::LoadBox => self.update_slot_box(input, false),
            Mode::Annihilated { mut list } => {
                if input.clicked() || input.pressed(key::ENTER) {
                    list.remove(0);
                }
                if list.is_empty() {
                    self.after_elimination_reports()
                } else {
                    Mode::Annihilated { list }
                }
            }
            Mode::HalTurn { timer } => self.update_hal(timer, dt, input),
            Mode::Resolving => self.update_resolving(),
            Mode::Summary { battle } => {
                if input.clicked() || input.pressed(key::ENTER) {
                    Mode::Resolving
                } else {
                    Mode::Summary { battle }
                }
            }
            Mode::Victory { timer } => {
                if timer > 8.0 || (timer > 1.5 && input.clicked()) {
                    Mode::Normal
                } else {
                    Mode::Victory { timer: timer + dt }
                }
            }
            Mode::Crushed { timer } => {
                if timer > 3.0 || input.clicked() {
                    Mode::Normal
                } else {
                    Mode::Crushed { timer: timer + dt }
                }
            }
            Mode::PendingBattle { battle } => Mode::PendingBattle { battle },
        };
    }

    fn update_normal(&mut self, input: &Input) -> Mode {
        self.scroll(input);
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if !input.clicked() {
            return Mode::Normal;
        }
        if art::BTN_DOOR.contains(mx, my) {
            return self.door_action();
        }
        if !self.doors_open {
            return Mode::Normal;
        }
        if art::BTN_OPTIONS.contains(mx, my) {
            return Mode::Options { confirm_quit: false };
        }
        for op in [Op::Move, Op::Repair, Op::Spy, Op::Cyborgs, Op::Capital, Op::Factory, Op::Enhance, Op::Fort] {
            if op.button().0.contains(mx, my) {
                let p = op.prompt();
                self.msg = vec![op.label().to_string(), p[0].to_string(), p[1].to_string()];
                self.msg_timer = 1e9;
                return Mode::Op(op);
            }
        }
        if art::MAP_WIN.contains(mx, my) {
            let c = self.country_at(mx, my);
            if c != 0 && self.game.owner(c) == self.cur() {
                return Mode::CountryStatus { cty: c, sub: 0 };
            }
        }
        Mode::Normal
    }

    /// The door button: start of turn (open) or end of turn (close, with confirmation).
    fn door_action(&mut self) -> Mode {
        if self.game_over {
            self.request = Request::Exit;
            return Mode::Normal;
        }
        if !self.doors_open {
            self.doors_open = true;
            self.msg.clear();
            // CHECK_ELIMINATION
            let elims = self.game.check_elimination();
            self.game.lastrowy = self.game.testrowy;
            let alive = self.game.check_status();
            if !alive && !self.crushed_shown {
                self.crushed_shown = true;
                return Mode::Crushed { timer: 0.0 };
            }
            if self.game.players_alive() == 1 && alive && !self.congrats_shown {
                self.congrats_shown = true;
                self.game_over = true;
                return Mode::Victory { timer: 0.0 };
            }
            if !elims.is_empty() {
                return Mode::Annihilated { list: elims };
            }
            Mode::Normal
        } else {
            self.msg = vec!["END OF TURN.".into(), "RE-CLICK BUTTON".into(), "TO CONFIRM.".into()];
            self.msg_timer = 1e9;
            Mode::ConfirmEndTurn
        }
    }

    fn after_elimination_reports(&mut self) -> Mode {
        if !self.game.check_status() {
            self.crushed_shown = true;
            return Mode::Crushed { timer: 0.0 };
        }
        Mode::Normal
    }

    fn update_confirm_end(&mut self, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::ConfirmEndTurn;
        }
        self.msg.clear();
        if art::BTN_DOOR.contains(input.mouse_x, input.mouse_y) {
            self.end_turn()
        } else {
            Mode::Normal
        }
    }

    /// Close the doors and advance to the next player / round.
    fn end_turn(&mut self) -> Mode {
        self.doors_open = false;
        self.save_player_view();
        self.next_player()
    }

    fn next_player(&mut self) -> Mode {
        if !self.game.humans_left() {
            self.game_over = true;
            self.request = Request::Exit;
            return Mode::Normal;
        }
        match self.game.get_player() {
            TurnStep::Turn(_) => {
                self.begin_player_turn_sequence();
                self.mode_after_begin()
            }
            TurnStep::RoundEnd => {
                self.game.advance_date();
                self.phase = Some(ActionPhase::new());
                Mode::Resolving
            }
            TurnStep::GameOver => {
                self.game_over = true;
                Mode::Normal
            }
        }
    }

    fn mode_after_begin(&self) -> Mode {
        self.mode.clone()
    }

    fn update_hal(&mut self, timer: f32, dt: f32, input: &Input) -> Mode {
        let t = timer + dt;
        let p = self.cur();
        if t < 0.6 && !input.clicked() {
            return Mode::HalTurn { timer: t };
        }
        // run the AI turn (if still alive)
        if self.game.check_status() {
            self.game.lastrowy = self.game.testrowy;
            hals_turn(&mut self.game);
        }
        let _ = p;
        self.next_player()
    }

    fn update_resolving(&mut self) -> Mode {
        let mut phase = match self.phase.take() {
            Some(p) => p,
            None => return Mode::Normal,
        };
        match phase.step(&mut self.game) {
            PhaseEvent::Battle(b) => {
                self.phase = Some(phase);
                if self.complete {
                    self.request = Request::Battle(b.clone());
                    Mode::PendingBattle { battle: b }
                } else {
                    let b = simulate_battle(&mut self.game, b);
                    let b = self.phase.as_mut().unwrap().finish_battle(&mut self.game, b);
                    Mode::Summary { battle: b }
                }
            }
            PhaseEvent::Simulated(b) => {
                self.phase = Some(phase);
                let human_involved = self.game.is_human(b.attacker_owner(&self.game)) || self.game.is_human(b.defender_owner(&self.game));
                if human_involved {
                    Mode::Summary { battle: b }
                } else {
                    Mode::Resolving
                }
            }
            PhaseEvent::Done => {
                self.phase = None;
                match self.game.after_action_phase() {
                    TurnStep::GameOver => {
                        self.game_over = true;
                        Mode::Normal
                    }
                    _ => {
                        self.begin_player_turn_sequence();
                        self.mode.clone()
                    }
                }
            }
        }
    }

    /// Called by the App when an interactive battle has been resolved.
    pub fn finish_battle(&mut self, b: Battle) {
        if let Some(phase) = self.phase.as_mut() {
            let b = phase.finish_battle(&mut self.game, b);
            self.mode = Mode::Summary { battle: b };
        } else {
            self.mode = Mode::Resolving;
        }
    }

    // ------------------------------------------------------------------ operations

    fn update_op(&mut self, op: Op, input: &Input) -> Mode {
        self.scroll(input);
        if !input.clicked() {
            return Mode::Op(op);
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if my >= 142 {
            // button bar: cancel the op (or switch)
            self.msg.clear();
            self.msg_timer = 0.0;
            return self.update_normal(input);
        }
        let c = self.country_at(mx, my);
        if c == 0 {
            self.set_msg(vec!["CLICK IN THE CENTER".into(), "OF THE COUNTRY.".into(), "SELECTION CANCELLED.".into(), "SELECT NEW OPERATION".into()]);
            return Mode::Normal;
        }
        let p = self.cur();
        let owner = self.game.owner(c) == p;
        let vault = self.game.vault(p);
        match op {
            Op::Spy => {
                if owner {
                    return Mode::CountryStatus { cty: c, sub: 0 };
                }
                if !self.game.spying_allowed() {
                    self.set_msg(vec!["SPYING ON THE".into(), "INITIAL TURN".into(), "IS PROHIBITED.".into(), "SELECTION CANCELLED.".into()]);
                    return Mode::Normal;
                }
                return Mode::SpyMoney { cty: c, amount: 0 };
            }
            _ => {}
        }
        if !owner {
            self.set_msg(err_lines(&RuleError::NotOwner, 0, vault));
            return Mode::Normal;
        }
        match op {
            Op::Move => {
                self.msg = vec!["SELECT A DESTINATION".into(), "COUNTRY.".into()];
                self.msg_timer = 1e9;
                Mode::MoveDest { src: c }
            }
            Op::Repair => Mode::RepairBox { cty: c },
            Op::Capital => {
                match self.game.buy_capital(p, c) {
                    Ok(()) => {
                        let name = self.game.country(c).name().to_string();
                        self.set_msg(vec!["YOU HAVE ESTABLISHED".into(), "GOVERNMENT CONTROL".into(), format!("IN {}", name)]);
                    }
                    Err(RuleError::NoMoney(_)) => {
                        self.set_msg(vec!["IT WILL COST".into(), "30000".into(), "TO ESTABLISH A".into(), format!("GOVERNMENT. YOU HAVE: {}", vault)]);
                    }
                    Err(e) => self.set_msg(err_lines(&e, 0, vault)),
                }
                Mode::Op(op)
            }
            Op::Enhance => {
                match self.game.buy_enhancement(p, c) {
                    Ok(()) => self.set_msg(vec!["FACTORY UPGRADE".into(), "INSTALLED.".into()]),
                    Err(RuleError::NoFactory) => self.set_msg(vec!["A FACTORY UPGRADE".into(), "CANNOT BE BUILT".into(), "WITHOUT A FACTORY".into(), "ESTABLISHED.".into()]),
                    Err(RuleError::NoMoney(_)) => self.set_msg(vec!["A FACTORY UPGRADE".into(), "COSTS: 400000".into(), "YOU HAVE:".into(), fmt_money(vault)]),
                    Err(e) => self.set_msg(err_lines(&e, 0, vault)),
                }
                Mode::Op(op)
            }
            Op::Fort => {
                let cty = self.game.country(c);
                if !cty.has_capital() {
                    self.set_msg(err_lines(&RuleError::NoCapital, 0, vault));
                    return Mode::Normal;
                }
                if cty.fort == 0 && cty.free_space() <= 0 || cty.is_city() {
                    self.set_msg(err_lines(&RuleError::NoSpace, 0, vault));
                    return Mode::Normal;
                }
                if cty.fort == 3 {
                    self.set_msg(err_lines(&RuleError::MaxFort, 0, vault));
                    return Mode::Normal;
                }
                let next_cost = FORT_COST[cty.fort as usize + 1] as i64;
                if vault < next_cost {
                    self.set_msg(vec!["IT WILL COST AT LEAST".into(), fmt_money(next_cost), "TO FORTIFY".into(), format!("THIS COUNTRY. YOU HAVE: {}", vault)]);
                    return Mode::Normal;
                }
                Mode::FortMenu { cty: c, chosen: -1 }
            }
            Op::Factory => {
                let cty = self.game.country(c);
                if !cty.has_capital() {
                    self.set_msg(err_lines(&RuleError::NoCapital, 0, vault));
                    return Mode::Normal;
                }
                if cty.free_space() <= 0 {
                    self.set_msg(err_lines(&RuleError::NoSpace, 0, vault));
                    return Mode::Normal;
                }
                if vault < 500_000 {
                    self.set_msg(vec!["IT WILL COST AT LEAST".into(), "500000 TO BUILD".into(), "ANY TYPE OF FACTORY.".into(), format!("YOU HAVE: {}", vault)]);
                    return Mode::Normal;
                }
                Mode::FactMenu { cty: c, counts: [0; 3] }
            }
            Op::Cyborgs => {
                let cty = self.game.country(c);
                if !cty.has_capital() {
                    self.set_msg(err_lines(&RuleError::NoCapital, 0, vault));
                    return Mode::Normal;
                }
                if cty.factory_count() == 0 {
                    self.set_msg(err_lines(&RuleError::NoFactory, 0, vault));
                    return Mode::Normal;
                }
                Mode::CybMenu { cty: c, bought: Vec::new(), total_cost: 0 }
            }
            Op::Spy => Mode::Normal,
        }
    }

    fn update_move_dest(&mut self, src: u8, input: &Input) -> Mode {
        self.scroll(input);
        if !input.clicked() {
            return Mode::MoveDest { src };
        }
        if input.mouse_y >= 142 {
            self.msg.clear();
            self.msg_timer = 0.0;
            return self.update_normal(input);
        }
        let c = self.country_at(input.mouse_x, input.mouse_y);
        if c == 0 {
            self.set_msg(vec!["CLICK IN THE CENTER".into(), "OF THE COUNTRY.".into(), "SELECTION CANCELLED.".into(), "SELECT NEW OPERATION".into()]);
            return Mode::Normal;
        }
        if !self.game.adjacent(src, c) {
            self.set_msg(err_lines(&RuleError::NotAdjacent, 0, 0));
            return Mode::Normal;
        }
        self.msg.clear();
        self.msg_timer = 0.0;
        Mode::MoveBox { src, dst: c }
    }

    /// Rows of the move/repair box: (unit slot index, y) for resident units.
    fn unit_rows(&self, c: u8, repair: bool) -> Vec<(usize, i32)> {
        let cty = self.game.country(c);
        let mut rows = Vec::new();
        let mut y = 0x46;
        for (i, u) in cty.units.iter().enumerate() {
            if u.present() && (u.dest == c || (!repair && u.dest != c)) {
                if repair && u.status >= 100 {
                    y += 6;
                    continue;
                }
                rows.push((i, y));
                y += 6;
            }
        }
        rows
    }

    fn update_move_box(&mut self, src: u8, dst: u8, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::MoveBox { src, dst };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if art::OK_HIT.contains(mx, my) || input.pressed(key::ESC) {
            return Mode::Normal;
        }
        let p = self.cur();
        for (i, y) in self.unit_rows(src, false) {
            if my >= y && my <= y + 5 {
                let u = self.game.country(src).units[i];
                if u.dest == src && mx >= 0x60 && mx < 0xB4 {
                    if let Err(RuleError::Full) = self.game.plot_move(p, src, i, dst) {
                        // destination full: ignore
                    }
                } else if u.dest == dst && mx >= 0xB4 {
                    self.game.unplot_move(src, i);
                }
            }
        }
        Mode::MoveBox { src, dst }
    }

    fn update_repair_box(&mut self, cty: u8, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::RepairBox { cty };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if art::OK_HIT.contains(mx, my) {
            return Mode::Normal;
        }
        let p = self.cur();
        for (i, y) in self.unit_rows(cty, true) {
            if my >= y && my <= y + 5 && mx >= 0x74 && mx <= 0xC0 {
                if let Err(RuleError::NoMoney(_)) = self.game.repair_unit(p, cty, i) {
                    self.set_msg(vec!["YOU DO NOT HAVE".into(), "ENOUGH MONEY TO".into(), "REPAIR THIS CYBORG".into()]);
                }
            }
        }
        Mode::RepairBox { cty }
    }

    fn update_fort_menu(&mut self, cty: u8, chosen: i32, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::FortMenu { cty, chosen };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        let p = self.cur();
        if art::OK_HIT.contains(mx, my) {
            if chosen >= 0 {
                let _ = self.game.buy_fort(p, cty, chosen as u8 + 1);
            }
            return Mode::Normal;
        }
        if art::CLEAR_HIT.contains(mx, my) {
            return Mode::FortMenu { cty, chosen: -1 };
        }
        let c = self.game.country(cty);
        let vault = self.game.vault(p);
        for (i, r) in art::SUB_HIT.iter().enumerate() {
            if r.contains(mx, my) {
                let level = i as u8 + 1;
                if FORT_COST[level as usize] as i64 <= vault && level > c.fort && (i as i32) > chosen {
                    return Mode::FortMenu { cty, chosen: i as i32 };
                }
            }
        }
        Mode::FortMenu { cty, chosen }
    }

    fn update_fact_menu(&mut self, cty: u8, mut counts: [i32; 3], input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::FactMenu { cty, counts };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        let p = self.cur();
        if art::OK_HIT.contains(mx, my) {
            for k in 0..3 {
                for _ in 0..counts[k] {
                    let _ = self.game.buy_factory(p, cty, k);
                }
            }
            return Mode::Normal;
        }
        if art::CLEAR_HIT.contains(mx, my) {
            return Mode::FactMenu { cty, counts: [0; 3] };
        }
        let total: i64 = (0..3).map(|k| counts[k] as i64 * FACTORY_COST[k] as i64).sum();
        let used: i32 = counts.iter().sum();
        let free = self.game.country(cty).free_space() - used;
        for (k, r) in art::SUB_HIT.iter().enumerate() {
            if r.contains(mx, my) && total + FACTORY_COST[k] as i64 <= self.game.vault(p) && free > 0 {
                counts[k] += 1;
            }
        }
        Mode::FactMenu { cty, counts }
    }

    fn update_cyb_menu(&mut self, cty: u8, mut bought: Vec<usize>, mut total_cost: i64, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::CybMenu { cty, bought, total_cost };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        let p = self.cur();
        if art::CYB_OK.contains(mx, my) {
            for &t in &bought {
                let _ = self.game.buy_cyborg(p, cty, t);
            }
            return Mode::Normal;
        }
        if art::CYB_CLEAR.contains(mx, my) {
            return Mode::CybMenu { cty, bought: Vec::new(), total_cost: 0 };
        }
        for t in 0..9 {
            let s = art::CYB_SLX[t];
            if mx >= s[0] && mx <= s[1] && my >= s[2] && my <= s[3] {
                let (cost, _) = self.game.cyborg_price(cty, t);
                let bays = self.game.country(cty).free_bays() as usize;
                if self.game.vault(p) - total_cost - cost < 0 {
                    self.set_msg(vec!["YOU CANNOT AFFORD".into(), "THIS CYBORG.".into(), "SELECTION".into(), "CANCELLED.".into()]);
                } else if bought.len() >= bays {
                    self.set_msg(vec!["ALL FACTORY BAYS".into(), "UTILIZED.".into()]);
                } else {
                    bought.push(t);
                    total_cost += cost;
                }
            }
            let s = art::CYB_PLX[t];
            if mx >= s[0] && mx <= s[1] && my >= s[2] && my <= s[3] {
                return Mode::CybInfo { cty, kind: t, weapon: 0, bought, total_cost };
            }
        }
        Mode::CybMenu { cty, bought, total_cost }
    }

    fn update_cyb_info(&mut self, cty: u8, kind: usize, mut weapon: usize, bought: Vec<usize>, total_cost: i64, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::CybInfo { cty, kind, weapon, bought, total_cost };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if mx >= 245 && mx <= 308 && my >= 7 && my <= 23 || input.pressed(key::ESC) {
            return Mode::CybMenu { cty, bought, total_cost };
        }
        let tabs = [(13, 29), (46, 62), (79, 95), (112, 128)];
        for (w, &(x0, x1)) in tabs.iter().enumerate() {
            if mx >= x0 && mx <= x1 && my >= 130 && my <= 188 && LOADOUT[kind][w].0 >= 0 {
                weapon = w;
            }
        }
        Mode::CybInfo { cty, kind, weapon, bought, total_cost }
    }

    fn update_spy_money(&mut self, cty: u8, mut amount: i64, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::SpyMoney { cty, amount };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        let p = self.cur();
        let vault = self.game.vault(p);
        if art::OK_HIT.contains(mx, my) {
            let report = self.game.spy(p, cty, amount);
            return Mode::SpyReport { report };
        }
        if mx >= 115 && mx <= 138 && my >= 110 && my <= 124 {
            amount -= if amount == 5000 { 5000 } else { 1000 };
            if amount < 0 {
                amount = 0;
            }
        } else if mx >= 115 && mx <= 138 && my >= 94 && my <= 108 {
            amount -= 10000;
            if amount > 0 && amount < 5000 {
                amount = 5000;
            }
            if amount < 0 {
                amount = 0;
            }
        } else if mx >= 180 && mx <= 203 && my >= 110 && my <= 124 {
            amount += if amount == 0 { 5000 } else { 1000 };
            amount = amount.min(100_000).min(vault);
            if amount < 5000 {
                amount = 0;
            }
        } else if mx >= 180 && mx <= 203 && my >= 94 && my <= 108 {
            amount += 10000;
            amount = amount.min(100_000).min(vault);
        }
        Mode::SpyMoney { cty, amount }
    }

    fn update_country_status(&mut self, cty: u8, sub: u8, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::CountryStatus { cty, sub };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if art::OK_HIT.contains(mx, my) {
            return if sub == 0 { Mode::Normal } else { Mode::CountryStatus { cty, sub: 0 } };
        }
        if sub == 0 {
            if mx >= 94 && mx <= 156 && my >= 110 && my <= 120 {
                return Mode::CountryStatus { cty, sub: 1 };
            }
            if mx >= 160 && mx <= 222 && my >= 110 && my <= 120 {
                return Mode::CountryStatus { cty, sub: 2 };
            }
        }
        Mode::CountryStatus { cty, sub }
    }

    fn update_options(&mut self, confirm_quit: bool, input: &Input) -> Mode {
        if !input.clicked() {
            return Mode::Options { confirm_quit };
        }
        let (mx, my) = (input.mouse_x, input.mouse_y);
        if mx >= 0x118 && mx <= 0x12F && my >= 5 && my <= 0x14 || input.pressed(key::ESC) {
            return Mode::Normal;
        }
        if mx >= 0x75 && mx <= 0xB3 && my >= 0xA6 && my <= 0xB1 {
            return Mode::SaveBox;
        }
        if mx >= 0x75 && mx <= 0xB3 && my >= 0xB2 && my <= 0xBD {
            if confirm_quit {
                self.request = Request::Exit;
                return Mode::Normal;
            }
            return Mode::Options { confirm_quit: true };
        }
        if mx >= 0xB6 && mx <= 0xC4 && my >= 0xA6 && my <= 0xB0 {
            self.delay_time = (self.delay_time - 1).max(0);
        }
        if mx >= 0xD5 && mx <= 0xE3 && my >= 0xA6 && my <= 0xB0 {
            self.delay_time = (self.delay_time + 1).min(9);
        }
        Mode::Options { confirm_quit: false }
    }

    fn update_slot_box(&mut self, input: &Input, save: bool) -> Mode {
        if !input.clicked() {
            return if save { Mode::SaveBox } else { Mode::LoadBox };
        }
        const LOADB: [[i32; 4]; 6] = [[112, 148, 62, 70], [112, 148, 82, 90], [112, 148, 102, 110], [162, 198, 62, 70], [162, 198, 82, 90], [162, 198, 102, 110]];
        for (i, b) in LOADB.iter().enumerate() {
            if input.mouse_x >= b[0] && input.mouse_x <= b[1] && input.mouse_y >= b[2] && input.mouse_y <= b[3] {
                if i > 0 {
                    self.request = if save { Request::Save(i) } else { Request::Load(i) };
                    if save {
                        self.set_msg(vec!["GAME HAS BEEN SAVED".into()]);
                    }
                }
                return Mode::Normal;
            }
        }
        if save {
            Mode::SaveBox
        } else {
            Mode::LoadBox
        }
    }

    // ------------------------------------------------------------------ drawing

    /// Draw the screen. Returns a palette override (victory picture) when needed.
    pub fn draw(&self, fb: &mut Surface, sheets: &Sheets) -> Option<crate::gfx::Palette> {
        match &self.mode {
            Mode::Victory { timer } if *timer > 1.5 => {
                fb.blit(&sheets.victory, Rect::new(0, 0, 320, 200), 0, 0);
                return Some(sheets.victory_pal);
            }
            Mode::Summary { battle } => {
                super::summary::draw_battle_summary(fb, sheets, &self.game, battle);
                return None;
            }
            Mode::CybMenu { cty, bought, total_cost } => {
                self.draw_cyb_menu(fb, sheets, *cty, bought, *total_cost);
                return None;
            }
            Mode::CybInfo { kind, weapon, .. } => {
                self.draw_cyb_info(fb, sheets, *kind, *weapon);
                return None;
            }
            Mode::Options { confirm_quit } => {
                self.draw_options(fb, sheets, *confirm_quit);
                return None;
            }
            _ => {}
        }
        fb.blit(&sheets.frame, Rect::new(0, 0, 320, 200), 0, 0);
        self.draw_map(fb, sheets);
        self.draw_message_box(fb, sheets);
        let pressed_op = match &self.mode {
            Mode::Op(op) => Some(*op),
            Mode::MoveDest { .. } | Mode::MoveBox { .. } => Some(Op::Move),
            Mode::RepairBox { .. } => Some(Op::Repair),
            Mode::SpyMoney { .. } | Mode::SpyReport { .. } => Some(Op::Spy),
            Mode::FortMenu { .. } => Some(Op::Fort),
            Mode::FactMenu { .. } => Some(Op::Factory),
            _ => None,
        };
        if let Some(op) = pressed_op {
            let (hit, pressed) = op.button();
            fb.blit(&sheets.button, pressed, hit.x + 3, hit.y + 3);
        }
        match &self.mode {
            Mode::MoveBox { src, dst } => self.draw_move_box(fb, sheets, *src, *dst),
            Mode::RepairBox { cty } => self.draw_repair_box(fb, sheets, *cty),
            Mode::FortMenu { cty, chosen } => self.draw_fort_menu(fb, sheets, *cty, *chosen),
            Mode::FactMenu { cty, counts } => self.draw_fact_menu(fb, sheets, *cty, counts),
            Mode::SpyMoney { cty, amount } => self.draw_spy_money(fb, sheets, *cty, *amount),
            Mode::SpyReport { report } => self.draw_spy_report(fb, sheets, report),
            Mode::CountryStatus { cty, sub } => self.draw_country_status(fb, sheets, *cty, *sub),
            Mode::SaveBox => super::options::draw_slot_box(fb, sheets, "SELECT SAVE GAME"),
            Mode::LoadBox => super::options::draw_slot_box(fb, sheets, "SELECT LOAD GAME"),
            Mode::Annihilated { list } => super::summary::draw_annihilated(fb, sheets, &self.game, list[0]),
            Mode::Victory { .. } => {
                sheets.message(fb, &["CONGRATULATIONS", "HUMAN YOU ARE", "SUPREME"]);
            }
            Mode::Crushed { .. } => {
                sheets.message(fb, &["YOU HAVE BEEN", "CRUSHED"]);
            }
            Mode::HalTurn { .. } => {
                let p = self.cur();
                let name = self.game.players[p as usize].name.clone();
                sheets.message(fb, &["GENERAL", &name, "PLOTS YOUR", "DESTRUCTION"]);
                sheets.flag_icon(fb, p, 294, 167);
            }
            Mode::Resolving => {
                sheets.message(fb, &["RESOLVING", "CONFLICTS..."]);
            }
            _ => {}
        }
        None
    }

    fn visible(&self, c: u8) -> bool {
        let g = &self.game;
        if !g.fog {
            return true;
        }
        let p = self.cur();
        if g.owner(c) == p {
            return true;
        }
        g.country(c).adj().iter().any(|&a| g.owner(a) == p)
    }

    fn draw_map(&self, fb: &mut Surface, sheets: &Sheets) {
        let win = art::MAP_WIN;
        fb.blit(&sheets.world, Rect::new(self.map_x, self.map_y, win.w, win.h), win.x, win.y);
        let g = &self.game;
        let p = self.cur();
        for c in 1..=NUM_COUNTRIES as u8 {
            let cty = g.country(c);
            if cty.owner == NEUTRAL || !self.visible(c) {
                continue;
            }
            let own = cty.owner == p;
            let detail = !g.fog || own;
            let sx = cty.x() as i32 - self.map_x + 16;
            let sy = cty.y() as i32 - self.map_y + 16;
            if detail && cty.attacked_by < 5 {
                fb.blit_clipped(&sheets.button, art::explode(cty.attacked_by), sx + 17, sy, win, Some(0));
            }
            if cty.has_capital() {
                if detail && cty.factory_count() > 0 {
                    fb.blit_clipped(&sheets.button, art::factory(cty.owner), sx, sy, win, Some(0));
                } else {
                    fb.blit_clipped(&sheets.button, art::flag(cty.owner), sx, sy, win, Some(0));
                }
                if detail && cty.fort > 0 {
                    fb.blit_clipped(&sheets.button, art::fort_icon(cty.fort), sx + 19, sy + 4, win, Some(0));
                }
            }
            if detail && cty.num_units() > 0 {
                fb.blit_clipped(&sheets.button, art::cyb(cty.owner), sx - 14, sy + 6, win, Some(0));
            }
        }
    }

    fn draw_message_box(&self, fb: &mut Surface, sheets: &Sheets) {
        let g = &self.game;
        let p = self.cur();
        if !self.msg.is_empty() {
            let lines: Vec<&str> = self.msg.iter().map(|s| s.as_str()).collect();
            sheets.message(fb, &lines);
            return;
        }
        if matches!(self.mode, Mode::Normal) && self.doors_open && self.hover != 0 && g.owner(self.hover) == p {
            let cty = g.country(self.hover);
            let networth = if cty.damage == 100 { 0 } else { (cty.value as f64 * ((100.0 - cty.damage as f64) / 100.0) * 1000.0) as i64 };
            let n = cty.resident_units().count();
            sheets.message(fb, &[cty.name(), &format!("NETWORTH: {}", networth), &format!("CYBORGS: {}", n), &format!("TONNAGE: {}", cty.tonnage())]);
            return;
        }
        if !self.doors_open {
            if self.game_over {
                sheets.message(fb, &["GAME OVER", "CLICK THE DOOR", "TO RETURN TO", "THE MENU"]);
            } else {
                let name = g.players[p as usize].name.clone();
                sheets.message(fb, &["GENERAL", &name, "TAKE THE COMMAND", "CHAIR."]);
                sheets.flag_icon(fb, p, 294, 167);
            }
            return;
        }
        let pl = &g.players[p as usize];
        sheets.message(fb, &[
            &format!("VAULT: {}", pl.vault),
            &format!("YEAR: {}", g.year),
            &format!("SEASON: {}", SEASON_NAMES[(g.season.clamp(1, 7) - 1) as usize]),
            &format!("GENERAL: {}", pl.name),
        ]);
        sheets.flag_icon(fb, p, 294, 167);
    }

    fn draw_move_box(&self, fb: &mut Surface, sheets: &Sheets, src: u8, dst: u8) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        let s = g.country(src);
        let d = g.country(dst);
        f.draw(fb, Font::SmallYellow, 0x5C, 0x1C, "CLICK ON BATTLE");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x23, "UNIT NAME TO MOVE");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x2E, &format!("SOURCE: {} {}", s.name(), s.count));
        let same = s.owner == d.owner;
        let dcount = if same { format!(" {}{}", d.count, if d.count >= 10 { " MAX" } else { "" }) } else { String::new() };
        f.draw(fb, Font::SmallYellow, 0x5C, 0x35, &format!("DESTIN: {}{}", d.name(), dcount));
        f.draw(fb, Font::SmallRed, 0x60, 0x3E, "SOURCE");
        f.draw(fb, Font::SmallRed, 0x8E, 0x3E, "STATUS");
        f.draw(fb, Font::SmallRed, 0xB4, 0x3E, "DESTIN");
        for (i, y) in self.unit_rows(src, false) {
            let u = s.units[i];
            let name = CYBORG_NAMES[u.kind as usize];
            let x = if u.dest == src { 0x60 } else { 0xB4 };
            f.draw(fb, Font::SmallYellow, x, y, name);
            f.draw(fb, if u.status < 100 { Font::SmallRed } else { Font::SmallYellow }, 0x8E + 8, y, &format!("{}", u.status));
        }
        // incoming units (in transit) from adjacent own countries
        let mut y = 0x46 + 6 * self.unit_rows(src, false).len() as i32;
        for &a in s.adj() {
            if g.owner(a) != s.owner {
                continue;
            }
            for u in g.country(a).units.iter() {
                if u.present() && u.dest == src {
                    f.draw(fb, Font::SmallYellow, 0xB4, y, CYBORG_NAMES[u.kind as usize]);
                    f.draw(fb, Font::SmallRed, 0xD8, y, "TRN");
                    y += 6;
                }
            }
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_repair_box(&self, fb: &mut Surface, sheets: &Sheets, cty: u8) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        let c = g.country(cty);
        f.draw(fb, Font::SmallYellow, 0x5C, 0x1C, "CLICK ON BATTLE");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x23, "UNIT NAME TO REPAIR");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x2E, &format!("COUNTRY: {}", c.name()));
        f.draw(fb, Font::SmallYellow, 0x5C, 0x35, &format!("VAULT: {}", g.vault(self.cur())));
        f.draw(fb, Font::SmallRed, 0x74, 0x3E, "BATTLE UNIT");
        f.draw(fb, Font::SmallRed, 0xB2, 0x3E, "STATUS");
        let mut y = 0x46;
        for (i, u) in c.units.iter().enumerate() {
            if u.present() && u.dest == cty {
                f.draw(fb, Font::SmallYellow, 0x74, y, CYBORG_NAMES[u.kind as usize]);
                f.draw(fb, if u.status < 100 { Font::SmallRed } else { Font::SmallYellow }, 0xB2, y, &format!("{}", u.status));
                if u.status < 100 {
                    f.draw(fb, Font::SmallYellow, 0xC4, y, &format!("{}", g.repair_cost(cty, i)));
                }
                y += 6;
            }
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_fort_menu(&self, fb: &mut Surface, sheets: &Sheets, cty: u8, chosen: i32) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        let c = g.country(cty);
        let vault = g.vault(self.cur());
        let temp = if chosen >= 0 { vault - FORT_COST[chosen as usize + 1] as i64 } else { vault };
        for i in 0..3 {
            let r = art::SUB_HIT[i];
            fb.fill_rect(r, 54);
            fb.blit_key(&sheets.button, art::fort_icon(i as u8 + 1), r.x + 7, r.y + 3, 0);
            if chosen == i as i32 {
                fb.rect_outline(Rect::new(r.x, r.y, r.w, r.h), 2);
            }
            let tx = r.right() + 6;
            f.draw(fb, Font::SmallYellow, tx, r.y + 1, FORT_NAMES[i + 1].trim());
            f.draw(fb, Font::SmallYellow, tx, r.y + 7, FORT_WEAPONS[i + 1]);
            f.draw(fb, Font::SmallYellow, tx, r.y + 13, &format!("COST: {}", fmt_money(FORT_COST[i + 1] as i64)));
        }
        f.draw(fb, Font::SmallYellow, 0x60, 0x72, &format!("VAULT: {}", temp));
        let level = if chosen >= 0 { chosen as usize + 1 } else { c.fort as usize };
        f.draw(fb, Font::SmallYellow, 0x60, 0x79, &format!("FORT. LEVEL: {}", FORT_NAMES[level].trim()));
        let free = c.free_space() - if chosen >= 0 && c.fort == 0 { 1 } else { 0 };
        f.draw(fb, Font::SmallYellow, 0x60, 0x80, &format!("SPACE AVAIL: {}", free));
        self.draw_ok_clear(fb, sheets);
    }

    fn draw_ok_clear(&self, fb: &mut Surface, sheets: &Sheets) {
        super::summary::draw_ok(fb, sheets);
        let r = art::CLEAR_HIT;
        fb.fill_rect(r, 5);
        fb.rect_outline(r, 7);
        sheets.fonts.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 7, "CLEAR");
    }

    fn draw_fact_menu(&self, fb: &mut Surface, sheets: &Sheets, cty: u8, counts: &[i32; 3]) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        let c = g.country(cty);
        let vault = g.vault(self.cur());
        let total: i64 = (0..3).map(|k| counts[k] as i64 * FACTORY_COST[k] as i64).sum();
        for i in 0..3 {
            let r = art::SUB_HIT[i];
            fb.blit(&sheets.button, art::FACT_ART[i], r.x + 1, r.y + 1);
            let tx = r.right() + 6;
            f.draw(fb, Font::SmallYellow, tx, r.y + 1, FACTORY_NAMES[i]);
            f.draw(fb, Font::SmallYellow, tx, r.y + 7, &format!("CAPACITY: {}", FACTORY_BAYS[i]));
            f.draw(fb, Font::SmallYellow, tx, r.y + 13, &format!("COST: {}", fmt_money(FACTORY_COST[i] as i64)));
            f.draw(fb, Font::SmallRed, 210, r.y + 7, &format!("{}", counts[i]));
        }
        f.draw(fb, Font::SmallYellow, 0x60, 0x72, &format!("VAULT: {}", vault - total));
        f.draw(fb, Font::SmallYellow, 0x60, 0x79, &format!("TOTAL COST: {}", total));
        f.draw(fb, Font::SmallYellow, 0x60, 0x80, &format!("SPACE AVAIL: {}", c.free_space() - counts.iter().sum::<i32>()));
        self.draw_ok_clear(fb, sheets);
    }

    fn draw_cyb_menu(&self, fb: &mut Surface, sheets: &Sheets, cty: u8, bought: &[usize], total_cost: i64) {
        fb.clear(0);
        let f = &sheets.fonts;
        let g = &self.game;
        let c = g.country(cty);
        let p = self.cur();
        // layout: 3 columns x 3 rows of portrait + stats boxes
        for t in 0..9 {
            let s = art::CYB_SLX[t];
            let pr = art::CYB_PLX[t];
            fb.fill_rect(Rect::xyxy(pr[0], pr[2], pr[1], pr[3]), 0);
            fb.rect_outline(Rect::xyxy(pr[0], pr[2], pr[1], pr[3]), 7);
            let cp = art::CYB_PORTRAITS[t];
            let src = Rect::xyxy(cp[2], cp[3], cp[4], cp[5]);
            fb.blit_key(&sheets.button, src, cp[0], cp[1], 0);
            fb.rect_outline(Rect::xyxy(s[0], s[2], s[1], s[3]), 7);
            let (cost, bt) = g.cyborg_price(cty, t);
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 3, CYBORG_NAMES[t]);
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 10, &format!("WGT: {} TONS", ARMOR[t]));
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 17, &format!("COST: {}", cost));
            f.draw(fb, Font::SmallYellow, s[0] + 3, s[2] + 24, &format!("B-TIME: {}", bt));
        }
        f.draw(fb, Font::Orange, 90, 12, "SELECT CYBORG");
        fb.fill_rect(art::CYB_OK, 5);
        fb.rect_outline(art::CYB_OK, 7);
        f.draw_centered(fb, Font::SmallYellow, (art::CYB_OK.x + art::CYB_OK.right()) / 2, art::CYB_OK.y + 7, "OK");
        fb.fill_rect(art::CYB_CLEAR, 5);
        fb.rect_outline(art::CYB_CLEAR, 7);
        f.draw_centered(fb, Font::SmallYellow, (art::CYB_CLEAR.x + art::CYB_CLEAR.right()) / 2, art::CYB_CLEAR.y + 7, "CLEAR");
        // summary strip
        let mut counts = [0; 9];
        for &b in bought {
            counts[b] += 1;
        }
        for t in 0..9 {
            let (x, y) = if t < 5 { (14, 152 + 6 * t as i32) } else { (110, 152 + 6 * (t as i32 - 5)) };
            f.draw(fb, Font::SmallYellow, x, y, &format!("{:<10} {}", CYBORG_NAMES[t], counts[t]));
        }
        let free_bays = c.free_bays() as usize - bought.len().min(c.free_bays() as usize);
        f.draw(fb, Font::SmallYellow, 200, 152, &format!("FACTORY MAX: {}", c.nbays));
        f.draw(fb, Font::SmallYellow, 200, 158, &format!("FACTORY BAY: {}", free_bays));
        f.draw(fb, Font::SmallYellow, 200, 164, &format!("SPACE AVAIL: {}", 10 - c.num_units()));
        f.draw(fb, Font::SmallYellow, 200, 170, &format!("VAULT: {}", g.vault(p) - total_cost));
        f.draw(fb, Font::SmallYellow, 200, 176, &format!("TOTAL COST: {}", total_cost));
        if c.num_units() >= 10 {
            f.draw(fb, Font::SmallRed, 200, 184, "WARNING: 10 CYBORGS");
            f.draw(fb, Font::SmallRed, 200, 190, "BUILT STAY IN BAYS");
        }
        if !self.msg.is_empty() {
            let lines: Vec<&str> = self.msg.iter().map(|s| s.as_str()).collect();
            let mut y = 184;
            for l in lines.iter().take(2) {
                f.draw(fb, Font::SmallRed, 14, y, l);
                y += 6;
            }
        }
    }

    fn draw_cyb_info(&self, fb: &mut Surface, sheets: &Sheets, kind: usize, weapon: usize) {
        fb.clear(0);
        let f = &sheets.fonts;
        fb.rect_outline(Rect::new(4, 4, 312, 192), 7);
        f.draw(fb, Font::Orange, 12, 10, CYBORG_FULL_NAMES[kind]);
        f.draw(fb, Font::SmallYellow, 12, 30, "SPECIFICATIONS");
        f.draw(fb, Font::SmallYellow, 12, 40, &format!("ARMOR: {}", ARMOR[kind]));
        f.draw(fb, Font::SmallYellow, 12, 47, &format!("SPEED: {}", if FAST[kind] { "FAST" } else { "SLOW" }));
        f.draw(fb, Font::SmallYellow, 12, 54, &format!("COOLING FACTOR: {}", COOLING[kind]));
        f.draw(fb, Font::SmallYellow, 12, 61, "MOST EFFECTIVE TERRAIN:");
        let mut y = 68;
        for &t in EFFECTIVE_TERRAIN[kind] {
            f.draw(fb, Font::SmallYellow, 16, y, &format!("-{}", TERRAIN_NAMES[t as usize]));
            y += 7;
        }
        let (w, n, ammo) = LOADOUT[kind][weapon];
        if w >= 0 {
            let w = w as usize;
            f.draw(fb, Font::SmallYellow, 160, 30, WEAPON_NAMES[w]);
            f.draw(fb, Font::SmallYellow, 160, 40, &format!("NUMBER: {}", n));
            f.draw(fb, Font::SmallYellow, 160, 47, &format!("AMMO: {}", if ammo < 0 { "UNLIMITED".to_string() } else { ammo.to_string() }));
            let ws = WEAPON_SPECS[w];
            f.draw(fb, Font::SmallYellow, 160, 54, &format!("RANGE: {}", ws[0]));
            f.draw(fb, Font::SmallYellow, 160, 61, &format!("IMPACT DAMAGE: {}", ws[1]));
            f.draw(fb, Font::SmallYellow, 160, 68, &format!("IMPACT HEAT: {}", ws[2]));
            f.draw(fb, Font::SmallYellow, 160, 75, &format!("FIRING HEAT: {}", ws[3]));
        }
        f.draw(fb, Font::SmallYellow, 12, 120, "WEAPONS:");
        for (i, lo) in LOADOUT[kind].iter().enumerate() {
            if lo.0 < 0 {
                continue;
            }
            let x = [13, 46, 79, 112][i];
            let r = Rect::xyxy(x, 130, x + 16, 188);
            fb.fill_rect(r, if i == weapon { 2 } else { 5 });
            fb.rect_outline(r, 7);
            f.draw(fb, Font::SmallYellow, x + 2, 134, WEAPON_ABBR[lo.0 as usize]);
            f.draw(fb, Font::SmallYellow, x + 2, 142, &format!("X{}", lo.1));
        }
        let r = Rect::xyxy(245, 7, 308, 23);
        fb.fill_rect(r, 5);
        fb.rect_outline(r, 7);
        f.draw_centered(fb, Font::SmallYellow, 276, 12, "OK");
    }

    fn draw_spy_money(&self, fb: &mut Surface, sheets: &Sheets, cty: u8, amount: i64) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        f.draw(fb, Font::SmallYellow, 0x5C, 0x1C, &format!("INVESTIGATE: {}", g.country(cty).name()));
        f.draw(fb, Font::SmallYellow, 0x5C, 0x2E, "ENTER THE AMOUNT MONEY");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x35, "YOU WISH TO SPEND.");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x44, &format!("VAULT: {}", g.vault(self.cur())));
        f.draw(fb, Font::SmallYellow, 0x5C, 0x52, "AMOUNT TO SPEND");
        f.draw(fb, Font::Orange, 150, 114, &format!("{}", amount));
        for (r, label) in [(Rect::xyxy(115, 94, 138, 108), "--"), (Rect::xyxy(115, 110, 138, 124), "-"), (Rect::xyxy(180, 94, 203, 108), "++"), (Rect::xyxy(180, 110, 203, 124), "+")] {
            fb.fill_rect(r, 5);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::Orange, (r.x + r.right()) / 2, r.y + 4, label);
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_spy_report(&self, fb: &mut Surface, sheets: &Sheets, report: &SpyReport) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let x = 0x5C;
        let mut y = 0x30;
        let mut line = |fb: &mut Surface, s: String| {
            f.draw(fb, Font::SmallYellow, x, y, &s);
            y += 7;
        };
        match report {
            SpyReport::Nothing => line(fb, "NO INFORMATION OBTAINED.".into()),
            SpyReport::Vague { value, space } => {
                line(fb, "THIS COUNTRY APPEARS TO".into());
                line(fb, format!("NETWORTH: {}", value));
                line(fb, format!("BUILD SPACE: {}", space));
            }
            SpyReport::Full { owner, government, value, space, tonnage, fort, factories, upgrades } => {
                let name = if *owner < 5 { self.game.players[*owner as usize].name.clone() } else { "NEUTRAL".into() };
                line(fb, "THIS COUNTRY IS OCCUPIED BY:".into());
                line(fb, name);
                line(fb, format!("GOVERNMENT CONTROL {}", if *government { "IS ESTABLISHED" } else { "NOT ESTABLISHED" }));
                line(fb, format!("NETWORTH: {}", value));
                line(fb, format!("BUILD SPACE: {}", space));
                line(fb, format!("EST. TOTAL TONNAGE: {}", tonnage));
                if let Some(fl) = fort {
                    line(fb, format!("FORTIFICATION LEVEL: {}", FORT_LEVEL_NAMES[*fl as usize]));
                }
                if let Some((n, pot)) = factories {
                    line(fb, format!("NUMBER OF FACTORIES: {}", n));
                    line(fb, format!("FACTORY POTENTIAL: {}", pot));
                }
                if let Some(u) = upgrades {
                    line(fb, format!("FACTORY UPGRADES: {}", u));
                }
            }
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_country_status(&self, fb: &mut Surface, sheets: &Sheets, cty: u8, sub: u8) {
        sheets.dialog(fb);
        let f = &sheets.fonts;
        let g = &self.game;
        let c = g.country(cty);
        f.draw(fb, Font::SmallYellow, 0x5C, 0x1C, "COUNTRY STATUS");
        f.draw(fb, Font::SmallYellow, 0x5C, 0x23, &format!("FOR: {}", c.name()));
        let x = 0x5C;
        let mut y = 0x30;
        let mut line = |fb: &mut Surface, s: String, red: bool| {
            f.draw(fb, if red { Font::SmallRed } else { Font::SmallYellow }, x, y, &s);
            y += 7;
        };
        match sub {
            0 => {
                let networth = if c.damage == 100 { 0 } else { (c.value as f64 * ((100.0 - c.damage as f64) / 100.0) * 1000.0) as i64 };
                line(fb, format!("GOVERNMENT CONTROL {}", if c.has_capital() { "IS ESTABLISHED" } else { "NOT ESTABLISHED" }), false);
                line(fb, format!("NETWORTH: {}", networth), false);
                line(fb, format!("BUILD SPACE: {}", c.space), false);
                line(fb, format!("AVAILABLE SPACE: {}", c.free_space()), false);
                line(fb, format!("FORTIFICATION LEVEL: {}", FORT_LEVEL_NAMES[c.fort as usize]), false);
                line(fb, format!("NUMBER OF FACTORIES: {}", c.factory_count()), false);
                line(fb, format!("FACTORY POTENTIAL: {}", c.nbays), false);
                line(fb, format!("FACTORY UPGRADES: {}", c.enhancements()), false);
                line(fb, format!("TOTAL TONNAGE: {}", c.tonnage()), false);
                if c.space == 1 {
                    line(fb, format!("DAMAGE PERCENTAGE: {}", c.damage), false);
                }
                for (r, label) in [(Rect::xyxy(94, 110, 156, 120), "CYBORG STATUS"), (Rect::xyxy(160, 110, 222, 120), "BAY STATUS")] {
                    fb.fill_rect(r, 5);
                    fb.rect_outline(r, 7);
                    f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 3, label);
                }
            }
            1 => {
                if c.num_units() == 0 {
                    line(fb, "YOU HAVE NO CYBORGS IN THIS COUNTRY".into(), false);
                } else {
                    line(fb, "CYBORG            STATUS".into(), true);
                    for u in c.units.iter() {
                        if u.present() && u.dest == cty {
                            line(fb, format!("{:<12} {}", CYBORG_NAMES[u.kind as usize], u.status), u.status < 100);
                        }
                    }
                    for &a in c.adj() {
                        if g.owner(a) != c.owner {
                            continue;
                        }
                        for u in g.country(a).units.iter() {
                            if u.present() && u.dest == cty {
                                line(fb, format!("{:<12} {}  TRN", CYBORG_NAMES[u.kind as usize], u.status), false);
                            }
                        }
                    }
                    line(fb, format!("TOTAL CYBORGS: {}", c.num_units()), false);
                }
            }
            _ => {
                if c.nbays == 0 {
                    line(fb, "THERE ARE NO FACTORIES IN THIS".into(), false);
                    line(fb, "COUNTRY AND THUS NO FACTORY BAYS".into(), false);
                } else {
                    line(fb, "CYBORGS           C.T.".into(), true);
                    for b in 0..(c.nbays.max(0) as usize).min(24) {
                        let v = c.bays[b];
                        if v == EMPTY_BAY {
                            line(fb, "EMPTY BAY".into(), false);
                        } else if v >= 100 {
                            line(fb, format!("{:<12} {}", CYBORG_NAMES[(v % 100) as usize], v / 100), false);
                        } else if (0..9).contains(&v) {
                            line(fb, format!("{:<12} R", CYBORG_NAMES[v as usize]), true);
                        }
                    }
                }
            }
        }
        super::summary::draw_ok(fb, sheets);
    }

    fn draw_options(&self, fb: &mut Surface, sheets: &Sheets, confirm_quit: bool) {
        fb.clear(0);
        let f = &sheets.fonts;
        let g = &self.game;
        let p = self.cur();
        fb.blit(&sheets.door, Rect::new(0, 0, 320, 152), 0, 0);
        fb.fill_rect(Rect::new(8, 8, 304, 184), 0);
        fb.rect_outline(Rect::new(8, 8, 304, 184), 7);
        f.draw(fb, Font::Orange, 16, 12, "STATISTICS");
        f.draw(fb, Font::Orange, 180, 12, "OPTIONS");
        // statistics
        let mut y = 28;
        let mut line = |fb: &mut Surface, s: String| {
            f.draw(fb, Font::SmallYellow, 16, y, &s);
            y += 7;
        };
        let mut counts = [0i32; 9];
        let mut countries = 0;
        let mut capitals = 0;
        let mut forts = 0;
        let mut enh = 0;
        let mut facts = 0;
        let mut bays = 0;
        for c in 1..=NUM_COUNTRIES {
            let cty = &g.countries[c];
            if cty.owner != p {
                continue;
            }
            countries += 1;
            if cty.has_capital() {
                capitals += 1;
            }
            if cty.fort > 0 {
                forts += 1;
            }
            enh += cty.enhancements();
            facts += cty.factory_count();
            bays += cty.nbays as i32;
            for u in cty.units.iter() {
                if u.present() {
                    counts[u.kind as usize] += 1;
                }
            }
        }
        line(fb, format!("INCOME.......... {}", g.income(p)));
        line(fb, format!("VAULT........... {}", g.vault(p)));
        line(fb, format!("COUNTRIES....... {}", countries));
        line(fb, format!("CAPITALS........ {}", capitals));
        line(fb, format!("FORTIFICATIONS.. {}", forts));
        line(fb, format!("ENHANCEMENTS.... {}", enh));
        line(fb, format!("FACTORIES....... {}", facts));
        line(fb, format!("FACTORY BAYS.... {}", bays));
        line(fb, String::new());
        for t in 0..9 {
            line(fb, format!("{:<14}.. {}", CYBORG_NAMES[t], counts[t]));
        }
        line(fb, format!("TOTAL............ {}", counts.iter().sum::<i32>()));
        // options
        for (r, label) in [(Rect::xyxy(0x75, 0xA6, 0xB3, 0xB1), "SAVE GAME"), (Rect::xyxy(0x75, 0xB2, 0xB3, 0xBD), if confirm_quit { "CONFIRM?" } else { "QUIT GAME" })] {
            fb.fill_rect(r, 5);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 4, label);
        }
        f.draw(fb, Font::SmallYellow, 180, 0x90, &format!("MESSAGE DELAY: {}", self.delay_time));
        for (r, label) in [(Rect::xyxy(0xB6, 0xA6, 0xC4, 0xB0), "-"), (Rect::xyxy(0xD5, 0xA6, 0xE3, 0xB0), "+")] {
            fb.fill_rect(r, 5);
            fb.rect_outline(r, 7);
            f.draw_centered(fb, Font::Orange, (r.x + r.right()) / 2, r.y + 2, label);
        }
        let r = Rect::xyxy(0x118, 5, 0x12F, 0x14);
        fb.fill_rect(r, 5);
        fb.rect_outline(r, 7);
        f.draw_centered(fb, Font::SmallYellow, (r.x + r.right()) / 2, r.y + 5, "OK");
        if confirm_quit {
            f.draw(fb, Font::SmallRed, 180, 0xC2, "RE-CLICK BUTTON");
            f.draw(fb, Font::SmallRed, 180, 0xC9, "TO CONFIRM.");
        }
        let _ = wrap;
    }
}
