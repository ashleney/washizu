use riichi::algo::agari::yaku::{YakuLanguage, localize_yaku};
use riichi::algo::agari::{Agari, AgariWithYaku};
use riichi::algo::danger::{PlayerDanger, WaitShape};
use riichi::algo::sp::{EventCandidate, SPOptions};
use riichi::hand::tiles_to_string;
use riichi::state::PlayerState;
use riichi::tile::Tile;
use riichi::{must_tile, t};

/// Expanded mortal state
use crate::ekyumoe::Detail;

/// State of the board that is not immediately evident such as shanten, expected score or tile danger
pub struct ExpandedState {
    /// Mortal's player state
    pub state: PlayerState,
    /// Expanded metadata given by mortal for the current state.
    /// Empty for actions that cannot have a response (equivalent to a single 100% "none").
    pub details: Vec<Detail>,
    /// Shanten of the current hand. -1 for agari hands.
    pub shanten: i8,
    /// Actions that the player can do and their expected values assuming tsumo-only.
    /// When riichi is an option, dahai will be assumed to be damaten.
    /// Each candidate contains the chance the player will reach agari/tenpai in `length - n` tsumos.
    /// Expected value is equal to average score * win probability where average score assumes riichi tsumo ippatsu if possible.
    /// Candidates are sorted by expected value.
    /// Shanten down candidates are not processed for hands with 3+ shanten.
    pub candidates: Vec<EventCandidate>,
    /// Agari state (including specific yaku names, han and fu) of individual waits.
    /// For tenpai hands assumes the score is calculated as ron with no ura-dora.
    /// For agari hands (implied tsumo) the tile will be "?" and the score will be calculated as tsumo with no ura-dora.
    pub agari: Vec<(Tile, Option<AgariWithYaku>)>,
    /// Danger weights and wait types for each tile based on a player's discard.
    /// Estimates danger by calculating the amount of tile combinations that can lead to a player having this wait.
    /// Uses multipliers for more common types of waits. Does not analyze tedashi patterns.
    pub danger: [PlayerDanger; 4],
}

impl ExpandedState {
    pub fn from_state(state: PlayerState, details: Option<Vec<Detail>>) -> Self {
        let shanten = state.real_time_shanten();

        let options = if shanten <= 3 {
            SPOptions {
                max_shanten: 3,
                calc_tegawari: Some(2),
                calc_shanten_down: Some(2),
                ..Default::default()
            }
        } else {
            SPOptions {
                max_shanten: 5,
                ..Default::default()
            }
        };

        // TODO: proper agari after Hora event
        // Hora is not available in live so low priority
        Self {
            shanten,
            details: details.unwrap_or_default(),
            candidates: state.single_player_tables_for_events(&options),
            agari: if shanten == -1
                && let Some(winning_tile) = state.last_self_tsumo
            {
                vec![(
                    t!(?),
                    state.calculate_agari(winning_tile, false, &[]).expect("incorrect shanten"),
                )]
            } else if !state.last_cans.can_discard {
                state
                    .waits
                    .iter()
                    .enumerate()
                    .filter(|&(_, &b)| b)
                    .map(|(tile, _)| must_tile!(tile))
                    .map(|tile| (tile, state.calculate_agari(tile, true, &[]).expect("incorrect wait")))
                    .collect()
            } else {
                vec![]
            },
            danger: state.calculate_danger(),
            state,
        }
    }

    pub fn to_log_string(&self) -> String {
        let details_string = self
            .details
            .iter()
            .map(|detail| format!("{}({:.2}%)", detail.action.to_decision_string(), detail.prob * 100.0))
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
                        Some(agari_with_yaku) => match agari_with_yaku.agari {
                            a @ Agari::Normal { fu, han } => format!(
                                "{}han{}fu = {}{extra_points_string} [{}]",
                                han,
                                if fu != 0 { fu.to_string() } else { "".to_owned() },
                                if *tile == t!(?) {
                                    a.point(self.state.is_oya()).tsumo_total(self.state.is_oya())
                                } else {
                                    a.point(self.state.is_oya()).ron
                                },
                                agari_with_yaku.localize_yaku(YakuLanguage::RomajiShort).join(", "),
                            ),
                            a @ Agari::Yakuman(count) => format!(
                                "{}yakuman = {}{extra_points_string} [{}]",
                                if count == 1 { "".to_owned() } else { format!("{count}x ") },
                                a.point(self.state.is_oya()).tsumo_total(self.state.is_oya()),
                                agari_with_yaku.localize_yaku(YakuLanguage::RomajiShort).join(", "),
                            ),
                        },
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let candidates_string = self
            .candidates
            .iter()
            .map(|candidate| {
                let exp_value = candidate.exp_values.first().cloned().unwrap_or(0.0);
                let win_prob = candidate.win_probs.first().cloned().unwrap_or(0.0);
                let tenpai_prob = candidate.tenpai_probs.first().cloned().unwrap_or(0.0);
                let mut yaku_str = vec![];
                if let Some(yaku_probs) = candidate.yaku.first() {
                    for (y, p) in yaku_probs.sorted_yaku() {
                        yaku_str.push(format!(
                            "{} ({}%)",
                            localize_yaku(y, YakuLanguage::RomajiShort),
                            ((p / win_prob) * 100.0).round()
                        ));
                    }
                    if candidate.yaku[0].dora > 0.0 {
                        yaku_str.push(format!("Dora ({:.2})", candidate.yaku[0].dora / win_prob));
                    }
                    if candidate.yaku[0].aka_dora > 0.0 {
                        yaku_str.push(format!("Aka ({:.2})", candidate.yaku[0].aka_dora / win_prob));
                    }
                    if candidate.yaku[0].ura_dora > 0.0 {
                        yaku_str.push(format!("Ura ({:.2})", candidate.yaku[0].ura_dora / win_prob));
                    }
                }
                format!(
                    "{:<3} {:>5} {:>6} {:>6.2}% {:>6.2}% {} {} {} {}",
                    candidate.event.to_decision_string(),
                    exp_value.round(),
                    if win_prob > 0.0 { (exp_value / win_prob).round() } else { 0.0 },
                    win_prob * 100.0,
                    tenpai_prob * 100.0,
                    candidate.shanten,
                    candidate.num_required_tiles,
                    candidate
                        .required_tiles
                        .iter()
                        .map(|r| format!("{}@{}", r.tile, r.count))
                        .collect::<Vec<_>>()
                        .join(" "),
                    yaku_str.join(" "),
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
                        for wait in self.danger[i].waits.iter() {
                            if wait.kind.waits.contains(&tile.as_u8()) {
                                if matches!(wait.kind.shape, WaitShape::Ryanmen) && wait.genbutsu {
                                    danger_info.insert("suji");
                                }
                                if wait.matagi_suji_early {
                                    danger_info.insert("msE");
                                }
                                if wait.weight > 0.0 {
                                    if wait.ura_suji {
                                        danger_info.insert("urasuji");
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
            "{} ({}{}){}{}\n{}\n{}\n{}",
            tiles_to_string(&self.state.tehai, self.state.akas_in_hand),
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
            "act   EV  avg.win  win%  tenpai% s. ukeire",
            candidates_string,
            danger_string,
        )
    }
}
