//! The battle screen: input, tick timing, end-of-battle result.

use super::render::{BattleArt, Renderer};
use super::sim::{BattleInit, Sim, SimEvent, TICK_SECONDS};
use crate::assets::Assets;
use crate::gfx::Surface;
use crate::input::{key, Input};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    PreStart,
    Running,
    Paused,
    Over { timer_frames: i32 },
}

/// Key set for one side.
#[derive(Clone, Copy)]
pub struct KeySet {
    pub dirs: [[u8; 2]; 8],
    pub fire: [u8; 2],
    pub weapon: [u8; 2],
    pub switch: [u8; 2],
    pub hold: [u8; 2],
    pub retreat: [u8; 2],
}

/// Defender / single player: numeric keypad (plus arrows).
pub const KEYS_RIGHT: KeySet = KeySet {
    dirs: [[key::KP8, key::UP], [key::KP9, 0], [key::KP6, key::RIGHT], [key::KP3, 0], [key::KP2, key::DOWN], [key::KP1, 0], [key::KP4, key::LEFT], [key::KP7, 0]],
    fire: [key::KP_PLUS, key::KP_ENTER],
    weapon: [key::KP0, key::KP_DOT],
    switch: [key::F1, 0],
    hold: [key::F9, 0],
    retreat: [key::F8, 0],
};
/// Attacker / second player: left-hand keys.
pub const KEYS_LEFT: KeySet = KeySet {
    dirs: [[b'W', 0], [b'E', 0], [b'D', 0], [b'C', 0], [b'X', 0], [b'Z', 0], [b'A', 0], [b'Q', 0]],
    fire: [key::SPACE, 0],
    weapon: [key::TAB, b'S'],
    switch: [key::F4, 0],
    hold: [key::F2, 0],
    retreat: [key::F3, 0],
};

pub struct BattleScreen {
    pub sim: Sim,
    pub renderer: Renderer,
    pub phase: Phase,
    pub pc: [u8; 2],
    accum: f32,
    frame_clock: i32,
    /// (result, aret, dret, health per slot)
    pub result: Option<(i32, bool, bool, [i32; 36])>,
    pub sounds: Vec<SimEvent>,
    lkey: [bool; 256],
}

impl BattleScreen {
    pub fn new(assets: &mut Assets, init: &BattleInit) -> Option<BattleScreen> {
        let art = BattleArt::load(assets, init.ttype, init.pc)?;
        let sim = Sim::new(init);
        Some(BattleScreen { sim, renderer: Renderer::new(art), phase: Phase::PreStart, pc: init.pc, accum: 0.0, frame_clock: 0, result: None, sounds: Vec::new(), lkey: [false; 256] })
    }

    fn keyset(&self, p: usize) -> (KeySet, bool) {
        // single human: accept both sets
        let both = self.sim.human[0] != self.sim.human[1];
        if p == 1 || both {
            (KEYS_RIGHT, both)
        } else {
            (KEYS_LEFT, false)
        }
    }

    fn any(input: &Input, ks: &[u8; 2]) -> bool {
        ks.iter().any(|&k| k != 0 && input.key(k))
    }

    fn edge(&mut self, input: &Input, ks: &[u8; 2]) -> bool {
        let mut pressed = false;
        for &k in ks {
            if k == 0 {
                continue;
            }
            if input.key(k) {
                if !self.lkey[k as usize] {
                    pressed = true;
                }
                self.lkey[k as usize] = true;
            } else {
                self.lkey[k as usize] = false;
            }
        }
        pressed
    }

    /// `_CONTROL` for human sides.
    fn read_controls(&mut self, input: &Input) {
        for p in 0..2 {
            if !self.sim.human[p] {
                continue;
            }
            let (ks, both) = self.keyset(p);
            let alt = if both { Some(KEYS_LEFT) } else { None };
            let cur = self.sim.cur[p];
            let mut joy = -1;
            for d in 0..8 {
                if Self::any(input, &ks.dirs[d]) || alt.map_or(false, |a| Self::any(input, &a.dirs[d])) {
                    joy = d as i32;
                    break;
                }
            }
            self.sim.joy[cur] = joy;
            // commands (edge triggered)
            let weapon = self.edge(input, &ks.weapon) || alt.map_or(false, |a| self.edge(input, &a.weapon));
            let switch = self.edge(input, &ks.switch) || alt.map_or(false, |a| self.edge(input, &a.switch));
            let hold = self.edge(input, &ks.hold) || alt.map_or(false, |a| self.edge(input, &a.hold));
            let retreat = self.edge(input, &ks.retreat) || alt.map_or(false, |a| self.edge(input, &a.retreat));
            let fire_held = Self::any(input, &ks.fire) || alt.map_or(false, |a| Self::any(input, &a.fire));
            let mut keyv = 0;
            if weapon {
                keyv = 2;
            } else if switch {
                keyv = 3;
            } else if hold {
                keyv = 4;
            } else if retreat {
                keyv = 5;
            } else {
                let m = &self.sim.mechs[cur];
                let w = super::tables::STATMECH[m.kind.clamp(0, 12) as usize][3 + m.weapon.min(3)];
                if w == 7 && self.sim.lfire[p] && fire_held {
                    self.sim.lfire[p] = true;
                    self.sim.fire[p] = false;
                } else {
                    self.sim.lfire[p] = self.sim.fire[p];
                    self.sim.fire[p] = fire_held;
                    if fire_held {
                        keyv = 1;
                    }
                }
            }
            self.sim.key[cur] = keyv;
            match keyv {
                2 => self.sim.bump_weapon(cur),
                3 => {
                    if !self.sim.bump_mech(p) {
                        self.sim.over = true;
                    }
                }
                4 => self.sim.toggle_hold(p),
                5 => self.sim.toggle_retreat(p),
                _ => {}
            }
        }
    }

    pub fn update(&mut self, input: &Input, dt: f32) {
        match self.phase {
            Phase::PreStart => {
                if input.any_pressed() || input.clicked() {
                    self.phase = Phase::Running;
                }
            }
            Phase::Paused => {
                if input.pressed(key::ESC) {
                    self.phase = Phase::Running;
                }
            }
            Phase::Running => {
                if input.pressed(key::ESC) {
                    self.phase = Phase::Paused;
                    return;
                }
                self.accum += dt.min(0.25);
                let mut ticks = 0;
                while self.accum >= TICK_SECONDS && ticks < 8 {
                    self.accum -= TICK_SECONDS;
                    ticks += 1;
                    self.frame_clock += 1;
                    self.renderer.clock = self.frame_clock;
                    self.read_controls(input);
                    self.sim.check_holds();
                    let msize = self.renderer.art.msize;
                    self.sim.tick(&msize);
                    let over = self.sim.frame_check() || self.sim.over;
                    self.sounds.extend(self.sim.events.drain(..));
                    if over {
                        self.result = Some(self.sim.result());
                        self.phase = Phase::Over { timer_frames: 0 };
                        break;
                    }
                }
            }
            Phase::Over { timer_frames } => {
                self.phase = Phase::Over { timer_frames: timer_frames + 1 };
            }
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self.phase, Phase::Over { timer_frames } if timer_frames > 30)
    }

    pub fn draw(&self, fb: &mut Surface) {
        self.renderer.draw_frame(fb);
        self.renderer.draw_view(fb, &self.sim, 0);
        self.renderer.draw_view(fb, &self.sim, 1);
        self.renderer.draw_hud(fb, &self.sim);
        if self.phase == Phase::PreStart {
            self.renderer.draw_prestart(fb, self.pc);
        }
    }

    pub fn palette(&self) -> crate::gfx::Palette {
        self.renderer.art.pal
    }
}
