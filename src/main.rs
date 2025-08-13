mod danger;
mod mortalcompat;
use mortalcompat::{ActionType, calculate_agari, single_player_tables_after_calls};
use riichi::{must_tile, tu8};
use std::io::BufRead;

use crate::mortalcompat::CandidateExt;

#[allow(dead_code)]
fn read_json_log<P: AsRef<std::path::Path>>(path: P) -> std::io::Result<Vec<riichi::mjai::Event>> {
    Ok(std::io::BufReader::new(std::fs::File::open(path)?)
        .lines()
        .filter_map(|line| line.ok().and_then(|l| serde_json::from_str::<riichi::mjai::Event>(&l).ok()))
        .collect())
}

fn read_ekyumoe_log(path: &str) -> Vec<riichi::mjai::Event> {
    let v: serde_json::Value = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path).unwrap())).unwrap();
    v.get("mjai_log")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|ev| serde_json::from_value(ev.clone()).unwrap())
        .collect()
}

/// State of the board that is not immediately evident such as shanten, expected score or tile danger
pub struct ExpandedState {
    /// Mortal's player state
    state: riichi::state::PlayerState,
    /// Shanten of the current hand. -1 for agari hands.
    shanten: i8,
    /// Tiles that the player can discard assuming they will perform a specific action, and their expected values assuming tsumo-only.
    /// When the player cannot dahai, it will only be a single candidate "?" with a list of tiles that are being waited on.
    /// Each candidate contains the chance the player will reach agari/tenpai in `length - n` tsumos.
    /// Expected value is equal to average score * win probability where average score assumes riichi tsumo ippatsu if possible.
    /// Candidates are sorted by expected value.
    /// Shanten down candidates are not processed for hands with 3+ shanten.
    candidates: Vec<(ActionType, Vec<riichi::algo::sp::Candidate>)>,
    /// Agari score (including specific han and fu) of individual waits.
    /// For tenpai hands assumes the score is calculated as ron with no ura-dora.
    /// For agari hands (implied tsumo) the tile will be "?" and the score will be calculated as tsumo with no ura-dora.
    agari: Vec<(riichi::tile::Tile, Option<riichi::algo::agari::Agari>)>,
    /// Danger weights and wait types for each tile based on a player's discard.
    /// Estimates danger by calculating the amount of tile combinations that can lead to a player having this wait.
    /// Uses multipliers for more common types of waits. Does not analyze tedashi patterns.
    danger: [danger::PlayerDanger; 3],
    /// Wall danger kind for each tile based on Chance rules. Player danger implicitly already calculates Chance.
    wall_danger: [danger::WallDangerKind; 34],
}

impl ExpandedState {
    pub fn from_state(state: riichi::state::PlayerState) -> Self {
        let shanten = state.real_time_shanten();

        // TODO: proper agari after Hora event
        Self {
            shanten,
            candidates: single_player_tables_after_calls(&state),
            agari: if shanten == -1 {
                vec![(
                    riichi::must_tile!(riichi::tu8!(?)),
                    calculate_agari(&state, state.last_self_tsumo.unwrap_or_default(), false),
                )]
            } else if !state.last_cans.can_discard {
                state
                    .waits
                    .iter()
                    .enumerate()
                    .filter(|&(_, &b)| b)
                    .map(|(i, _)| riichi::must_tile!(i))
                    .map(|tile| (tile, calculate_agari(&state, tile, true)))
                    .collect()
            } else {
                vec![]
            },
            danger: danger::calculate_board_danger(&state),
            wall_danger: danger::calculate_wall_danger(&state.tiles_seen.map(|x| 4 - x)),
            state,
        }
    }

    fn to_log_string(&self) -> String {
        let extra_points_string = if self.state.honba > 0 || self.state.kyotaku > 0 {
            format!("+{}", self.state.honba as i32 * 300 + self.state.kyotaku as i32 * 1000)
        } else {
            "".to_owned()
        };
        let agari_string = self
            .agari
            .iter()
            .map(|(tile, agari)| {
                format!(
                    "{} - {}",
                    tile,
                    match agari {
                        None => "yakunashi = 0".to_owned(),
                        Some(a @ riichi::algo::agari::Agari::Normal { fu, han }) => format!(
                            "{}han{}fu = {}{extra_points_string}",
                            han,
                            fu,
                            if *tile == must_tile!(tu8!(?)) {
                                a.point(self.state.is_oya()).tsumo_total(self.state.is_oya())
                            } else {
                                a.point(self.state.is_oya()).ron
                            }
                        ),
                        Some(a @ riichi::algo::agari::Agari::Yakuman(count)) => format!(
                            "{}yakuman = {}{extra_points_string}",
                            if *count == 1 { "".to_owned() } else { format!("{count}x ") },
                            a.point(self.state.is_oya()).tsumo_total(self.state.is_oya())
                        ),
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let candidates_string = self
            .candidates
            .iter()
            .map(|(call, candidate)| {
                format!(
                    "{}:\n{}",
                    call.to_string(),
                    candidate
                        .iter()
                        .map(|candidate| candidate.to_candidate_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let danger_string = self
            .danger
            .clone()
            .map(|danger| {
                danger
                    .sorted_tile_weights()
                    .iter()
                    .filter(|(_, danger)| *danger > 0.0)
                    .map(|(tile, danger)| {
                        format!(
                            "{}({:.1}{})",
                            tile,
                            danger,
                            if !matches!(self.wall_danger[tile.as_usize()], danger::WallDangerKind::None) {
                                self.wall_danger[tile.as_usize()].to_acronym()
                            } else {
                                ""
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .join("\n");
        format!(
            "{} ({}{}){}{}\n{}",
            riichi::hand::tiles_to_string(&self.state.tehai, self.state.akas_in_hand),
            self.shanten,
            if self.state.at_furiten { " - furiten" } else { "" },
            if !agari_string.is_empty() {
                format!("\nwaits: {agari_string}")
            } else {
                "".to_string()
            },
            if !candidates_string.is_empty() {
                format!("\n{candidates_string}")
            } else {
                "".to_string()
            },
            danger_string
        )
    }
}

pub fn main() {
    let player_id = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u8>().ok())
        .expect("Missing player_id");
    if let Some(path) = std::env::args().nth(2) {
        let mut state = riichi::state::PlayerState::new(player_id);
        for event in read_ekyumoe_log(&path) {
            state.update(&event).unwrap();
            println!("\n{event:?}");
            if let riichi::mjai::Event::Tsumo { actor, .. } = event
                && actor != state.player_id
            {
                continue;
            }
            println!("{}", ExpandedState::from_state(state.clone()).to_log_string());
        }
    } else {
        let mut state = riichi::state::PlayerState::new(player_id);
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(l) = line else {
                eprintln!("failed to read line");
                continue;
            };
            let Ok(event) = serde_json::from_str::<riichi::mjai::Event>(&l) else {
                eprintln!("failed to parse json");
                continue;
            };
            state.update(&event).unwrap();
            if let riichi::mjai::Event::Tsumo { actor, .. } = event
                && actor != state.player_id
            {
                continue;
            }
            print!("\x1B[2J\x1B[1;1H");
            println!("{}", ExpandedState::from_state(state.clone()).to_log_string());
        }
    }
}
