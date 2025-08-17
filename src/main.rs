mod danger;
mod ekyumoecompat;
mod mortalcompat;
use ekyumoecompat::{Detail, read_ekyumoe_log};
use mortalcompat::{ActionType, calculate_agari, event_to_string, single_player_tables_after_calls};
use std::io::BufRead;

use crate::mortalcompat::CandidateExt;

/// State of the board that is not immediately evident such as shanten, expected score or tile danger
pub struct ExpandedState {
    /// Mortal's player state
    state: riichi::state::PlayerState,
    /// Expanded metadata given by mortal for the current state.
    /// Empty for actions that cannot have a response (equivalent to a single 100% "none").
    details: Vec<Detail>,
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
    /// Wall danger kind for each tile based on Chance rules.
    /// Player danger implicitly already calculates Chance, this field is only for quickly understanding why a tile is safe.
    wall_danger: [danger::WallDangerKind; 34],
}

impl ExpandedState {
    pub fn from_state(state: riichi::state::PlayerState, details: Option<Vec<Detail>>) -> Self {
        let shanten = state.real_time_shanten();

        // TODO: proper agari after Hora event
        Self {
            shanten,
            details: details.unwrap_or_default(),
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
        let details_string = self
            .details
            .iter()
            .map(|detail| format!("{}({:.2}%)", event_to_string(&detail.action), detail.prob * 100.0))
            .collect::<Vec<_>>()
            .join(" ");
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
                            if *fu != 0 { fu.to_string() } else { "".to_owned() },
                            if *tile == riichi::must_tile!(riichi::tu8!(?)) {
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
            .iter()
            .enumerate()
            .clone()
            .map(|(i, danger)| {
                danger
                    .sorted_tile_weights()
                    .iter()
                    .filter(|(_, danger)| *danger > 0.0)
                    .map(|(tile, danger)| {
                        let mut danger_info = std::collections::HashSet::new();
                        if !matches!(self.wall_danger[tile.as_usize()], danger::WallDangerKind::None) {
                            danger_info.insert(self.wall_danger[tile.as_usize()].to_acronym());
                        }
                        for wait in self.danger[i].waits.iter() {
                            if wait.wait.waits.contains(&tile.as_u8()) {
                                if matches!(wait.wait.kind, danger::WaitKind::Ryanmen) && wait.genbutsu {
                                    danger_info.insert("suji");
                                }
                                if wait.ura_suji {
                                    danger_info.insert("urasuji");
                                }
                                if wait.matagi_suji_early {
                                    danger_info.insert("msE");
                                }
                                if wait.matagi_suji_riichi {
                                    danger_info.insert("msR");
                                }
                                if wait.riichi_suji_trap {
                                    danger_info.insert("sujitrap");
                                }
                                if wait.dora_involved {
                                    danger_info.insert("dora");
                                }
                            }
                        }
                        format!(
                            "{}({:.1}{})",
                            tile,
                            danger,
                            if !danger_info.is_empty() {
                                " ".to_owned() + &danger_info.iter().cloned().collect::<Vec<_>>().join(" ")
                            } else {
                                "".to_owned()
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{} ({}{}){}{}{}\n{}",
            riichi::hand::tiles_to_string(&self.state.tehai, self.state.akas_in_hand),
            self.shanten,
            if self.state.at_furiten { " - furiten" } else { "" },
            if !agari_string.is_empty() {
                format!("\nwaits: {agari_string}")
            } else {
                "".to_string()
            },
            if !details_string.is_empty() {
                format!("\n{details_string}")
            } else {
                "".to_string()
            },
            if !candidates_string.is_empty() {
                format!("\n{candidates_string}")
            } else {
                "".to_string()
            },
            danger_string,
        )
    }
}

/// Provide live analysis, meant to be used alongside a mortal analysis tool
fn main_live_analysis(player_id: u8) {
    // TODO: Colored logs
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
        println!("{}", ExpandedState::from_state(state.clone(), None).to_log_string());
    }
}

/// Provide extra information to an ekyumoe analysis
fn main_ekyumoe_analysis(path: &str) {
    // TODO: Include mortal's thoughts directly in
    let log = read_ekyumoe_log(&path);
    let mut state = riichi::state::PlayerState::new(log.player_id);
    for (event, details) in log.events_with_detail() {
        state.update(&event).unwrap();
        println!("\n{event:?}");
        if let riichi::mjai::Event::Tsumo { actor, .. } = event
            && actor != state.player_id
        {
            continue;
        }
        println!("{}", ExpandedState::from_state(state.clone(), details).to_log_string());
    }
}

pub fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args
        .get(0)
        .expect("usage: washizu live [player_id] | washizu ekyumoe [path]")
        .as_str()
    {
        "live" => {
            let player_id = args
                .get(1)
                .expect("missing player_id for live")
                .parse::<u8>()
                .expect("invalid player_id");
            main_live_analysis(player_id);
        }
        "ekyumoe" => {
            let path = args.get(1).expect("missing path for ekyumoe");
            main_ekyumoe_analysis(path);
        }
        cmd => {
            panic!("unrecognized command: {}", cmd);
        }
    }
}
