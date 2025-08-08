//! Port of EndlessCheng's tile risk calculation.
//! Intended to help a player understand an engine's decision, rather than determining what's the safest discard.
//! original: <https://github.com/EndlessCheng/mahjong-helper/blob/master/util/risk_base.go>
use tinyvec;

/// Wall danger for ryanmen waits using chance strategies.
#[derive(Copy, Clone, Debug)]
pub enum WallDangerType {
    /// no guarantees about the danger of the tile
    None,
    /// tanki (single wait), shanpon (pair wait)
    DoubleNoChance,
    /// tanki (single wait), penchan (edge wait), kanchan (closed wait)
    NoChance,
    /// ryanmen (open wait) but all forming tiles have only 1 unseen (e.g. waiting on 5s when 333444666777s has been discarded)
    DoubleOneChance,
    /// ryanmen (open wait) but one forming side has both only 1 unseen and the other has one only 1 unseen (e.g. waiting on 5s when 333444777s has been discarded)
    MixedOneChance,
    /// ryanmen (open wait) but both forming side have one only 1 unseen (e.g. waiting on 5s when 333777s has been discarded)
    OneChance,
}

impl WallDangerType {
    pub fn to_acronym(&self) -> &'static str {
        match self {
            WallDangerType::None => "",
            WallDangerType::DoubleNoChance => "DNC",
            WallDangerType::NoChance => "NC",
            WallDangerType::DoubleOneChance => "DOC",
            WallDangerType::MixedOneChance => "MOC",
            WallDangerType::OneChance => "OC",
        }
    }
}

/// Type of danger sorted most to least danger.
/// This is not a good metric and is meant more for quickly understanding the board state.
#[derive(Copy, Clone, Debug)]
pub enum DangerType {
    NoSuji5 = 0,
    NoSuji46,
    NoSuji37,
    NoSuji28,
    NoSuji19,
    HalfSuji5,
    HalfSuji46A,
    HalfSuji46B,
    Suji37,
    Suji28,
    Suji19,
    DoubleSuji5,
    DoubleSuji46,
    YakuHaiLeft3,
    YakuHaiLeft2,
    YakuHaiLeft1,
    OtakazeLeft3,
    OtakazeLeft2,
    OtakazeLeft1,
    Safe,
}

impl DangerType {
    pub fn to_part_string(&self) -> &'static str {
        match self {
            DangerType::NoSuji46 => "",
            DangerType::NoSuji5 => "",
            DangerType::NoSuji37 => "",
            DangerType::NoSuji28 => "",
            DangerType::HalfSuji46B => "Hsuji",
            DangerType::NoSuji19 => "",
            DangerType::HalfSuji5 => "Hsuji",
            DangerType::HalfSuji46A => "Hsuji",
            DangerType::Suji37 => "suji",
            DangerType::YakuHaiLeft3 => "",
            DangerType::OtakazeLeft3 => "",
            DangerType::Suji28 => "suji",
            DangerType::DoubleSuji46 => "Dsuji",
            DangerType::DoubleSuji5 => "Dsuji",
            DangerType::YakuHaiLeft2 => "",
            DangerType::OtakazeLeft2 => "",
            DangerType::Suji19 => "suji",
            DangerType::YakuHaiLeft1 => "",
            DangerType::OtakazeLeft1 => "",
            DangerType::Safe => "safe",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Danger {
    pub tile: riichi::tile::Tile,
    pub danger_type: DangerType,
    pub wall_danger_type: WallDangerType,
    pub danger_score: f64,
}

impl Danger {
    /// The danger score of a given danger type at a specific turn.
    /// dora tiles should be multiplied by factor
    fn calculate_danger_score(&self, turn: u8, is_dora: bool) -> f64 {
        let effective_danger_type = match self.wall_danger_type {
            WallDangerType::DoubleNoChance => DangerType::Suji19,
            WallDangerType::NoChance => match self.tile.as_u8() % 9 + 1 {
                1 | 9 => DangerType::Suji19,
                2 | 8 => DangerType::Suji19,
                3 | 7 => DangerType::Suji28,
                4 | 6 => DangerType::DoubleSuji46,
                5 => DangerType::DoubleSuji5,
                _ => unreachable!(),
            },
            _ => self.danger_type,
        };
        let mut score = match effective_danger_type {
            DangerType::Safe => return 0.0,
            x => RISK_RATE[turn.clamp(0, 18) as usize][x as usize],
        };
        if is_dora {
            score *= DORA_RISK_RATE[effective_danger_type as usize];
        }

        score
    }
    pub fn to_short_string(&self) -> String {
        let meta_string = vec![
            self.danger_type.to_part_string().to_owned(),
            match self.wall_danger_type {
                WallDangerType::None => "".to_owned(),
                x => format!("{}", x.to_acronym()),
            },
        ]
        .iter()
        .filter_map(|x| if x.is_empty() { None } else { Some(x.clone()) })
        .collect::<Vec<_>>()
        .join(" ");

        format!(
            "{}[{:.1}]{}",
            self.tile,
            self.danger_score,
            if meta_string.is_empty() {
                "".to_owned()
            } else {
                format!("({})", meta_string)
            }
        )
    }
}

const SUHAI_DANGER_TYPE_TABLE: [&[DangerType]; 9] = [
    &[DangerType::NoSuji19, DangerType::Suji19],
    &[DangerType::NoSuji28, DangerType::Suji28],
    &[DangerType::NoSuji37, DangerType::Suji37],
    &[
        DangerType::NoSuji46,
        DangerType::HalfSuji46B,
        DangerType::HalfSuji46A,
        DangerType::DoubleSuji46,
    ],
    &[
        DangerType::NoSuji5,
        DangerType::HalfSuji5,
        DangerType::HalfSuji5,
        DangerType::DoubleSuji5,
    ],
    &[
        DangerType::NoSuji46,
        DangerType::HalfSuji46A,
        DangerType::HalfSuji46B,
        DangerType::DoubleSuji46,
    ],
    &[DangerType::NoSuji37, DangerType::Suji37],
    &[DangerType::NoSuji28, DangerType::Suji28],
    &[DangerType::NoSuji19, DangerType::Suji19],
];
const JIHAI_DANGER_TYPE_TABLE: [[DangerType; 4]; 2] = [
    [
        DangerType::OtakazeLeft1,
        DangerType::OtakazeLeft2,
        DangerType::OtakazeLeft3,
        DangerType::OtakazeLeft3,
    ],
    [
        DangerType::YakuHaiLeft1,
        DangerType::YakuHaiLeft2,
        DangerType::YakuHaiLeft3,
        DangerType::YakuHaiLeft3,
    ],
];

/// risk rate during specific turns of specific tile wait types
#[rustfmt::skip]
pub const RISK_RATE: [[f64; 19]; 20] = [
    [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [5.7, 5.7, 5.8, 4.7, 3.4, 2.5, 2.5, 3.1, 5.6, 3.8, 1.8, 0.8, 2.6, 2.1, 1.2, 0.5, 2.4, 1.4, 1.2],
    [6.6, 6.9, 6.3, 5.2, 4.0, 3.5, 3.5, 4.1, 5.3, 3.5, 1.9, 0.8, 2.6, 2.3, 1.2, 0.5, 2.7, 1.3, 0.4],
    [7.7, 8.0, 6.7, 5.8, 4.6, 4.3, 4.1, 4.9, 5.2, 3.6, 1.8, 1.6, 2.0, 2.4, 1.2, 0.3, 2.6, 1.2, 0.3],
    [8.5, 8.9, 7.1, 6.2, 5.1, 4.8, 4.7, 5.6, 5.2, 3.8, 1.7, 1.6, 2.0, 2.6, 1.1, 0.2, 2.6, 1.2, 0.2],
    [9.4, 9.7, 7.5, 6.7, 5.5, 5.3, 5.1, 6.0, 5.3, 3.7, 1.7, 1.7, 2.0, 2.9, 1.2, 0.2, 2.8, 1.2, 0.2],
    [10.2, 10.5, 7.9, 7.1, 5.9, 5.8, 5.6, 6.4, 5.2, 3.7, 1.7, 1.8, 2.0, 3.2, 1.3, 0.2, 2.9, 1.3, 0.2],
    [11.0, 11.3, 8.4, 7.5, 6.3, 6.3, 6.1, 6.8, 5.3, 3.7, 1.7, 2.0, 2.1, 3.6, 1.4, 0.2, 3.2, 1.4, 0.2],
    [11.9, 12.2, 8.9, 8.0, 6.8, 6.9, 6.6, 7.4, 5.3, 3.8, 1.7, 2.1, 2.2, 4.0, 1.6, 0.2, 3.5, 1.6, 0.2],
    [12.8, 13.1, 9.5, 8.6, 7.4, 7.4, 7.2, 7.9, 5.5, 3.9, 1.8, 2.2, 2.3, 4.6, 1.9, 0.3, 4.0, 1.8, 0.2],
    [13.8, 14.1, 10.1, 9.2, 8.0, 8.0, 7.8, 8.5, 5.6, 4.0, 1.9, 2.4, 2.4, 5.3, 2.2, 0.3, 4.6, 2.1, 0.3],
    [14.9, 15.1, 10.8, 9.9, 8.7, 8.7, 8.5, 9.2, 5.7, 4.2, 2.0, 2.5, 2.6, 6.0, 2.6, 0.4, 5.1, 2.5, 0.3],
    [16.0, 16.3, 11.6, 10.6, 9.4, 9.4, 9.2, 9.9, 6.0, 4.4, 2.2, 2.7, 2.7, 6.8, 3.1, 0.4, 5.1, 2.5, 0.3],
    [17.2, 17.5, 12.4, 11.4, 10.2, 10.2, 10.0, 10.6, 6.2, 4.6, 2.4, 3.0, 3.0, 7.8, 3.7, 0.5, 6.6, 3.7, 0.5],
    [18.5, 18.8, 13.3, 12.3, 11.1, 11.0, 10.9, 11.4, 6.6, 4.9, 2.7, 3.2, 3.1, 8.8, 4.4, 0.7, 7.4, 4.4, 0.6],
    [19.9, 20.1, 14.3, 13.3, 12.0, 11.9, 11.8, 12.3, 7.0, 5.3, 3.0, 3.4, 3.4, 9.9, 5.2, 0.8, 8.4, 5.3, 0.8],
    [21.3, 21.7, 15.4, 14.3, 13.1, 12.9, 12.8, 13.3, 7.4, 5.7, 3.3, 3.7, 3.6, 11.2, 6.2, 1.0, 9.4, 6.5, 0.9],
    [22.9, 23.2, 16.6, 15.4, 14.2, 14.0, 13.8, 14.4, 8.0, 6.1, 3.6, 3.9, 3.9, 12.4, 7.3, 1.3, 10.5, 7.7, 1.2],
    [24.7, 24.9, 17.9, 16.7, 15.4, 15.2, 15.0, 15.6, 8.5, 6.6, 4.0, 4.3, 4.2, 13.9, 8.5, 1.7, 11.8, 9.4, 1.6],
    [27.5, 27.8, 20.4, 19.1, 17.8, 17.5, 17.5, 17.5, 9.8, 7.4, 5.0, 5.1, 5.1, 18.1, 12.1, 2.8, 14.7, 12.6, 2.1],
];

#[rustfmt::skip]
pub const DORA_RISK_RATE: [f64; 19] = [1.565, 1.540, 1.706, 1.664, 1.747, 1.769, 1.669, 1.777, 1.948, 2.040, 3.083, 2.645, 2.531, 1.619, 2.186, 5.252, 2.095, 2.738, 6.571];

/// Calculate general wall danger based on NoChance and OneChance strategies, including guaranteed suji
fn calculate_wall_danger(tiles: &[u8; 34], safe_tiles: &[bool; 34]) -> [WallDangerType; 34] {
    let mut result = [WallDangerType::None; 34];

    for i in 0..3 {
        for j in 0..3 {
            let idx = 9 * i + j;
            if tiles[idx + 1] == 1 && tiles[idx + 2] == 1 {
                result[idx] = WallDangerType::DoubleOneChance;
            } else if tiles[idx + 1] == 1 || tiles[idx + 2] == 1 {
                result[idx] = WallDangerType::OneChance;
            }
        }
        for j in 3..6 {
            let idx = 9 * i + j;
            if (tiles[idx - 2] == 1 || tiles[idx - 1] == 1) && (tiles[idx + 1] == 1 || tiles[idx + 2] == 1) {
                if tiles[idx - 2] == 1 && tiles[idx - 1] == 1 && tiles[idx + 1] == 1 && tiles[idx + 2] == 1 {
                    result[idx] = WallDangerType::DoubleOneChance;
                } else if (tiles[idx - 2] == 1 && tiles[idx - 1] == 1) || (tiles[idx + 1] == 1 && tiles[idx + 2] == 1) {
                    result[idx] = WallDangerType::MixedOneChance;
                } else {
                    result[idx] = WallDangerType::OneChance;
                }
            }
        }
        for j in 6..9 {
            let idx = 9 * i + j;
            if tiles[idx - 2] == 1 && tiles[idx - 1] == 1 {
                result[idx] = WallDangerType::DoubleOneChance;
            } else if tiles[idx - 2] == 1 || tiles[idx - 1] == 1 {
                result[idx] = WallDangerType::OneChance;
            }
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let idx = 9 * i + j;
            if tiles[idx + 1] == 0 || tiles[idx + 2] == 0 {
                result[idx] = WallDangerType::NoChance;
            }
        }
        for j in 3..6 {
            let idx = 9 * i + j;
            if (tiles[idx - 2] == 0 || tiles[idx - 1] == 0) && (tiles[idx + 1] == 0 || tiles[idx + 2] == 0) {
                result[idx] = WallDangerType::NoChance;
            }
        }
        for j in 6..9 {
            let idx = 9 * i + j;
            if tiles[idx - 2] == 0 || tiles[idx - 1] == 0 {
                result[idx] = WallDangerType::NoChance;
            }
        }
    }
    for i in 0..3 {
        if tiles[9 * i + 1] == 0 || tiles[9 * i + 2] == 0 {
            result[9 * i] = WallDangerType::DoubleNoChance;
        }
        if tiles[9 * i + 2] == 0 || (tiles[9 * i] == 0 && tiles[9 * i + 3] == 0) {
            result[9 * i + 1] = WallDangerType::DoubleNoChance;
        }
        for j in 2..=6 {
            let idx = 9 * i + j;
            if (tiles[idx - 2] == 0 && tiles[idx + 1] == 0)
                || (tiles[idx - 1] == 0 && tiles[idx + 1] == 0)
                || (tiles[idx - 1] == 0 && tiles[idx + 2] == 0)
            {
                result[idx] = WallDangerType::DoubleNoChance;
            }
        }
        if tiles[9 * i + 6] == 0 || (tiles[9 * i + 5] == 0 && tiles[9 * i + 8] == 0) {
            result[9 * i + 7] = WallDangerType::DoubleNoChance;
        }
        if tiles[9 * i + 6] == 0 || tiles[9 * i + 7] == 0 {
            result[9 * i + 8] = WallDangerType::DoubleNoChance;
        }
    }
    for i in 0..3 {
        for j in 1..3 {
            let idx = 9 * i + j;
            if tiles[idx - 1] == 0 && safe_tiles[idx + 3] {
                result[idx] = WallDangerType::DoubleNoChance;
            }
        }
        for j in 3..6 {
            let idx = 9 * i + j;
            if (tiles[idx - 1] == 0 && safe_tiles[idx + 3]) || (tiles[idx + 1] == 0 && safe_tiles[idx - 3]) {
                result[idx] = WallDangerType::DoubleNoChance;
            }
        }
        for j in 6..8 {
            let idx = 9 * i + j;
            if tiles[idx + 1] == 0 && safe_tiles[idx - 3] {
                result[idx] = WallDangerType::DoubleNoChance;
            }
        }
    }

    result
}

/// Calculate general wall danger based on NoChance and OneChance strategies
#[allow(dead_code)]
pub fn calculate_general_wall_danger(left_tiles: &[u8; 34]) -> [WallDangerType; 34] {
    calculate_wall_danger(left_tiles, &[false; 34])
}

/// Calculate the danger for each tile based on safe and left tiles using suji and genbutsu strategies
pub fn calculate_tile_danger(
    safe_tiles: &[bool; 34],
    left_tiles: &[u8; 34],
    round_wind_tile: u8,
    player_wind_tile: u8,
    turns: u8,
    doras: Vec<u8>,
) -> Vec<Danger> {
    let wall_danger = calculate_wall_danger(left_tiles, safe_tiles);
    let mut low_risk_tiles = safe_tiles.clone();
    for (tile, &danger) in wall_danger.iter().enumerate() {
        if matches!(danger, WallDangerType::DoubleNoChance | WallDangerType::NoChance) {
            low_risk_tiles[tile] = true;
        }
    }

    let mut danger = [DangerType::Safe; 34];
    for kind in 0..3 {
        for num in 0..3 {
            let tile = 9 * kind + num;
            if num == 0 && safe_tiles[num + 3] && left_tiles[tile] == 0 {
                danger[tile] = DangerType::Safe;
            } else if num == 2 && left_tiles[tile + 2] == 0 {
                danger[tile] = DangerType::Suji37;
            } else {
                danger[tile] = SUHAI_DANGER_TYPE_TABLE[num as usize][low_risk_tiles[tile + 3] as usize];
            }
        }
        for num in 3..6 {
            let tile = 9 * kind + num;
            danger[tile] = SUHAI_DANGER_TYPE_TABLE[num as usize]
                [((low_risk_tiles[tile - 3] as usize) << 1) | (low_risk_tiles[tile + 3] as usize)];
        }
        for num in 6..9 {
            let tile = 9 * kind + num;
            if num == 8 && safe_tiles[tile - 3] && left_tiles[tile] == 0 {
                danger[tile] = DangerType::Safe;
            } else if num == 2 && left_tiles[tile + 2] == 0 {
                danger[tile] = DangerType::Suji37;
            } else {
                danger[tile] = SUHAI_DANGER_TYPE_TABLE[num as usize][low_risk_tiles[tile + 3] as usize];
            }
        }
    }
    for tile in 27..34 {
        if left_tiles[tile] == 0 {
            danger[tile] = DangerType::Safe;
        } else {
            let yakuhai = tile as u8 == round_wind_tile || tile as u8 == player_wind_tile;
            danger[tile] = JIHAI_DANGER_TYPE_TABLE[yakuhai as usize][(left_tiles[tile] - 1) as usize]
        }
    }

    for (tile, &wall_danger) in wall_danger.iter().take(27).enumerate() {
        if matches!(wall_danger, WallDangerType::DoubleNoChance) && left_tiles[tile] == 0 {
            danger[tile] = DangerType::Safe;
        }
    }

    for (tile, &safe) in safe_tiles.iter().enumerate() {
        if safe {
            danger[tile] = DangerType::Safe;
        }
    }

    let mut danger = danger
        .iter()
        .cloned()
        .zip(wall_danger)
        .enumerate()
        .map(|(tile, (danger_type, wall_danger_type))| {
            let mut d = Danger {
                tile: riichi::must_tile!(tile),
                danger_type,
                wall_danger_type,
                danger_score: 0.0,
            };
            d.danger_score = d.calculate_danger_score(turns, doras.iter().any(|x| *x == tile as u8));
            d
        })
        .collect::<Vec<_>>();
    danger.sort_by(|a, b| b.danger_score.partial_cmp(&a.danger_score).unwrap());
    danger
}

/// Determines safe tiles for the other three players asuming kawa is relative
fn determine_safe_tiles(kawa: &[tinyvec::TinyVec<[Option<riichi::state::item::KawaItem>; 24]>; 4]) -> [[bool; 34]; 3] {
    let mut safe_tiles = [[false; 34]; 3]; // furiten
    let mut temporary_safe_tiles = [[false; 34]; 3]; // temporary furiten, riichi furiten, and implied no wait change

    for turn in 0..(kawa.iter().map(|x| x.len()).max().unwrap_or_default()) {
        for kawa_actor in 0..=3 {
            if let Some(item) = kawa[kawa_actor].get(turn).cloned().flatten() {
                let tile = item.sutehai.tile.deaka();
                for player in 0..=2 {
                    temporary_safe_tiles[player][tile.as_usize()] = true;
                }
                if kawa_actor != 0 {
                    safe_tiles[kawa_actor - 1][tile.as_usize()] = true;
                    if item.sutehai.is_tedashi {
                        temporary_safe_tiles[kawa_actor - 1] = [false; 34];
                    }
                }
            }
        }
    }

    for (player, tiles) in temporary_safe_tiles.iter().enumerate() {
        for (tile, &safe) in tiles.iter().enumerate() {
            if safe {
                safe_tiles[player][tile] = true;
            }
        }
    }

    return safe_tiles;
}

/// Calculate the danger type for each tile for each player
pub fn calculate_board_tile_danger(state: &riichi::state::PlayerState) -> [Vec<Danger>; 3] {
    let left_tiles = state.tiles_seen.map(|x| 4 - x);
    determine_safe_tiles(&state.kawa)
        .iter()
        .enumerate()
        .map(|(player, safe_tiles)| {
            calculate_tile_danger(
                safe_tiles,
                &left_tiles,
                state.bakaze.as_u8(),
                27 + (4 + player as u8 - state.oya) % 4,
                state.at_turn,
                state.dora_indicators.iter().map(|x| x.next().as_u8()).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

#[cfg(test)]
mod test {
    use super::*;

    fn visible_string_to_left_tiles(visible_tiles: &str) -> [u8; 34] {
        riichi::hand::hand(visible_tiles).unwrap().map(|x| 4 - x)
    }

    fn check_wall_tiles(visible_tiles: &str, expected: &str) {
        let tiles = calculate_general_wall_danger(&visible_string_to_left_tiles(visible_tiles));
        let result = riichi::hand::tiles_to_string(&tiles.map(|x| !matches!(x, WallDangerType::None) as u8), [false; 3]);
        assert_eq!(
            expected, result,
            "expected {} to have {} be OneChance but got {} instead",
            visible_tiles, expected, result
        )
    }

    #[test]
    fn test_calc_wall_tiles() {
        check_wall_tiles("2222777888s", "189s");
        check_wall_tiles("33337777s", "12589s");
        check_wall_tiles("333777s", "12589s");
        check_wall_tiles("333444777s", "1235689s");
        check_wall_tiles("8888s", "9s");
    }

    fn check_dnc_safe_tiles(visible_tiles: &str, expected: &str) {
        let tiles = calculate_general_wall_danger(&visible_string_to_left_tiles(visible_tiles));
        let result = riichi::hand::tiles_to_string(&tiles.map(|x| matches!(x, WallDangerType::DoubleNoChance) as u8), [false; 3]);
        assert_eq!(
            expected, result,
            "expected {} to have {} be DoubleNoChance but got {} instead",
            visible_tiles, expected, result
        )
    }

    #[test]
    fn test_dnc_safe_tiles() {
        check_dnc_safe_tiles("8888s", "9s");
        check_dnc_safe_tiles("33336666s", "1245s");
        check_dnc_safe_tiles("33335555s", "124s");
        check_dnc_safe_tiles("33337777s", "1289s");
        check_dnc_safe_tiles("333355557777s", "124689s");
    }

    fn check_safe_tiles(visible_tiles: &str, safe_tiles_string: &str, expected_safe_tiles: &str) {
        let safe_tiles = riichi::hand::hand(safe_tiles_string).unwrap().map(|x| x > 0);
        let left_tiles = visible_string_to_left_tiles(visible_tiles);
        let expected_safe_tiles = riichi::hand::hand(expected_safe_tiles).unwrap().map(|x| x > 0);

        let risk = calculate_tile_danger(&safe_tiles, &left_tiles, 27, 28, 9, vec![]);
        for (tile, (&should_be_safe, calculated_risk)) in expected_safe_tiles.iter().zip(risk).enumerate() {
            if should_be_safe {
                assert!(
                    matches!(calculated_risk.danger_type, DangerType::Safe),
                    "for visible {} and safe {}, {} expected safe but got {:?}",
                    visible_tiles,
                    safe_tiles_string,
                    riichi::must_tile!(tile),
                    calculated_risk
                );
            } else {
                assert!(
                    !matches!(calculated_risk.danger_type, DangerType::Safe),
                    "for visible {} and safe {}, {} expected unsafe",
                    visible_tiles,
                    safe_tiles_string,
                    riichi::must_tile!(tile)
                );
            }
        }
    }

    #[test]
    fn test_calculate_risk_tiles_safe_tile() {
        check_safe_tiles("2222333377779999m 22228888p 333355557777s 4444z", "", "29m 4z");
        check_safe_tiles(
            "111177778888m 1111222288889999p 222233339999s",
            "4m 5p 6s",
            "1478m 12589p 2369s",
        );
    }
}
