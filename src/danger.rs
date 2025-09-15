//! Port of killerducky's tile danger calculation.
//! Intended to help a player understand an engine's decision.
//! original: <https://github.com/killerducky/killer_mortal_gui#dealin-rate>

use riichi::{
    must_tile,
    state::{PlayerState, item::KawaItem},
    tile::Tile,
};

/// Wall danger for ryanmen waits using chance strategies.
#[derive(Copy, Clone, Debug)]
pub enum WallDangerKind {
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

impl WallDangerKind {
    pub fn to_acronym(self) -> &'static str {
        match self {
            WallDangerKind::None => "",
            WallDangerKind::DoubleNoChance => "DNC",
            WallDangerKind::NoChance => "NC",
            WallDangerKind::DoubleOneChance => "DOC",
            WallDangerKind::MixedOneChance => "MOC",
            WallDangerKind::OneChance => "OC",
        }
    }
}

/// Simple kinds of waits used for danger calculation.
#[derive(Debug, Clone, Copy)]
pub enum WaitKind {
    Ryanmen,
    Kanchan,
    Penchan,
    Tanki,
    Shanpon,
}

/// Boardstate-agnostic wait type
#[derive(Debug, Clone)]
pub struct GeneralWait {
    pub tiles: Vec<u8>,
    pub waits: Vec<u8>,
    pub kind: WaitKind,
}

/// A specific wait that a player might have and all its flags.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Wait {
    pub wait: GeneralWait,
    pub genbutsu: bool,
    pub combinations: u8,
    pub ura_suji: bool,
    pub matagi_suji_early: bool,
    pub matagi_suji_riichi: bool,
    pub riichi_suji_trap: bool,
    pub dora_involved: bool,
    pub weight: f32,
}

impl Wait {
    /// The weight specifically for this wait
    /// Doubles the weight of shanpon.
    #[allow(dead_code)]
    pub fn individual_weight(&self) -> f32 {
        if matches!(self.wait.kind, WaitKind::Shanpon) {
            self.weight * 2.0
        } else {
            self.weight
        }
    }
}

/// The danger weights for a specific player for each tile
#[derive(Clone, Debug)]
pub struct PlayerDanger {
    pub tile_weights: [f32; 34],
    pub waits: Vec<Wait>,
}

impl PlayerDanger {
    pub fn sorted_tile_weights(&self) -> Vec<(Tile, f32)> {
        let mut tile_weights = self
            .tile_weights
            .iter()
            .enumerate()
            .map(|(tile, weight)| (must_tile!(tile), *weight))
            .collect::<Vec<_>>();
        tile_weights.sort_unstable_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        tile_weights
    }

    #[allow(dead_code)]
    pub fn tile_waits(&self, tile: u8) -> Vec<Wait> {
        self.waits
            .iter()
            .filter(|wait| wait.wait.waits.contains(&tile))
            .cloned()
            .collect::<Vec<_>>()
    }
}

pub static POSSIBLE_WAITS: std::sync::LazyLock<Vec<GeneralWait>> = std::sync::LazyLock::new(|| {
    let mut waits_array: Vec<GeneralWait> = Vec::new();

    for suit in 0..3 {
        for number in 1..7 {
            waits_array.push(GeneralWait {
                tiles: vec![suit * 9 + number, suit * 9 + number + 1],
                waits: vec![suit * 9 + number - 1, suit * 9 + number + 2],
                kind: WaitKind::Ryanmen,
            });
        }
    }
    for suit in 0..3 {
        for number in 1..8 {
            waits_array.push(GeneralWait {
                tiles: vec![suit * 9 + number - 1, suit * 9 + number + 1],
                waits: vec![suit * 9 + number],
                kind: WaitKind::Kanchan,
            });
        }
    }

    for suit in 0..3 {
        waits_array.push(GeneralWait {
            tiles: vec![suit * 9, suit * 9 + 1],
            waits: vec![suit * 9 + 2],
            kind: WaitKind::Penchan,
        });
        waits_array.push(GeneralWait {
            tiles: vec![suit * 9 + 7, suit * 9 + 8],
            waits: vec![suit * 9 + 6],
            kind: WaitKind::Penchan,
        });
    }

    for suit in 0..=3 {
        for number in 0..9 {
            if suit == 3 && number > 6 {
                continue;
            }
            waits_array.push(GeneralWait {
                tiles: vec![suit * 9 + number],
                waits: vec![suit * 9 + number],
                kind: WaitKind::Shanpon,
            });
            waits_array.push(GeneralWait {
                tiles: vec![suit * 9 + number],
                waits: vec![suit * 9 + number],
                kind: WaitKind::Tanki,
            });
        }
    }

    waits_array
});

pub fn calculate_player_danger(
    safe_tiles: [bool; 34],
    discards_before_riichi: Vec<u8>,
    riichi_tile: Option<u8>,
    unseen_tiles: [u8; 34],
    doras: Vec<u8>,
) -> PlayerDanger {
    let mut waits = vec![];
    let mut tile_weights = [0.0; 34];
    for wait in POSSIBLE_WAITS.iter() {
        let genbutsu = wait.waits.iter().any(|&tile| safe_tiles[tile as usize]);
        let combinations = if matches!(wait.kind, WaitKind::Shanpon) {
            (unseen_tiles[wait.tiles[0] as usize] * unseen_tiles[wait.tiles[0] as usize].saturating_sub(1)) / 2
        } else {
            wait.tiles.iter().map(|&tile| unseen_tiles[tile as usize]).product()
        };

        let mut ura_suji = false;
        let mut matagi_suji_early = false;
        let mut matagi_suji_riichi = false;
        if matches!(wait.kind, WaitKind::Ryanmen) {
            for discarded_tile in discards_before_riichi.iter() {
                if !matches!(discarded_tile % 9, 3..6) {
                    continue;
                }
                if wait.tiles.contains(discarded_tile) {
                    continue;
                }
                for &wait_tile in wait.tiles.iter() {
                    if discarded_tile.abs_diff(wait_tile) == 2 {
                        ura_suji = true;
                        break;
                    }
                }
            }
            for discarded_tile in discards_before_riichi.iter() {
                if wait.tiles.contains(discarded_tile) {
                    matagi_suji_early = true;
                    break;
                }
            }
            if let Some(riichi_tile) = riichi_tile
                && wait.tiles.contains(&riichi_tile)
            {
                matagi_suji_riichi = true;
            }
        }
        let riichi_suji_trap = matches!(wait.kind, WaitKind::Kanchan)
            && riichi_tile.is_some_and(|riichi_tile| {
                matches!(riichi_tile % 9, 3..6) && wait.waits.iter().any(|wait_tile| riichi_tile.abs_diff(*wait_tile) == 3)
            });
        let dora_involved = wait
            .tiles
            .iter()
            .chain(wait.waits.iter())
            .any(|involved_tile| doras.contains(involved_tile));

        let weight = if genbutsu {
            0.0
        } else {
            let mut weight = combinations as f32;
            weight *= match wait.kind {
                WaitKind::Ryanmen => 3.5,
                WaitKind::Tanki | WaitKind::Shanpon if wait.tiles[0] >= 27 => 1.7,
                WaitKind::Tanki | WaitKind::Shanpon => 1.0,
                WaitKind::Kanchan if riichi_suji_trap => 2.6,
                WaitKind::Kanchan => 0.21,
                WaitKind::Penchan => 1.0,
            };
            if ura_suji {
                weight *= 1.3;
            }
            if matagi_suji_early {
                weight *= 0.6;
            }
            if matagi_suji_riichi {
                weight *= 1.2;
            }
            if dora_involved {
                weight *= 1.2;
            }
            weight
        };
        for &wait_tile in wait.waits.iter() {
            tile_weights[wait_tile as usize] += weight;
        }

        waits.push(Wait {
            wait: wait.clone(),
            genbutsu,
            combinations,
            ura_suji,
            matagi_suji_early,
            matagi_suji_riichi,
            riichi_suji_trap,
            dora_involved,
            weight,
        });
    }

    PlayerDanger { tile_weights, waits }
}

/// Calculate general wall danger based on NoChance and OneChance strategies
/// original: <https://github.com/EndlessCheng/mahjong-helper/blob/master/util/risk_base.go>
pub fn calculate_wall_danger(unseen_tiles: &[u8; 34]) -> [WallDangerKind; 34] {
    let mut result = [WallDangerKind::None; 34];

    for i in 0..3 {
        for j in 0..3 {
            let idx = 9 * i + j;
            if unseen_tiles[idx + 1] == 1 && unseen_tiles[idx + 2] == 1 {
                result[idx] = WallDangerKind::DoubleOneChance;
            } else if unseen_tiles[idx + 1] == 1 || unseen_tiles[idx + 2] == 1 {
                result[idx] = WallDangerKind::OneChance;
            }
        }
        for j in 3..6 {
            let idx = 9 * i + j;
            if (unseen_tiles[idx - 2] == 1 || unseen_tiles[idx - 1] == 1)
                && (unseen_tiles[idx + 1] == 1 || unseen_tiles[idx + 2] == 1)
            {
                if unseen_tiles[idx - 2] == 1
                    && unseen_tiles[idx - 1] == 1
                    && unseen_tiles[idx + 1] == 1
                    && unseen_tiles[idx + 2] == 1
                {
                    result[idx] = WallDangerKind::DoubleOneChance;
                } else if (unseen_tiles[idx - 2] == 1 && unseen_tiles[idx - 1] == 1)
                    || (unseen_tiles[idx + 1] == 1 && unseen_tiles[idx + 2] == 1)
                {
                    result[idx] = WallDangerKind::MixedOneChance;
                } else {
                    result[idx] = WallDangerKind::OneChance;
                }
            }
        }
        for j in 6..9 {
            let idx = 9 * i + j;
            if unseen_tiles[idx - 2] == 1 && unseen_tiles[idx - 1] == 1 {
                result[idx] = WallDangerKind::DoubleOneChance;
            } else if unseen_tiles[idx - 2] == 1 || unseen_tiles[idx - 1] == 1 {
                result[idx] = WallDangerKind::OneChance;
            }
        }
    }
    for i in 0..3 {
        for j in 0..3 {
            let idx = 9 * i + j;
            if unseen_tiles[idx + 1] == 0 || unseen_tiles[idx + 2] == 0 {
                result[idx] = WallDangerKind::NoChance;
            }
        }
        for j in 3..6 {
            let idx = 9 * i + j;
            if (unseen_tiles[idx - 2] == 0 || unseen_tiles[idx - 1] == 0)
                && (unseen_tiles[idx + 1] == 0 || unseen_tiles[idx + 2] == 0)
            {
                result[idx] = WallDangerKind::NoChance;
            }
        }
        for j in 6..9 {
            let idx = 9 * i + j;
            if unseen_tiles[idx - 2] == 0 || unseen_tiles[idx - 1] == 0 {
                result[idx] = WallDangerKind::NoChance;
            }
        }
    }
    for i in 0..3 {
        if unseen_tiles[9 * i + 1] == 0 || unseen_tiles[9 * i + 2] == 0 {
            result[9 * i] = WallDangerKind::DoubleNoChance;
        }
        if unseen_tiles[9 * i + 2] == 0 || (unseen_tiles[9 * i] == 0 && unseen_tiles[9 * i + 3] == 0) {
            result[9 * i + 1] = WallDangerKind::DoubleNoChance;
        }
        for j in 2..=6 {
            let idx = 9 * i + j;
            if (unseen_tiles[idx - 2] == 0 && unseen_tiles[idx + 1] == 0)
                || (unseen_tiles[idx - 1] == 0 && unseen_tiles[idx + 1] == 0)
                || (unseen_tiles[idx - 1] == 0 && unseen_tiles[idx + 2] == 0)
            {
                result[idx] = WallDangerKind::DoubleNoChance;
            }
        }
        if unseen_tiles[9 * i + 6] == 0 || (unseen_tiles[9 * i + 5] == 0 && unseen_tiles[9 * i + 8] == 0) {
            result[9 * i + 7] = WallDangerKind::DoubleNoChance;
        }
        if unseen_tiles[9 * i + 6] == 0 || unseen_tiles[9 * i + 7] == 0 {
            result[9 * i + 8] = WallDangerKind::DoubleNoChance;
        }
    }

    result
}

/// Determines safe tiles for the other three players asuming kawa is relative
pub fn determine_safe_tiles(kawa: &[tinyvec::TinyVec<[Option<KawaItem>; 24]>; 4]) -> [[bool; 34]; 3] {
    let mut safe_tiles = [[false; 34]; 3]; // furiten
    let mut temporary_safe_tiles = [[false; 34]; 3]; // temporary furiten, riichi furiten, or implied no wait change

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
        for (tile, &is_safe) in tiles.iter().enumerate() {
            if is_safe {
                safe_tiles[player][tile] = true;
            }
        }
    }

    safe_tiles
}

pub fn calculate_board_danger(state: &PlayerState) -> [PlayerDanger; 3] {
    let unseen_tiles = state.tiles_seen.map(|x| 4 - x);
    determine_safe_tiles(&state.kawa)
        .iter()
        .enumerate()
        .map(|(player, safe_tiles)| {
            let discards_before_riichi = state.kawa[player + 1]
                .iter()
                .filter_map(|item| item.as_ref().map(|item| item.sutehai))
                .take_while(|item| !item.is_riichi)
                .map(|x| x.tile.as_u8())
                .collect::<Vec<_>>();
            let riichi_tile = state.kawa[player + 1]
                .iter()
                .filter_map(|item| item.as_ref().map(|item| item.sutehai))
                .find(|item| item.is_riichi)
                .map(|x| x.tile.as_u8());
            calculate_player_danger(
                *safe_tiles,
                discards_before_riichi,
                riichi_tile,
                unseen_tiles,
                state.dora_indicators.iter().map(|x| x.next().as_u8()).collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
