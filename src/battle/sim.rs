//! Real-time battle simulation (ACTION.PRG `_DOCLICK` and the mech AI), one step per 18.2 Hz tick.

use super::map::{BattleMap, MAP_W};
use super::tables::*;

pub const NMECH: usize = 32;
pub const NO_TARGET: i16 = -40;
pub const TICK_SECONDS: f32 = 1.0 / 18.2;

#[derive(Clone, Copy, Debug, Default)]
pub struct Mech {
    pub x: i32,
    pub y: i32,
    pub nbullets: i32,
    pub avoid_dir: i32,
    pub avoid_turn: i32,
    pub crowd: i32,
    pub missile: [i32; 4],
    pub missile_shot: [i32; 4],
    pub target: i16,
    pub tdist: i32,
    pub nmissiles: i32,
    pub reload: i32,
    pub kind: i32,
    pub dir: i32,
    pub want_dir: i32,
    pub status: i32,
    pub lockout: i32,
    pub side: i32,
    pub heat: i32,
    pub anim: i32,
    pub in_water: bool,
    pub weapon: usize,
    pub hold: bool,
    pub avoid_cnt: i32,
    pub bump: i32,
    pub goal_x: i32,
    pub goal_y: i32,
    pub ammo: [i32; 4],
    pub hits: i32,
    pub redraw: i32,
    /// true for the slot being an actual unit (type != -1)
    pub exists: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Bullet {
    pub x: i32,
    pub y: i32,
    pub vx: i32,
    pub vy: i32,
    pub dir: i32,
    pub wtype: usize,
    pub owner: usize,
    pub age: i32,
    pub range: i32,
    pub alive: bool,
    pub seq: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub x: i32,
    pub y: i32,
    pub alive: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MechInit {
    pub x: i32,
    pub y: i32,
    /// 0..8 cyborg, 9..12 turret, -1 empty
    pub kind: i32,
    pub health: i32,
    pub dir: i32,
    pub side: i32,
}

#[derive(Clone, Debug)]
pub struct BattleInit {
    pub map: BattleMap,
    /// terrain 0..4 (arctic, conifer, desert, volcanic, city)
    pub ttype: usize,
    pub mechs: [MechInit; 36],
    pub pieces: Vec<Piece>,
    pub att_goal: usize,
    pub att_agg: usize,
    pub human: [bool; 2],
    pub cur: [usize; 2],
    /// player ids (0..4) of attacker / defender, for colours
    pub pc: [u8; 2],
}

pub struct Sim {
    pub map: BattleMap,
    pub hitloc: Vec<u8>,
    pub ttype: usize,
    pub mechs: [Mech; NMECH],
    pub bullets: Vec<Bullet>,
    pub pieces: Vec<Piece>,
    pub itime: i32,
    pub joy: [i32; NMECH],
    pub key: [i32; NMECH],
    /// -1 = AI controlled
    pub cfl: [i32; NMECH],
    pub cur: [usize; 2],
    pub human: [bool; 2],
    pub hold_override: [i32; 2],
    pub lho: [i32; 2],
    pub retreat: [bool; 2],
    pub rdir: [i32; 2],
    pub fire: [bool; 2],
    pub lfire: [bool; 2],
    pub att_goal: usize,
    pub att_agg: i32,
    pub seq: u32,
    /// events for sound / view: (kind, x, y)
    pub events: Vec<SimEvent>,
    pub over: bool,
    /// set when a side needs its view re-centred (SWITCHMECH / missile view)
    pub recenter: [bool; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimEvent {
    Fire(usize, usize),
    Boom(usize),
    Crit,
    Switch(usize),
}

#[inline]
fn stat(kind: i32) -> &'static [i16; 19] {
    &STATMECH[kind.clamp(0, 12) as usize]
}
#[inline]
fn weapon_of(kind: i32, slot: usize) -> i32 {
    stat(kind)[3 + slot] as i32
}
#[inline]
pub fn wtab(w: usize) -> &'static [i16] {
    let w = w.min(7);
    &WEAPONS[w * 11..w * 11 + 11]
}
#[inline]
fn anim_len(a: usize) -> i32 {
    ANIMATION[a][1] as i32
}

pub fn ndir(cur: i32, want: i32) -> i32 {
    if cur == want {
        return cur;
    }
    let mut d = want - cur;
    if d < -4 {
        d += 8;
    }
    if d > 4 {
        d -= 8;
    }
    if d < 0 {
        (cur + 7) % 8
    } else {
        (cur + 9) % 8
    }
}

impl Sim {
    pub fn new(init: &BattleInit) -> Sim {
        let mut mechs = [Mech::default(); NMECH];
        for i in 0..NMECH {
            let r = init.mechs[i];
            let m = &mut mechs[i];
            m.x = r.x;
            m.y = r.y;
            m.kind = r.kind;
            m.side = r.side;
            m.dir = r.dir.rem_euclid(8);
            m.target = NO_TARGET;
            m.bump = -1;
            if r.kind < 0 {
                m.status = -1;
                m.hits = 1;
                m.exists = false;
            } else {
                m.status = 0;
                m.hits = ((stat(r.kind)[1] as i64 * r.health as i64) / 100) as i32;
                m.exists = true;
                for s in 0..4 {
                    m.ammo[s] = stat(r.kind)[7 + s] as i32;
                }
                m.hold = true;
                if m.status == 0 {
                    m.status = 1;
                }
            }
        }
        let hitloc: Vec<u8> = init.map.tiles.iter().map(|&t| WALL_POWER[(t as usize).min(179)]).collect();
        let rdir = [(mechs[0].dir + 4) % 8, (mechs[10].dir + 4) % 8];
        let mut s = Sim {
            map: init.map.clone(),
            hitloc,
            ttype: init.ttype.min(4),
            mechs,
            bullets: Vec::new(),
            pieces: init.pieces.clone(),
            itime: 0,
            joy: [-1; NMECH],
            key: [0; NMECH],
            cfl: [-1; NMECH],
            cur: init.cur,
            human: init.human,
            hold_override: [-1; 2],
            lho: [-1; 2],
            retreat: [false; 2],
            rdir,
            fire: [false; 2],
            lfire: [false; 2],
            att_goal: init.att_goal.min(3),
            att_agg: PRIOR[init.att_agg.min(4)] as i32,
            seq: 0,
            events: Vec::new(),
            over: false,
            recenter: [true; 2],
        };
        for p in 0..2 {
            if s.human[p] {
                s.cfl[s.cur[p]] = 0;
            }
        }
        s
    }

    pub fn alive(&self, m: usize) -> bool {
        self.mechs[m].status >= 0
    }

    /// Side p's current mech; switches to the next one if needed. Returns false when none left.
    pub fn bump_mech(&mut self, p: usize) -> bool {
        let old = self.cur[p];
        self.cfl[old] = -1;
        let start = (old + 1) % 20;
        let mut k = start;
        loop {
            if self.mechs[k].status >= 0 && self.mechs[k].side == p as i32 {
                self.cur[p] = k;
                if self.human[p] {
                    self.cfl[k] = 0;
                }
                self.recenter[p] = true;
                self.events.push(SimEvent::Switch(p));
                return true;
            }
            k = (k + 1) % 20;
            if k == start {
                break;
            }
        }
        false
    }

    pub fn bump_weapon(&mut self, m: usize) {
        let kind = self.mechs[m].kind;
        let mut w = self.mechs[m].weapon;
        for _ in 0..4 {
            w = (w + 1) % 4;
            if stat(kind)[11 + w] != 0 {
                break;
            }
        }
        self.mechs[m].weapon = w;
    }

    pub fn toggle_hold(&mut self, p: usize) {
        self.retreat[p] = false;
        self.hold_override[p] = if self.hold_override[p] == -1 { 1 - self.lho[p].max(0) } else { 1 - self.hold_override[p] };
    }

    pub fn toggle_retreat(&mut self, p: usize) {
        self.retreat[p] = !self.retreat[p];
        self.hold_override[p] = 0;
    }

    /// `_CHECKHOLDS` for both sides.
    pub fn check_holds(&mut self) {
        for p in 0..2 {
            let hold = if self.hold_override[p] == -1 {
                let mut h = false;
                for i in 0..10 {
                    let m = &self.mechs[p * 10 + i];
                    if m.status < 0 {
                        continue;
                    }
                    for s in 0..3 {
                        if weapon_of(m.kind, s) == 7 && m.ammo[s] != 0 {
                            h = true;
                        }
                    }
                }
                for i in 20..32 {
                    let m = &self.mechs[i];
                    if m.status >= 0 && m.side == p as i32 && m.kind == 12 {
                        h = true;
                    }
                }
                h as i32
            } else {
                self.hold_override[p]
            };
            for i in 0..10 {
                self.mechs[p * 10 + i].hold = hold != 0;
            }
            self.lho[p] = hold;
        }
    }

    pub fn is_blocking_tile(&self, tile: u8) -> bool {
        let c = &COL[self.ttype * 6..self.ttype * 6 + 6];
        let t = tile as i16;
        (c[0] <= t && t <= c[1]) || (c[2] <= t && t <= c[3]) || (c[4] <= t && t <= c[5])
    }

    fn tile_at(&self, x: i32, y: i32) -> u8 {
        self.map.get(x.div_euclid(32), y.div_euclid(32))
    }

    fn destroy_wall(&self, tile: u8, m: usize) -> bool {
        let mm = &self.mechs[m];
        if mm.target < 0 && mm.target > NO_TARGET {
            return true;
        }
        if tile == 2 {
            return false;
        }
        if self.ttype == 1 && tile < 60 {
            return true;
        }
        if self.ttype == 4 && mm.side == 0 && self.att_goal == 0 {
            return true;
        }
        false
    }

    /// TESTSHIT for one corner: returns blocked flag; may set bump target / retreat exit.
    fn testshit(&mut self, tx: i32, ty: i32, m: usize, flag: &mut bool) {
        if *flag {
            return;
        }
        let tile = self.map.get(tx, ty);
        if tile == 2 {
            if !self.retreat[(m / 10).min(1)] || m >= 20 {
                *flag = true;
                self.mechs[m].avoid_cnt = -1;
            } else {
                self.mechs[m].status = -4;
                return;
            }
        } else if self.is_blocking_tile(tile) {
            *flag = true;
        }
        if *flag && self.mechs[m].bump == -1 && self.destroy_wall(tile, m) {
            self.mechs[m].goal_x = tx * 32 + 16;
            self.mechs[m].goal_y = ty * 32 + 16;
            self.mechs[m].bump = -(tile as i32);
        }
    }

    fn lcheck(&mut self, x: i32, y: i32, m: usize) -> bool {
        let h = stat(self.mechs[m].kind)[18] as i32 / 2 - 4;
        let mut flag = false;
        self.testshit((x - h).div_euclid(32), (y - h).div_euclid(32), m, &mut flag);
        self.testshit((x - h).div_euclid(32), (y + h).div_euclid(32), m, &mut flag);
        self.testshit((x + h).div_euclid(32), (y - h).div_euclid(32), m, &mut flag);
        self.testshit((x + h).div_euclid(32), (y + h).div_euclid(32), m, &mut flag);
        flag
    }

    fn coll(&self, m: usize) -> i32 {
        stat(self.mechs[m].kind)[18] as i32
    }

    /// TESTC: -1 on collision.
    fn testc(&mut self, a: usize, b: usize) -> bool {
        let r = (self.coll(a) + self.coll(b) + 10) / 2;
        let (ax, ay) = (self.mechs[a].x, self.mechs[a].y);
        let (bx, by) = (self.mechs[b].x, self.mechs[b].y);
        if bx - r < ax && ax < bx + r && by - r < ay && ay < by + r {
            if self.mechs[a].side != self.mechs[b].side {
                if self.mechs[a].bump == -1 {
                    self.mechs[a].bump = b as i32;
                    self.mechs[a].goal_x = bx;
                    self.mechs[a].goal_y = by;
                }
                if self.mechs[b].bump == -1 {
                    self.mechs[b].bump = a as i32;
                    self.mechs[b].goal_x = ax;
                    self.mechs[b].goal_y = ay;
                }
            }
            return true;
        }
        if self.mechs[b].bump == a as i32 {
            self.mechs[b].bump = -1;
        }
        if self.mechs[a].bump == b as i32 {
            self.mechs[a].bump = -1;
        }
        false
    }

    fn test_position(&mut self, x: i32, y: i32, m: usize) -> bool {
        if self.lcheck(x, y, m) {
            return true;
        }
        let base = self.coll(m) + 10;
        for k in 0..NMECH {
            if k == m {
                continue;
            }
            let r = (self.coll(k) + base) / 2;
            let (kx, ky) = (self.mechs[k].x, self.mechs[k].y);
            if kx - r < x && x < kx + r && ky - r < y && y < ky + r {
                if self.mechs[m].side == self.mechs[k].side {
                    self.mechs[m].crowd += 1;
                }
                return true;
            }
        }
        false
    }

    fn add_heat(&mut self, m: usize, h: i32) {
        let mm = &mut self.mechs[m];
        mm.heat += h;
        if mm.heat > 100 {
            mm.lockout = -3;
            mm.status = 0;
        }
        if mm.heat > 150 {
            mm.heat = 150;
            mm.status = -2;
            mm.hits = 0;
            mm.anim = 0;
            self.events.push(SimEvent::Boom(m));
        }
    }

    fn add_bullet(&mut self, b: Bullet) -> usize {
        self.seq += 1;
        let mut b = b;
        b.seq = self.seq;
        b.alive = true;
        if let Some(i) = self.bullets.iter().position(|x| !x.alive) {
            self.bullets[i] = b;
            i
        } else {
            self.bullets.push(b);
            self.bullets.len() - 1
        }
    }

    fn fire_weapon(&mut self, m: usize) {
        if self.mechs[m].nmissiles > 0 {
            return;
        }
        let kind = self.mechs[m].kind;
        let slot = self.mechs[m].weapon.min(3);
        let w = weapon_of(kind, slot);
        if w < 0 {
            return;
        }
        let w = w as usize;
        let half = stat(kind)[17] as i32 / 2;
        let bx = self.mechs[m].x - half + 8;
        let by = self.mechs[m].y - half + 8;
        let d = self.mechs[m].dir as usize;
        let ti = TRANS_TABLES[(kind as usize * 4 + slot).min(51)];
        let fsx = FIRESTUF[d * 2] as i32;
        let fsy = FIRESTUF[d * 2 + 1] as i32;
        let mut fs = fsx.abs() + fsy.abs();
        if fs == 0 {
            fs = 14;
        }
        let shots = stat(kind)[11 + slot] as i32;
        let wt = wtab(w);
        for i in 0..shots.max(0) as usize {
            if self.mechs[m].ammo[slot] == 0 {
                continue;
            }
            self.mechs[m].nbullets += 1;
            if w == 7 {
                self.mechs[m].nmissiles = i as i32 + 1;
                self.lfire[self.mechs[m].side as usize] = true;
            }
            if self.mechs[m].ammo[slot] != -1 {
                self.mechs[m].ammo[slot] -= 1;
            }
            self.add_heat(m, wt[3] as i32);
            self.events.push(SimEvent::Fire(m, w));
            let (ox, oy) = if ti >= 0 {
                let t = MUZZLE[ti as usize];
                let base = i * 16 + d * 2;
                if base + 1 < t.len() {
                    (t[base] as i32, t[base + 1] as i32)
                } else {
                    (8, 8)
                }
            } else {
                (8, 8)
            };
            let id = self.add_bullet(Bullet {
                x: bx + ox,
                y: by + oy,
                vx: fsx * wt[1] as i32 / 10,
                vy: fsy * wt[1] as i32 / 10,
                dir: d as i32,
                wtype: w,
                owner: m,
                age: 0,
                range: wt[0] as i32 * fs / 10,
                alive: true,
                seq: 0,
            });
            if i < 4 {
                self.mechs[m].missile[i] = id as i32;
                self.mechs[m].missile_shot[i] = i as i32;
            }
        }
    }

    fn clear_missile(&mut self, bi: usize, flag: bool) {
        let owner = self.bullets[bi].owner;
        let mm = &mut self.mechs[owner];
        let n = mm.nmissiles.max(0) as usize;
        if let Some(pos) = (0..n.min(4)).find(|&k| mm.missile[k] == bi as i32) {
            for k in pos..3 {
                mm.missile[k] = mm.missile[k + 1];
                mm.missile_shot[k] = mm.missile_shot[k + 1];
            }
            mm.nmissiles -= 1;
            if mm.nmissiles <= 0 {
                mm.nmissiles = 0;
                let side = mm.side as usize;
                if self.cur[side] == owner {
                    if flag {
                        mm.redraw = -self.bullets[bi].age;
                    } else {
                        self.recenter[side] = true;
                    }
                }
            }
        }
    }

    fn explode_bullet(&mut self, bi: usize) {
        let w = self.bullets[bi].wtype;
        let n = anim_len((25 + wtab(w)[8] as i32).clamp(25, 29) as usize);
        self.bullets[bi].age = -n;
        if w == 7 {
            self.clear_missile(bi, true);
        }
    }

    fn target_pos(&self, m: usize) -> (i32, i32) {
        let t = self.mechs[m].target;
        if t >= 0 {
            let k = &self.mechs[(t as usize).min(31)];
            (k.x, k.y)
        } else if t > NO_TARGET {
            let k = (-t - 1) as usize;
            if k < self.pieces.len() {
                (self.pieces[k].x + 16, self.pieces[k].y + 16)
            } else {
                (self.mechs[m].x, self.mechs[m].y)
            }
        } else {
            (self.mechs[m].x, self.mechs[m].y)
        }
    }

    fn getd(&mut self, dx: i32, dy: i32, m: usize) -> i32 {
        let qx = if dx > 7 { 1 } else if dx < -7 { -1 } else { 0 };
        let qy = if dy > 7 { 1 } else if dy < -7 { -1 } else { 0 };
        let mut d = 0;
        if dx.abs() > 7 {
            d += dx.abs();
        }
        if dy.abs() > 7 {
            d += dy.abs();
        }
        self.mechs[m].tdist = d;
        DIRTAB[((qy + 1) * 3 + qx + 1) as usize] as i32
    }

    fn compass(&mut self, m: usize) -> i32 {
        let side = (m / 10).min(1);
        if self.retreat[side] && m < 20 {
            return self.rdir[side];
        }
        let (tx, ty) = self.target_pos(m);
        let (mx, my) = (self.mechs[m].x, self.mechs[m].y);
        self.getd(tx - mx, ty - my, m)
    }

    /// SELECTT: nearest weighted enemy or structure; sets tr.
    fn selectt(&self, m: usize) -> i16 {
        let mm = &self.mechs[m];
        let g = &GOALTABLE[self.att_goal * 4..self.att_goal * 4 + 4];
        let agg = self.att_agg.max(1);
        let mut best_s = 32000;
        let mut sidx = 0i16;
        if mm.side == 0 {
            for (i, p) in self.pieces.iter().enumerate() {
                if !p.alive {
                    continue;
                }
                let mut d = (p.x + 16 - mm.x).abs() + (p.y + 16 - mm.y).abs();
                let w = if i == 0 { g[1] } else { g[2] } as i32;
                d /= (w * agg).max(1);
                if d < best_s {
                    best_s = d;
                    sidx = -(i as i16);
                }
            }
        }
        let mut best_m = 32000;
        let mut midx = 0i16;
        for j in 0..NMECH {
            let k = &self.mechs[j];
            if k.status < 0 || k.side == mm.side {
                continue;
            }
            let mut d = (k.x - mm.x).abs() + (k.y - mm.y).abs();
            let w = if j < 20 { g[0] } else { g[3] } as i32;
            d /= (w * agg).max(1);
            if d < best_m {
                best_m = d;
                midx = j as i16;
            }
        }
        if best_m == 32000 {
            return NO_TARGET;
        }
        if best_s <= best_m {
            sidx - 1
        } else {
            midx
        }
    }

    fn select_weapon(&self, m: usize) -> i32 {
        let mm = &self.mechs[m];
        let kind = mm.kind.clamp(0, 12) as usize;
        if kind > 8 {
            return 0;
        }
        for k in 0..4 {
            let s = PRIORITIES[kind * 4 + k];
            if s == -1 {
                break;
            }
            let s = s as usize;
            let w = weapon_of(mm.kind, s);
            if mm.ammo[s] != 0 && w >= 0 && (wtab(w as usize)[0] as u32) > (mm.tdist as u32) {
                return s as i32;
            }
        }
        -1
    }

    /// GETTD: best firing direction at any mech of `tside` within range; returns (dir, dist).
    fn gettd(&self, m: usize, range: i32, tside: i32) -> (i32, i32) {
        let mm = &self.mechs[m];
        let mut best = 32000;
        let mut bd = -1;
        let rd = if m < 20 { self.rdir[m / 10] } else { -9 };
        for k in 0..NMECH {
            let km = &self.mechs[k];
            if km.status < 0 || k == m || km.side != tside {
                continue;
            }
            let dx = km.x - mm.x;
            let dy = km.y - mm.y;
            let (ax, ay) = (dx.abs(), dy.abs());
            for s in 0..8i32 {
                if rd >= 0 && (s == rd || (s + 1) % 8 == rd || (s + 7) % 8 == rd) {
                    continue;
                }
                if ax + ay >= range || ax + ay >= best {
                    continue;
                }
                let d2x = D2[(s * 2) as usize] as i32;
                let d2y = D2[(s * 2 + 1) as usize] as i32;
                if (d2y * ax - d2x * ay).abs() >= 32 {
                    continue;
                }
                let d1x = D1[(s * 2) as usize] as i32;
                let d1y = D1[(s * 2 + 1) as usize] as i32;
                if d1x * dx < 0 || d1y * dy < 0 {
                    continue;
                }
                best = ax + ay;
                bd = s;
            }
        }
        (bd, best)
    }

    /// GETTD2: same test for one direction; enemy side tests only the current target.
    fn gettd2(&self, m: usize, range: i32, tside: i32, dir: i32) -> (i32, i32) {
        let mm = &self.mechs[m];
        let mut best = 32000;
        let mut bd = -1;
        let s = dir as usize;
        let check = |dx: i32, dy: i32, best: &mut i32, bd: &mut i32| {
            let (ax, ay) = (dx.abs(), dy.abs());
            if ax + ay >= range || ax + ay >= *best {
                return;
            }
            let d2x = D2[s * 2] as i32;
            let d2y = D2[s * 2 + 1] as i32;
            if (d2y * ax - d2x * ay).abs() >= 48 {
                return;
            }
            let d1x = D1[s * 2] as i32;
            let d1y = D1[s * 2 + 1] as i32;
            if d1x * dx < 0 || d1y * dy < 0 {
                return;
            }
            *best = ax + ay;
            *bd = dir;
        };
        if tside != mm.side {
            let t = mm.target;
            if t == NO_TARGET {
                return (-1, best);
            }
            let (tx, ty) = self.target_pos(m);
            if t >= 0 && self.mechs[(t as usize).min(31)].status < 0 {
                return (-1, best);
            }
            check(tx - mm.x, ty - mm.y, &mut best, &mut bd);
        } else {
            for k in 0..NMECH {
                let km = &self.mechs[k];
                if km.status < 0 || k == m || km.side != tside {
                    continue;
                }
                check(km.x - mm.x, km.y - mm.y, &mut best, &mut bd);
            }
        }
        (bd, best)
    }

    fn check_obstacle(&mut self, m: usize) {
        self.mechs[m].avoid_cnt += 1;
        if self.mechs[m].avoid_cnt == 1 {
            let (tx, ty) = self.target_pos(m);
            let dx = (tx - self.mechs[m].x).abs();
            let dy = (ty - self.mechs[m].y).abs();
            let mut t = DIR_CORRECT[self.mechs[m].dir.rem_euclid(8) as usize] as i32;
            if dy < dx {
                t = -t;
            }
            self.mechs[m].avoid_turn = t;
        }
        let mm = &mut self.mechs[m];
        mm.avoid_dir = (mm.dir + mm.avoid_turn + 8) & 7;
    }

    fn collision_matrix(&mut self, m: usize) -> [bool; 3] {
        let sp = stat(self.mechs[m].kind)[2] as i32;
        let mut obs = [false; 3];
        for s in [0usize, 2] {
            let d = ((self.mechs[m].dir + s as i32 + 7) % 8) as usize;
            let x = self.mechs[m].x + DIR[d * 2] as i32 * sp;
            let y = self.mechs[m].y + DIR[d * 2 + 1] as i32 * sp;
            obs[s] = self.test_position(x, y, m);
        }
        obs
    }

    fn domech(&mut self, m: usize) {
        if (self.itime & 15) == 1 {
            self.mechs[m].target = self.selectt(m);
        }
        self.key[m] = 0;
        if self.mechs[m].target == NO_TARGET {
            self.joy[m] = -1;
            return;
        }
        let cdir = self.compass(m);
        let kind = self.mechs[m].kind;
        if self.mechs[m].hold {
            let w = self.select_weapon(m);
            if w != -1 && self.mechs[m].weapon != w as usize {
                self.mechs[m].weapon = w as usize;
            }
            if w != -1 && weapon_of(kind, w as usize) == 7 {
                self.key[m] = 1;
            }
            let w0 = weapon_of(kind, 0).max(0) as usize;
            let range = wtab(w0)[0] as i32;
            let side = self.mechs[m].side;
            let (fdir, fdist) = self.gettd(m, range, side);
            let (edir, edist) = self.gettd(m, range, 1 - side);
            if edir == -1 {
                return;
            }
            if fdist <= edist && fdir == edir {
                return;
            }
            if self.mechs[m].dir != edir {
                self.joy[m] = edir;
            } else {
                self.key[m] = 1;
            }
            return;
        }
        if self.mechs[m].bump != -1 {
            let bump = self.mechs[m].bump;
            let w = if (0..=32).contains(&bump) { self.select_weapon(m) } else { 0 };
            self.mechs[m].avoid_cnt = 0;
            if w >= 0 && self.mechs[m].weapon != w as usize {
                self.mechs[m].weapon = w as usize;
            }
            let (gx, gy) = (self.mechs[m].goal_x, self.mechs[m].goal_y);
            let (mx, my) = (self.mechs[m].x, self.mechs[m].y);
            self.joy[m] = self.getd(gx - mx, gy - my, m);
            if self.joy[m] == self.mechs[m].dir {
                self.key[m] = 1;
            }
            if bump < 0 || bump > 33 {
                let tile = self.map.get(gx.div_euclid(32), gy.div_euclid(32));
                if ((tile as i32 + bump) & 0xFF) != 0 {
                    self.mechs[m].bump = -1;
                }
                if self.mechs[m].heat > 75 {
                    self.key[m] = 0;
                }
                return;
            }
            if self.mechs[(bump as usize).min(31)].status != -1 {
                return;
            }
            self.mechs[m].bump = -1;
            return;
        }
        let w = self.select_weapon(m);
        if w != -1 && self.mechs[m].weapon != w as usize {
            self.mechs[m].weapon = w as usize;
        }
        let w0 = weapon_of(kind, 0).max(0) as usize;
        let range = wtab(w0)[0] as i32;
        let side = self.mechs[m].side;
        let facing = self.mechs[m].dir;
        let (fdir, fdist) = self.gettd2(m, range, side, facing);
        let (edir, edist) = self.gettd2(m, range, 1 - side, facing);
        if edir != -1 && (fdist > edist || fdir != edir) {
            self.key[m] = 1;
        }
        if self.mechs[m].status == 0 && self.mechs[m].avoid_cnt == 0 {
            self.check_obstacle(m);
            self.joy[m] = self.mechs[m].avoid_dir;
            return;
        }
        let ac = self.mechs[m].avoid_cnt;
        if ac > 0 && ac != 32 && self.mechs[m].crowd <= 32 {
            let obs = self.collision_matrix(m);
            let turn = self.mechs[m].avoid_turn;
            let idx = (1 - turn).clamp(0, 2) as usize;
            if !obs[idx] {
                let mm = &mut self.mechs[m];
                mm.avoid_dir = (mm.avoid_dir - turn + 8) & 7;
                mm.avoid_cnt -= 1;
            } else if self.mechs[m].status == 0 {
                self.check_obstacle(m);
            }
            self.joy[m] = self.mechs[m].avoid_dir;
            return;
        }
        self.mechs[m].avoid_cnt = 0;
        self.mechs[m].crowd = 0;
        self.joy[m] = cdir;
    }

    fn dotur(&mut self, m: usize) {
        if self.mechs[m].kind == 12 {
            self.mechs[m].target = self.selectt(m);
            if self.mechs[m].target != NO_TARGET {
                self.key[m] = 1;
            }
            return;
        }
        let w0 = weapon_of(self.mechs[m].kind, 0).max(0) as usize;
        let range = wtab(w0)[0] as i32;
        let side = self.mechs[m].side;
        let (fdir, fdist) = self.gettd(m, range, side);
        let (edir, edist) = self.gettd(m, range, 1 - side);
        self.joy[m] = edir;
        if edir == -1 {
            return;
        }
        if fdist <= edist && edir == fdir {
            return;
        }
        self.key[m] = 1;
    }

    fn domissile(&mut self, m: usize) {
        let bi = self.mechs[m].missile[0];
        if bi < 0 || bi as usize >= self.bullets.len() {
            return;
        }
        let (tx, ty) = self.target_pos(m);
        let b = self.bullets[bi as usize];
        let dx = tx - b.x;
        let dy = ty - b.y;
        let sx = if dx > 7 { 1 } else if dx < -7 { -1 } else { 0 };
        let sy = if dy > 7 { 1 } else if dy < -7 { -1 } else { 0 };
        self.joy[m] = DIRTAB[((sy + 1) * 3 + sx + 1) as usize] as i32;
    }

    fn getcomp(&mut self, m: usize) {
        if self.cfl[m] != -1 {
            return;
        }
        let t = self.mechs[m].target;
        let dead = if t >= 0 {
            self.mechs[(t as usize).min(31)].status == -1
        } else if t > NO_TARGET {
            let k = (-t - 1) as usize;
            k >= self.pieces.len() || !self.pieces[k].alive
        } else {
            true
        };
        if dead {
            self.mechs[m].target = self.selectt(m);
        }
        self.key[m] = 0;
        self.joy[m] = -1;
        if self.mechs[m].redraw > 0 {
            return;
        }
        if self.mechs[m].nmissiles > 0 {
            self.domissile(m);
        } else if self.mechs[m].kind > 8 {
            self.dotur(m);
        } else {
            self.domech(m);
        }
    }

    /// DCHECK
    fn dcheck(bx: i32, by: i32, x0: i32, y0: i32, x1: i32, y1: i32, dir: i32) -> bool {
        match dir {
            0 | 4 => x0 <= bx && bx <= x1,
            2 | 6 => y0 <= by && by <= y1,
            1 | 5 => x0 + y0 <= bx + by && bx + by <= x1 + y1,
            _ => x0 - y1 <= bx - by && bx - by <= x1 - y0,
        }
    }

    /// One simulation tick (`_DOCLICK`). `msize` gives per (kind,dir) hit boxes (l,t,r,b).
    pub fn tick(&mut self, msize: &[[[i32; 4]; 8]; 13]) {
        self.itime += 1;
        for m in 0..NMECH {
            if self.mechs[m].redraw > 0 {
                self.mechs[m].redraw -= 1;
                if self.mechs[m].redraw == 0 && self.mechs[m].nmissiles == 0 {
                    let side = self.mechs[m].side as usize;
                    if self.cur[side] == m {
                        self.recenter[side] = true;
                    }
                }
            }
            let tile = self.tile_at(self.mechs[m].x, self.mechs[m].y);
            if self.mechs[m].status == -1 {
                continue;
            }
            self.getcomp(m);
            let kind = self.mechs[m].kind;
            let cooldiv = stat(kind)[15].max(1) as i32;
            if self.itime % 10 == (m as i32) % 10 && self.mechs[m].status != -2 {
                let hits = self.mechs[m].hits.max(0);
                self.mechs[m].heat -= hits / cooldiv + 1;
                if self.mechs[m].in_water {
                    if self.ttype == 3 {
                        self.add_heat(m, 15);
                    } else {
                        self.mechs[m].heat -= hits / cooldiv + 1;
                    }
                }
                if self.mechs[m].heat < 0 {
                    self.mechs[m].heat = 0;
                }
                if self.mechs[m].heat < 75 && self.mechs[m].lockout == -3 {
                    self.mechs[m].lockout = 0;
                }
            }
            self.mechs[m].anim += 1;
            // bullets hitting this mech
            let sz = msize[kind.clamp(0, 12) as usize][self.mechs[m].dir.rem_euclid(8) as usize];
            let (mx, my) = (self.mechs[m].x, self.mechs[m].y);
            let (x0, y0, x1, y1) = (mx + sz[0] - 8, my + sz[1] - 8, mx + sz[2] + 8, my + sz[3] + 8);
            for bi in (0..self.bullets.len()).rev() {
                let b = self.bullets[bi];
                if !b.alive || b.age < 0 {
                    continue;
                }
                if !(x0 < b.x && b.x < x1 && y0 < b.y && b.y < y1) {
                    continue;
                }
                if b.owner == m || self.mechs[m].status == -2 {
                    continue;
                }
                if !Self::dcheck(b.x, b.y, x0 + 8, y0 + 8, x1 - 8, y1 - 8, b.dir) {
                    continue;
                }
                let dmg = wtab(b.wtype)[2] as i32;
                if (dmg as u32) > (self.mechs[m].hits as u32) {
                    self.mechs[m].status = -2;
                    self.mechs[m].anim = 0;
                    self.mechs[m].hits = 0;
                    self.events.push(SimEvent::Boom(m));
                } else {
                    self.mechs[m].hits -= dmg;
                    self.mechs[m].target = self.selectt(m);
                    self.mechs[m].avoid_cnt = 0;
                    self.add_heat(m, wtab(b.wtype)[4] as i32);
                }
                self.explode_bullet(bi);
                self.bullets[bi].x += b.vx / 2;
                self.bullets[bi].y += b.vy / 2;
            }
            self.mechs[m].reload -= 1;
            if self.mechs[m].reload < 0 {
                self.mechs[m].reload = 0;
            }
            if self.mechs[m].status < 0 || self.mechs[m].lockout == -3 {
                self.tail(m);
                continue;
            }
            // detonate guided salvo on a fresh fire press by the controlling human
            if self.mechs[m].nmissiles > 0 {
                let side = self.mechs[m].side as usize;
                if self.cur[side] == m && self.cfl[m] != -1 && self.fire[side] && !self.lfire[side] {
                    for i in 0..self.mechs[m].nmissiles.min(4) as usize {
                        let bi = self.mechs[m].missile[i];
                        if bi >= 0 && (bi as usize) < self.bullets.len() && self.bullets[bi as usize].alive {
                            let w = self.bullets[bi as usize].wtype;
                            self.bullets[bi as usize].age = -anim_len((25 + wtab(w)[8] as i32).clamp(25, 29) as usize);
                        }
                    }
                    self.mechs[m].nmissiles = 0;
                }
            }
            let joy = self.joy[m];
            if joy == -1 {
                self.mechs[m].status = 0;
            } else if self.mechs[m].nmissiles > 0 {
                let li = self.mechs[m].missile[0];
                if li >= 0 && (li as usize) < self.bullets.len() && self.bullets[li as usize].alive && self.bullets[li as usize].age > 10 {
                    let nd = ndir(self.bullets[li as usize].dir, joy);
                    let sp = wtab(7)[1] as i32;
                    let (lx, ly) = {
                        let l = &mut self.bullets[li as usize];
                        l.dir = nd;
                        l.vx = FIRESTUF[nd as usize * 2] as i32 * sp / 10;
                        l.vy = FIRESTUF[nd as usize * 2 + 1] as i32 * sp / 10;
                        (l.x, l.y)
                    };
                    let ti = TRANS_TABLES[(kind as usize * 4 + self.mechs[m].weapon.min(3)).min(51)];
                    if ti >= 0 {
                        let t = MUZZLE[ti as usize];
                        let s0 = self.mechs[m].missile_shot[0] as usize;
                        let b0 = s0 * 16 + nd as usize * 2;
                        let (ox, oy) = if b0 + 1 < t.len() { (t[b0] as i32, t[b0 + 1] as i32) } else { (0, 0) };
                        for i in 1..self.mechs[m].nmissiles.min(4) as usize {
                            let bi = self.mechs[m].missile[i];
                            if bi < 0 || bi as usize >= self.bullets.len() {
                                continue;
                            }
                            let si = self.mechs[m].missile_shot[i] as usize;
                            let bb = si * 16 + nd as usize * 2;
                            let (fx, fy) = if bb + 1 < t.len() { (t[bb] as i32, t[bb + 1] as i32) } else { (0, 0) };
                            let (vx, vy) = (self.bullets[li as usize].vx, self.bullets[li as usize].vy);
                            let f = &mut self.bullets[bi as usize];
                            f.x = lx - ox + fx;
                            f.y = ly - oy + fy;
                            f.vx = vx;
                            f.vy = vy;
                            f.dir = nd;
                        }
                    }
                }
            } else {
                let sp = stat(kind)[2] as i32;
                self.mechs[m].want_dir = joy;
                self.mechs[m].dir = ndir(self.mechs[m].dir, joy);
                self.mechs[m].status = 1;
                if self.mechs[m].want_dir == self.mechs[m].dir && sp > 0 {
                    let dx = DIR[joy as usize * 2] as i32 * sp;
                    let dy = DIR[joy as usize * 2 + 1] as i32 * sp;
                    self.mechs[m].x += dx;
                    self.mechs[m].y += dy;
                    let (nx, ny) = (self.mechs[m].x, self.mechs[m].y);
                    if self.lcheck(nx, ny, m) {
                        self.mechs[m].x -= dx;
                        self.mechs[m].y -= dy;
                        self.mechs[m].status = 0;
                    }
                    for k in 0..NMECH {
                        if self.mechs[m].status != 1 {
                            break;
                        }
                        if k != m && self.mechs[k].status != -1 && self.mechs[k].status != -4 && self.testc(m, k) {
                            self.mechs[m].x -= dx;
                            self.mechs[m].y -= dy;
                            self.mechs[m].status = 0;
                        }
                    }
                }
            }
            if self.mechs[m].status == -4 {
                self.tail(m);
                continue;
            }
            self.mechs[m].in_water = tile == 53;
            if tile == 2 && m < 20 && self.retreat[m / 10] {
                self.mechs[m].status = -4;
            }
            if self.key[m] == 1 && self.mechs[m].reload == 0 && self.mechs[m].nbullets < 16 && self.mechs[m].status >= 0 {
                self.fire_weapon(m);
                let w = weapon_of(kind, self.mechs[m].weapon.min(3));
                if w >= 0 {
                    self.mechs[m].reload = wtab(w as usize)[6] as i32;
                }
                self.mechs[m].anim = 0;
            }
            self.tail(m);
        }
        self.update_bullets();
    }

    fn tail(&mut self, m: usize) {
        if self.mechs[m].lockout == -3 && self.mechs[m].status > 0 {
            self.mechs[m].status = 0;
        }
    }

    fn update_bullets(&mut self) {
        for bi in (0..self.bullets.len()).rev() {
            if !self.bullets[bi].alive {
                continue;
            }
            let mut b = self.bullets[bi];
            if b.age >= 0 {
                let (vx, vy) = (b.vx, b.vy);
                if (b.wtype == 7 && b.age < 10) || (b.wtype == 6 && b.age < 4) {
                    b.x += vx / 8;
                    b.y += vy / 8;
                } else {
                    b.x += vx;
                    b.y += vy;
                }
                b.range -= vx.abs() + vy.abs();
            }
            let tx = b.x.div_euclid(32);
            let ty = b.y.div_euclid(32);
            let tile = self.map.get(tx, ty);
            b.age += 1;
            self.bullets[bi] = b;
            if tile == 2 || b.age == -1 || tx < 0 || ty < 0 || tx >= MAP_W as i32 || ty >= 78 {
                if b.wtype == 7 && tile == 2 {
                    self.clear_missile(bi, false);
                }
                self.delete_bullet(bi);
                continue;
            }
            if b.range < 0 && b.age >= 0 {
                self.explode_bullet(bi);
            }
            if self.bullets[bi].age <= 0 {
                continue;
            }
            if !self.is_blocking_tile(tile) {
                continue;
            }
            let idx = (ty as usize) * MAP_W + tx as usize;
            let hp = self.hitloc[idx] as i32;
            let dmg = wtab(b.wtype)[2] as i32;
            if hp <= dmg {
                for p in self.pieces.iter_mut() {
                    if p.alive && p.x / 32 == tx && p.y / 32 == ty {
                        p.alive = false;
                        self.events.push(SimEvent::Crit);
                    }
                }
                let newt = match self.ttype {
                    1 => WALL_LISTCON[tile as usize],
                    4 => WALLCITY[tile as usize],
                    _ => WALL_LISTARC[tile as usize],
                };
                self.map.tiles[idx] = newt;
                self.hitloc[idx] = WALL_POWER[newt as usize];
            } else {
                self.hitloc[idx] = (hp - dmg) as u8;
            }
            self.explode_bullet(bi);
        }
    }

    fn delete_bullet(&mut self, bi: usize) {
        let owner = self.bullets[bi].owner;
        self.bullets[bi].alive = false;
        self.mechs[owner].nbullets = (self.mechs[owner].nbullets - 1).max(0);
    }

    /// Frame-level housekeeping done in `_DRAW_ALL_MECHS`: explosion timing and end-of-battle test.
    /// Returns true when the battle is over.
    pub fn frame_check(&mut self) -> bool {
        for m in 0..NMECH {
            let side = self.mechs[m].side as usize;
            match self.mechs[m].status {
                -4 => {
                    if m < 20 && self.cur[side] == m && !self.bump_mech(side) {
                        self.over = true;
                        return true;
                    }
                }
                -2 => {
                    let limit = if self.mechs[m].kind < 6 { 16 } else { 12 };
                    if self.mechs[m].anim > limit {
                        self.mechs[m].status = -1;
                        if m < 20 && self.cur[side] == m && !self.bump_mech(side) {
                            self.over = true;
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Result as in `_UNLOADCRUD`: (result 0/1/2, attacker retreated, defender retreated, health% per slot)
    pub fn result(&self) -> (i32, bool, bool, [i32; 36]) {
        let mut a = false;
        let mut d = false;
        let mut aret = false;
        let mut dret = false;
        let mut health = [0i32; 36];
        for i in 0..NMECH {
            let m = &self.mechs[i];
            if m.status >= 0 {
                if m.side == 0 {
                    a = true;
                } else {
                    d = true;
                }
            }
            if m.status == -4 {
                if m.side == 1 {
                    dret = true;
                } else {
                    aret = true;
                }
            }
            health[i] = if !m.exists {
                0
            } else if m.status == -1 || m.status == -2 {
                if m.hits == 0 {
                    101
                } else {
                    0
                }
            } else {
                (m.hits as i64 * 100 / stat(m.kind)[1].max(1) as i64) as i32
            };
        }
        let res = if !a && !d {
            0
        } else if a {
            2
        } else {
            1
        };
        (res, aret, dret, health)
    }
}
