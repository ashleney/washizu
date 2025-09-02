use super::action::ActionCandidate;
use super::item::{ChiPon, KawaItem, Sutehai};
use crate::algo::sp::Candidate;
use crate::hand::tiles_to_string;
use crate::must_tile;
use crate::tile::Tile;
use std::iter;
use anyhow::Result;
use derivative::Derivative;
use pyo3::prelude::*;
use serde_json as json;
use tinyvec::{ArrayVec, TinyVec};
/// `PlayerState` is the core of the lib, which holds all the observable game
/// state information from a specific seat's perspective with the ability to
/// identify the legal actions the specified player can make upon an incoming
/// mjai event, along with some helper functions to build an actual agent.
/// Notably, `PlayerState` encodes observation features into numpy arrays which
/// serve as inputs for deep learning model.
#[pyclass]
#[derive(Clone, Derivative)]
#[derivative(Default)]
pub struct PlayerState {
    pub player_id: u8,
    /// Does not include aka.
    #[derivative(Default(value = "[0; 34]"))]
    pub tehai: [u8; 34],
    /// Does not consider yakunashi, but does consider other kinds of
    /// furiten.
    #[derivative(Default(value = "[false; 34]"))]
    pub waits: [bool; 34],
    #[derivative(Default(value = "[0; 34]"))]
    pub dora_factor: [u8; 34],
    /// For calculating `waits` and `doras_seen`, also for SPCalculator.
    #[derivative(Default(value = "[0; 34]"))]
    pub tiles_seen: [u8; 34],
    /// For SPCalculator.
    pub akas_seen: [bool; 3],
    #[derivative(Default(value = "[false; 34]"))]
    pub keep_shanten_discards: [bool; 34],
    #[derivative(Default(value = "[false; 34]"))]
    pub next_shanten_discards: [bool; 34],
    #[derivative(Default(value = "[false; 34]"))]
    pub forbidden_tiles: [bool; 34],
    /// Used for furiten check.
    #[derivative(Default(value = "[false; 34]"))]
    pub discarded_tiles: [bool; 34],
    pub bakaze: Tile,
    pub jikaze: Tile,
    /// Counts from 0 unlike mjai.
    pub kyoku: u8,
    pub honba: u8,
    pub kyotaku: u8,
    /// Rotated to be relative, so `scores[0]` is the score of the player.
    pub scores: [i32; 4],
    pub rank: u8,
    /// Relative to `player_id`.
    pub oya: u8,
    /// Including 西入 sudden death.
    pub is_all_last: bool,
    pub dora_indicators: ArrayVec<[Tile; 5]>,
    /// 24 is the theoretical max size of kawa, however, since None is included
    /// in the kawa, in some very rare cases (about one in a million hanchans),
    /// the size can exceed 24.
    ///
    /// Reference:
    /// <https://detail.chiebukuro.yahoo.co.jp/qa/question_detail/q1020002370>
    pub kawa: [TinyVec<[Option<KawaItem>; 24]>; 4],
    pub last_tedashis: [Option<Sutehai>; 4],
    pub riichi_sutehais: [Option<Sutehai>; 4],
    /// Using 34-D arrays here may be more efficient, but I don't want to mess up
    /// with aka doras.
    pub kawa_overview: [ArrayVec<[Tile; 24]>; 4],
    pub fuuro_overview: [ArrayVec<[ArrayVec<[Tile; 4]>; 4]>; 4],
    /// In this field all `Tile` are deaka'd.
    pub ankan_overview: [ArrayVec<[Tile; 4]>; 4],
    pub riichi_declared: [bool; 4],
    pub riichi_accepted: [bool; 4],
    pub at_turn: u8,
    pub tiles_left: u8,
    pub intermediate_kan: ArrayVec<[Tile; 4]>,
    pub intermediate_chi_pon: Option<ChiPon>,
    pub shanten: i8,
    pub last_self_tsumo: Option<Tile>,
    pub last_kawa_tile: Option<Tile>,
    pub last_cans: ActionCandidate,
    /// Both deaka'd
    pub ankan_candidates: ArrayVec<[Tile; 3]>,
    pub kakan_candidates: ArrayVec<[Tile; 3]>,
    pub chankan_chance: Option<()>,
    pub can_w_riichi: bool,
    pub is_w_riichi: bool,
    pub at_rinshan: bool,
    pub at_ippatsu: bool,
    pub at_furiten: bool,
    pub to_mark_same_cycle_furiten: Option<()>,
    /// Used for 4-kan check.
    pub kans_on_board: u8,
    pub is_menzen: bool,
    /// For agari calc, all deaka'd.
    pub chis: ArrayVec<[u8; 4]>,
    pub pons: ArrayVec<[u8; 4]>,
    pub minkans: ArrayVec<[u8; 4]>,
    pub ankans: ArrayVec<[u8; 4]>,
    /// Including aka, originally for agari calc usage but also encoded as a
    /// feature to the obs.
    pub doras_owned: [u8; 4],
    pub doras_seen: u8,
    pub akas_in_hand: [bool; 3],
    /// For shanten calc.
    pub tehai_len_div3: u8,
    /// Used in can_riichi, also in single-player features to get the shanten
    /// for 3n+2.
    pub has_next_shanten_discard: bool,
}
#[pymethods]
impl PlayerState {
    /// Panics if `player_id` is outside of range [0, 3].
    #[new]
    #[must_use]
    pub fn new(player_id: u8) -> Self {
        assert!(player_id < 4, "{player_id} is not in range [0, 3]");
        Self {
            player_id,
            ..Default::default()
        }
    }
    /// Returns an `ActionCandidate`.
    #[pyo3(name = "update")]
    pub fn update_json(&mut self, mjai_json: &str) -> Result<ActionCandidate> {
        let event = json::from_str(mjai_json)?;
        self.update(&event)
    }
    /// Raises an exception if the action is not valid.
    #[pyo3(name = "validate_reaction")]
    pub fn validate_reaction_json(&self, mjai_json: &str) -> Result<()> {
        let action = json::from_str(mjai_json)?;
        self.validate_reaction(&action)
    }
    /// For debug only.
    ///
    /// Return a human readable description of the current state.
    #[must_use]
    pub fn brief_info(&self) -> String {
        let waits = self
            .waits
            .iter()
            .enumerate()
            .filter(|&(_, &b)| b)
            .map(|(i, _)| must_tile!(i))
            .collect::<Vec<_>>();
        let zipped_kawa = self
            .kawa[0]
            .iter()
            .chain(iter::repeat(&None))
            .zip(self.kawa[1].iter().chain(iter::repeat(&None)))
            .zip(self.kawa[2].iter().chain(iter::repeat(&None)))
            .zip(self.kawa[3].iter().chain(iter::repeat(&None)))
            .take_while(|row| !matches!(row, & (((None, None), None), None)))
            .enumerate()
            .map(|(i, (((a, b), c), d))| {
                format!(
                    "{i:2}. {}\t{}\t{}\t{}", a.as_ref().map_or_else(|| "-".to_owned(), |
                    item | item.to_string()), b.as_ref().map_or_else(|| "-".to_owned(), |
                    item | item.to_string()), c.as_ref().map_or_else(|| "-".to_owned(), |
                    item | item.to_string()), d.as_ref().map_or_else(|| "-".to_owned(), |
                    item | item.to_string()),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let can_discard = self.last_cans.can_discard;
        let mut sp_tables = Candidate::csv_header(can_discard).join("\t");
        if let Ok(tables) = self.single_player_tables() {
            for candidate in tables.max_ev_table {
                sp_tables.push('\n');
                sp_tables.push_str(&candidate.csv_row(can_discard).join("\t"));
            }
        }
        format!(
            r#"player (abs): {}
oya (rel): {}
kyoku: {}{}-{}
turn: {}
jikaze: {}
score (rel): {:?}
tehai: {}
fuuro: {:?}
ankan: {:?}
tehai len: {}
shanten: {} (actual: {})
furiten: {}
waits: {waits:?}
dora indicators: {:?}
doras owned: {:?}
doras seen: {}
action candidates: {:#?}
last self tsumo: {:?}
last kawa tile: {:?}
tiles left: {}
kawa:
{zipped_kawa}
single player table (max EV):
{sp_tables}"#,
            self.player_id, self.oya, self.bakaze, self.kyoku + 1, self.honba, self
            .at_turn, self.jikaze, self.scores, tiles_to_string(& self.tehai, self
            .akas_in_hand), self.fuuro_overview[0], self.ankan_overview[0], self
            .tehai_len_div3, self.shanten, self.real_time_shanten(), self.at_furiten,
            self.dora_indicators, self.doras_owned, self.doras_seen, self.last_cans, self
            .last_self_tsumo, self.last_kawa_tile, self.tiles_left,
        )
    }
}
