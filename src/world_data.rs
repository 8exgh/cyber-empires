//! Static world data for the planet Orion: 72 countries (generated from the original data files).
//! Terrain: 1 arctic, 2 coniferous, 3 desert, 4 volcanic, 5 city. Coordinates are on the 640x400 world map.

pub struct CountryDef {
    pub name: &'static str,
    pub terrain: u8,
    pub x: i16,
    pub y: i16,
    pub adj: &'static [u8],
}

pub const NUM_COUNTRIES: usize = 72;

/// Index 0 is unused so that country numbers 1..=72 index directly.
pub static COUNTRIES: [CountryDef; 73] = [
    CountryDef { name: "", terrain: 0, x: 0, y: 0, adj: &[] },
    CountryDef { name: "KARAG", terrain: 1, x: 98, y: 35, adj: &[2, 6] },
    CountryDef { name: "BABBAR", terrain: 5, x: 134, y: 70, adj: &[1, 4, 6, 7, 8] },
    CountryDef { name: "MITRON", terrain: 1, x: 217, y: 27, adj: &[4] },
    CountryDef { name: "IXIL", terrain: 1, x: 199, y: 67, adj: &[2, 3, 5, 8, 9] },
    CountryDef { name: "BOK TORUM", terrain: 1, x: 276, y: 46, adj: &[4, 9, 60] },
    CountryDef { name: "OTO", terrain: 1, x: 60, y: 68, adj: &[1, 2, 7] },
    CountryDef { name: "ZION", terrain: 2, x: 98, y: 108, adj: &[2, 6, 8, 10, 11] },
    CountryDef { name: "TRITON", terrain: 2, x: 175, y: 111, adj: &[2, 4, 7, 9, 11, 12] },
    CountryDef { name: "PAR THANX", terrain: 5, x: 260, y: 85, adj: &[4, 5, 8, 12, 60, 61] },
    CountryDef { name: "DYTH ROG", terrain: 5, x: 81, y: 145, adj: &[7, 11, 62, 63] },
    CountryDef { name: "YTHON MORG", terrain: 2, x: 163, y: 151, adj: &[7, 8, 10, 12, 63, 65] },
    CountryDef { name: "BEZANT ARG", terrain: 2, x: 224, y: 130, adj: &[8, 9, 11, 61, 65, 72] },
    CountryDef { name: "MAAT ARG TOR", terrain: 1, x: 351, y: 49, adj: &[14, 15, 16, 60] },
    CountryDef { name: "ORION", terrain: 5, x: 419, y: 60, adj: &[13, 16, 17] },
    CountryDef { name: "HERMES", terrain: 2, x: 352, y: 104, adj: &[13, 16, 21, 60, 61] },
    CountryDef { name: "SARTH", terrain: 1, x: 390, y: 94, adj: &[13, 14, 15, 17, 21, 22] },
    CountryDef { name: "QUO TOR", terrain: 1, x: 449, y: 96, adj: &[14, 16, 18, 22, 23] },
    CountryDef { name: "ZORGOTH BORON", terrain: 1, x: 514, y: 64, adj: &[17, 19, 20, 23, 24] },
    CountryDef { name: "TORUM SOR", terrain: 1, x: 588, y: 50, adj: &[18, 20] },
    CountryDef { name: "GARTH MARDANT", terrain: 5, x: 562, y: 83, adj: &[18, 19, 24, 25] },
    CountryDef { name: "PARTH MYRAX", terrain: 2, x: 366, y: 133, adj: &[15, 16, 22, 26, 61, 72] },
    CountryDef { name: "LARGUS", terrain: 2, x: 419, y: 129, adj: &[16, 17, 21, 23, 26, 27] },
    CountryDef { name: "CART BENAKON", terrain: 2, x: 469, y: 120, adj: &[17, 18, 22, 24, 27] },
    CountryDef { name: "ET AMMONENNON", terrain: 2, x: 513, y: 118, adj: &[18, 20, 23, 25, 27, 28, 29] },
    CountryDef { name: "TOR ASGARD", terrain: 2, x: 565, y: 126, adj: &[20, 24, 29] },
    CountryDef { name: "WYTHE", terrain: 3, x: 397, y: 170, adj: &[21, 22, 27, 68, 72] },
    CountryDef { name: "QETH MOR KRON", terrain: 5, x: 457, y: 162, adj: &[22, 23, 24, 26, 28, 68, 69, 70] },
    CountryDef { name: "RYKE", terrain: 3, x: 506, y: 178, adj: &[24, 27, 29, 70] },
    CountryDef { name: "HERITITE", terrain: 3, x: 552, y: 169, adj: &[24, 25, 28, 70, 71] },
    CountryDef { name: "IRUS AK MYRAK", terrain: 2, x: 36, y: 239, adj: &[31, 38] },
    CountryDef { name: "PROTHY", terrain: 2, x: 90, y: 255, adj: &[30, 32, 33, 39] },
    CountryDef { name: "TOR ARGUS", terrain: 5, x: 114, y: 228, adj: &[31, 33, 62, 64] },
    CountryDef { name: "NEON ROK TOR", terrain: 2, x: 136, y: 265, adj: &[31, 32, 34, 36, 64] },
    CountryDef { name: "ANNON", terrain: 4, x: 204, y: 254, adj: &[33, 35, 36, 37, 64] },
    CountryDef { name: "LEME SETH ZOR", terrain: 5, x: 259, y: 233, adj: &[34, 37, 64, 65, 66, 67, 72] },
    CountryDef { name: "KARB", terrain: 2, x: 168, y: 297, adj: &[33, 34, 37, 40] },
    CountryDef { name: "YUR", terrain: 2, x: 223, y: 310, adj: &[34, 35, 36, 67] },
    CountryDef { name: "TRATON", terrain: 2, x: 29, y: 306, adj: &[30, 39, 41] },
    CountryDef { name: "MARG YTHOR", terrain: 5, x: 59, y: 329, adj: &[31, 38, 40, 42] },
    CountryDef { name: "AMATIOM", terrain: 1, x: 104, y: 344, adj: &[36, 39, 42] },
    CountryDef { name: "RAK SOMME ARG", terrain: 1, x: 15, y: 373, adj: &[38, 42] },
    CountryDef { name: "DARTH ORION", terrain: 1, x: 77, y: 366, adj: &[39, 40, 41] },
    CountryDef { name: "TOR ARION", terrain: 5, x: 363, y: 237, adj: &[44, 49, 66, 68] },
    CountryDef { name: "KARAK", terrain: 4, x: 428, y: 251, adj: &[43, 45, 50, 68] },
    CountryDef { name: "BOREAS", terrain: 5, x: 469, y: 233, adj: &[44, 46, 52, 68, 69, 70] },
    CountryDef { name: "AMRAK", terrain: 3, x: 515, y: 235, adj: &[45, 47, 53, 54, 70, 71] },
    CountryDef { name: "IZEN BORK TOR", terrain: 4, x: 564, y: 232, adj: &[46, 48, 54] },
    CountryDef { name: "SERAPIS", terrain: 4, x: 606, y: 247, adj: &[47, 54] },
    CountryDef { name: "SERGOTH", terrain: 2, x: 373, y: 289, adj: &[43, 50, 51, 66, 67] },
    CountryDef { name: "PHEON AR BOR", terrain: 2, x: 424, y: 290, adj: &[44, 49, 51, 52] },
    CountryDef { name: "MORGANA", terrain: 5, x: 407, y: 319, adj: &[49, 50] },
    CountryDef { name: "ETIUM BAKUS", terrain: 2, x: 464, y: 301, adj: &[45, 50, 53, 55, 56, 57] },
    CountryDef { name: "SAR ET OTOM", terrain: 4, x: 506, y: 275, adj: &[46, 52, 54, 55] },
    CountryDef { name: "CARAH FOKOT", terrain: 4, x: 552, y: 261, adj: &[46, 47, 48, 53, 55, 58] },
    CountryDef { name: "GATH", terrain: 2, x: 520, y: 297, adj: &[52, 53, 54, 57, 58] },
    CountryDef { name: "SOR DEVETON", terrain: 1, x: 460, y: 343, adj: &[52, 57, 59] },
    CountryDef { name: "FRAG BORKOR", terrain: 1, x: 506, y: 326, adj: &[52, 55, 56, 58] },
    CountryDef { name: "DEONIUM", terrain: 1, x: 554, y: 321, adj: &[54, 55, 57, 59] },
    CountryDef { name: "ARGON", terrain: 1, x: 514, y: 373, adj: &[56, 58] },
    CountryDef { name: "BORTO MARAK", terrain: 1, x: 314, y: 79, adj: &[5, 9, 13, 15, 61] },
    CountryDef { name: "XION", terrain: 2, x: 300, y: 126, adj: &[9, 12, 15, 21, 60, 72] },
    CountryDef { name: "ZOR", terrain: 4, x: 78, y: 194, adj: &[10, 32, 63, 64] },
    CountryDef { name: "BORK TOR", terrain: 4, x: 164, y: 186, adj: &[10, 11, 62, 64, 65] },
    CountryDef { name: "MAR TOR ARGUS", terrain: 4, x: 190, y: 224, adj: &[32, 33, 34, 35, 62, 63, 65] },
    CountryDef { name: "BARTH", terrain: 4, x: 240, y: 182, adj: &[11, 12, 35, 63, 64, 72] },
    CountryDef { name: "SMYTHE", terrain: 3, x: 313, y: 251, adj: &[35, 43, 49, 67, 68, 72] },
    CountryDef { name: "XENON", terrain: 2, x: 314, y: 299, adj: &[35, 37, 49, 66] },
    CountryDef { name: "ZARITH", terrain: 3, x: 398, y: 202, adj: &[26, 27, 43, 44, 45, 66, 69, 72] },
    CountryDef { name: "NEBU TOR ETUM", terrain: 3, x: 458, y: 201, adj: &[27, 45, 68, 70] },
    CountryDef { name: "TOK BERETUM", terrain: 3, x: 507, y: 201, adj: &[27, 28, 29, 45, 46, 69, 71] },
    CountryDef { name: "MAR TORET", terrain: 3, x: 555, y: 200, adj: &[29, 46, 70] },
    CountryDef { name: "TITANIUM", terrain: 5, x: 311, y: 193, adj: &[12, 21, 26, 35, 61, 65, 66, 68] },
];

/// Countries that are cities (from CYBOR _CITYVALUE).
pub const CITY_COUNTRIES: [u8; 13] = [2, 9, 10, 14, 20, 27, 32, 35, 39, 43, 45, 51, 72];

/// Candidate starting countries per empire slot (from CYBOR _STARTINGLOCATION): 5 rows x 3 choices.
pub const STARTING_LOCATIONS: [[u8; 3]; 5] = [[1, 4, 7], [41, 30, 36], [47, 50, 55], [18, 22, 28], [65, 61, 66]];

/// Initial map scroll position per starting country (from CYBOR _SCTRY/_SMAPX/_SMAPY).
pub const START_SCROLL: [(u8, i16, i16); 15] = [(1,0,0),(4,112,0),(7,0,100),(18,352,16),(22,304,100),(28,336,70),(41,0,273),(30,0,200),(36,96,274),(47,352,180),(50,304,274),(55,336,274),(65,160,100),(61,190,65),(66,96,200)];
