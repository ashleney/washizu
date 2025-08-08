mod danger;
mod mortalcompat;
use mortalcompat::{calculate_agari, single_player_tables};
use std::io::BufRead;

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
pub struct HiddenState {
    /// Mortal's player state
    state: riichi::state::PlayerState,
    /// Shanten of the current hand. -1 for agari hands.
    shanten: i8,
    /// Tiles that the player can discard and their expected values assuming tsumo-only.
    /// When the player cannot dahai, it will only be a single candidate "?" with a list of tiles that are being waited on.
    /// Each candidate contains the chance the player will reach agari/tenpai in `length - n` tsumos.
    /// Expected value is equal to average score * win probability where average score assumes riichi tsumo ippatsu if possible.
    /// Candidates are sorted by win probability.
    /// Shanten down candidates are not processed for hands with 3+ shanten.
    candidates: Vec<riichi::algo::sp::Candidate>,
    /// Agari (including specific han and fu) of individual waits.
    /// For tenpai hands assumes the score is calculated as ron with no ura-dora.
    /// For agari hands (implied tsumo) the tile will be "?" and the score will be calculated as tsumo with no ura-dora.
    agari: Vec<(riichi::tile::Tile, Option<riichi::algo::agari::Agari>)>,
    /// The type of danger for each tile for each player (rel shimocha, toimen, kamicha).
    /// Safety only accounts for genbutsu, chance and ryanmen strategies and can be easily bluffed.
    danger: [Vec<danger::Danger>; 3],
}

impl ToString for HiddenState {
    fn to_string(&self) -> String {
        let agari_string = self
            .agari
            .iter()
            .map(|(tile, agari)| {
                format!(
                    "{} - {}",
                    tile.to_string(),
                    match agari {
                        None => "yakunashi = 0".to_owned(),
                        Some(a @ riichi::algo::agari::Agari::Normal { fu, han }) => format!(
                            "{}han{}fu = {}",
                            han,
                            fu,
                            a.point(self.state.is_oya()).tsumo_total(self.state.is_oya())
                        ),
                        Some(a @ riichi::algo::agari::Agari::Yakuman(count)) => format!(
                            "{}yakuman = {}",
                            count,
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
            .map(|candidate| {
                format!(
                    "{:<3} {:>6} {:>6.2}% {:>6.2}% {} {} {}",
                    candidate.tile.to_string(),
                    if candidate.exp_values.len() > 0 {
                        (candidate.exp_values[0] / candidate.win_probs[0]).round() as i32
                    } else {
                        0
                    },
                    if candidate.win_probs.len() > 0 {
                        candidate.win_probs[0] * 100.0
                    } else {
                        0.0
                    },
                    if candidate.tenpai_probs.len() > 0 {
                        candidate.tenpai_probs[0] * 100.0
                    } else {
                        0.0
                    },
                    if candidate.shanten_down { '-' } else { '+' },
                    candidate.num_required_tiles,
                    candidate
                        .required_tiles
                        .iter()
                        .map(|r| format!("{}[{}]", r.tile, r.count))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let danger_string = self
            .danger
            .iter()
            .map(|danger| {
                danger
                    .iter()
                    .filter(|danger| !matches!(danger.danger_type, danger::DangerType::Safe))
                    .map(|danger| danger.to_short_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{} ({}){}{}\n{}",
            riichi::hand::tiles_to_string(&self.state.tehai, self.state.akas_in_hand),
            self.shanten,
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
            danger_string,
        )
    }
}

impl HiddenState {
    pub fn from_state(state: riichi::state::PlayerState) -> Self {
        let shanten = state.real_time_shanten();

        Self {
            shanten: shanten,
            candidates: single_player_tables(&state, shanten).unwrap_or_default(),
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
            danger: danger::calculate_board_tile_danger(&state),
            state: state,
        }
    }
}

pub fn main() {
    let mut state = riichi::state::PlayerState::new(1);
    // for event in read_json_log("old/12483_8389512805380735157_a.json").unwrap() {
    for event in read_ekyumoe_log("5cfd81c76778959d.json") {
        state.update(&event).unwrap();
        println!("\n{:?}", event);
        if let riichi::mjai::Event::Tsumo { actor, .. } = event
            && actor != state.player_id
        {
            continue;
        }
        println!("{}", HiddenState::from_state(state.clone()).to_string());
    }
}
