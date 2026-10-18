//! Static game data recovered from the original executables and data files.

/// Cyborg types in the original order (index 0..=8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Cyborg {
    Titan = 0,
    Behemoth = 1,
    Hercules = 2,
    Achilles = 3,
    Crossbolt = 4,
    Mars = 5,
    Dragon = 6,
    Cyclops = 7,
    Mercury = 8,
}

pub const NUM_CYBORGS: usize = 9;
/// Slot value meaning "empty" in the original data structures.
pub const NO_CYBORG: u8 = 10;

pub const CYBORG_NAMES: [&str; 9] = ["TITAN", "BEHEMOTH", "HERCULES", "ACHILLES", "CROSSBOLT", "MARS", "DRAGON", "CYCLOPS", "MERCURY"];
pub const CYBORG_FULL_NAMES: [&str; 9] =
    ["TITAN MK IV", "BEHEMOTH", "HERCULES MK I", "ACHILLES HL", "CROSSBOLT", "MARS TYPE IV", "DRAGON", "CYCLOPS MK X", "MERCURY MK XI"];
pub const CYBORG_GIFS: [&str; 9] =
    ["titan.gif", "behemoth.gif", "hercules.gif", "achilles.gif", "crosbolt.gif", "mars.gif", "dragon.gif", "cyclops.gif", "mercury.gif"];

/// Purchase cost with no factory enhancements (CYBOR _UNITCOST).
pub const UNIT_COST: [i32; 9] = [100_000, 90_000, 40_000, 30_000, 70_000, 15_000, 10_000, 5_000, 2_500];
/// Build time in turns with no enhancements (CYBOR _BUILDTIME) and the minimum with enhancements (_MFBTMIN).
pub const BUILD_TIME: [i32; 9] = [5, 4, 3, 3, 4, 2, 1, 1, 1];
pub const BUILD_TIME_MIN: [i32; 9] = [3, 2, 1, 1, 2, 1, 1, 1, 1];
/// Armour / tonnage (CYBOR _HL and the ENG.DAT spec sheets).
pub const ARMOR: [i32; 9] = [750, 600, 390, 270, 150, 150, 90, 90, 30];
/// Speed class: true = FAST.
pub const FAST: [bool; 9] = [false, false, false, true, false, true, true, true, true];
/// Cooling factor as shown on the spec sheets (ENG.DAT).
pub const COOLING: [i32; 9] = [10, 8, 6, 4, 3, 3, 3, 3, 3];

/// Weapon systems (ENG.DAT order, used by _CYBWEAP/_WSTS).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Weapon {
    HeavyLaser = 0,
    LightLaser = 1,
    HeavyAutoCannon = 2,
    LightAutoCannon = 3,
    NeutrinoCannon = 4,
    HellFire = 5,
    ShortRangeMissile = 6,
    LongRangeMissile = 7,
}

pub const WEAPON_NAMES: [&str; 8] = [
    "HEAVY LASER", "LIGHT LASER", "HEAVY AUTO CANNON", "LIGHT AUTO CANNON", "NEUTRINO CANNON", "HELL FIRE",
    "SHORT RANGE MISSILE", "LONG RANGE MISSILE",
];
pub const WEAPON_ABBR: [&str; 8] = ["HL", "LL", "HAC", "LAC", "NC", "HFL", "SRM", "LRM"];

/// Spec-sheet weapon stats (CYBOR _WSTS): range (m), damage, impact heat, firing heat.
pub const WEAPON_SPECS: [[i32; 4]; 8] =
    [[700, 3, 4, 1], [250, 1, 1, 1], [350, 10, 0, 0], [350, 4, 0, 0], [700, 20, 10, 4], [80, 0, 12, 1], [400, 16, 2, 0], [3200, 10, 2, 2]];

/// Weapon loadout per cyborg (CYBOR _CYBWEAP): up to 4 systems of (weapon, count, ammo; -1 = unlimited).
pub const LOADOUT: [[(i8, i8, i16); 4]; 9] = [
    [(4, 2, -1), (2, 2, 200), (7, 2, 20), (-1, 0, 0)],   // Titan: 2 NC, 2 HAC (200), 2 LRM (20)
    [(0, 4, -1), (3, 2, 200), (6, 2, 80), (7, 2, 20)],   // Behemoth: 4 HL, 2 LAC (200), 2 SRM (80), 2 LRM (20)
    [(4, 2, -1), (0, 2, -1), (6, 2, 50), (-1, 0, 0)],    // Hercules: 2 NC, 2 HL, 2 SRM (50)
    [(0, 4, -1), (6, 2, 30), (-1, 0, 0), (-1, 0, 0)],    // Achilles: 4 HL, 2 SRM (30)
    [(1, 2, -1), (7, 4, 80), (-1, 0, 0), (-1, 0, 0)],    // Crossbolt: 2 LL, 4 LRM (80)
    [(4, 1, -1), (3, 2, 150), (-1, 0, 0), (-1, 0, 0)],   // Mars: 1 NC, 2 LAC (150)
    [(1, 2, -1), (5, 1, 25), (-1, 0, 0), (-1, 0, 0)],    // Dragon: 2 LL, 1 HFL (25)
    [(1, 2, -1), (3, 1, 25), (-1, 0, 0), (-1, 0, 0)],    // Cyclops: 2 LL, 1 LAC (25)
    [(1, 3, -1), (-1, 0, 0), (-1, 0, 0), (-1, 0, 0)],    // Mercury: 3 LL
];

/// Terrains where each cyborg is "most effective" (spec sheets). Terrain ids 1..=5.
pub const EFFECTIVE_TERRAIN: [&[u8]; 9] = [
    &[4, 3, 1, 2],    // Titan: volcanic, desert, arctic, coniferous
    &[4, 3, 1],       // Behemoth
    &[1, 2],          // Hercules
    &[5, 2, 1],       // Achilles
    &[4, 3, 1],       // Crossbolt
    &[5, 2, 1],       // Mars
    &[5, 3, 4],       // Dragon
    &[5],             // Cyclops
    &[5],             // Mercury
];

pub const TERRAIN_NAMES: [&str; 6] = ["", "ARCTIC", "CONIFEROUS", "DESERT", "VOLCANIC", "CITY"];

/// Fortifications: level 1..=3.
pub const FORT_COST: [i32; 4] = [0, 100_000, 200_000, 300_000];
pub const FORT_NAMES: [&str; 4] = ["NO FORTIFICATION", "LIGHT FORT.", "MEDIUM FORT.", "HEAVY FORT."];
pub const FORT_LEVEL_NAMES: [&str; 4] = ["NONE", "LIGHT", "MEDIUM", "HEAVY"];
pub const FORT_WEAPONS: [&str; 4] = ["", "WEAPONS: 4 LAC - 4 LRM", "WEAPONS: 4 HAC - 6 LRM", "WEAPONS: 4 NC  - 8 LRM"];
/// Average wall hit points per level (manual).
pub const FORT_WALL_HP: [i32; 4] = [0, 100, 300, 500];

/// Factories: kind 0..=2 (uni/dual/quadro post).
pub const FACTORY_COST: [i32; 3] = [500_000, 800_000, 1_200_000];
pub const FACTORY_BAYS: [i32; 3] = [1, 2, 4];
pub const FACTORY_NAMES: [&str; 3] = ["UNI-POST FACTORY", "DUAL-POST FACTORY", "QUADRO-POST FACTORY"];
pub const ENHANCEMENT_COST: i32 = 400_000;
pub const CAPITAL_COST: i32 = 30_000;
pub const MAX_CYBORGS_PER_COUNTRY: usize = 10;
pub const MAX_BAYS_PER_COUNTRY: usize = 24;
pub const MIN_SPY_COST: i32 = 2_000;

/// Repair cost fraction of base price by status decile 0-9, 10-19, ... 90-99 (CYBOR _REPFACTOR).
pub const REPAIR_FACTOR: [f32; 10] = [0.60, 0.55, 0.50, 0.45, 0.40, 0.35, 0.30, 0.20, 0.10, 0.05];

/// Default empire names, the generals' names and the empire symbol order.
pub const EMPIRE_NAMES: [&str; 5] = ["WAR HAMMER", "DESTROYER", "SUBJUGATOR", "DEVOURER", "DEATHDEALER"];
pub const GENERAL_NAMES: [&str; 5] = ["XITHOR", "RAK HOKEN", "TOA ZEN", "MYRAK", "RATHIOR"];
pub const SEASON_NAMES: [&str; 7] = ["TRIGLAV", "SAVITAR", "ADITYA", "VISHNU", "RUDRA", "THOTH", "CHOLE"];

/// Planet type factor used for country values (1 poor, 2 average, 3 rich).
pub const PLANET_NAMES: [&str; 3] = ["POOR", "AVERAGE", "RICH"];

pub fn cyborg_cost(t: usize, enhancements: i32) -> i32 {
    let e = enhancements.clamp(0, 5);
    UNIT_COST[t] * (10 - e) / 10
}

pub fn cyborg_build_time(t: usize, enhancements: i32) -> i32 {
    (BUILD_TIME[t] - enhancements).max(BUILD_TIME_MIN[t]).max(1)
}
