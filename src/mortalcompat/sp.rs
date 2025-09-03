//! Single-player table compatibility layer
use crate::mortalcompat::event::possible_events;

// TODO: When showing yaku names, use a bitfield instead of a hashmap and include the average dora count
// This will allow for localization and more standardization
// See tenhou

/// Expected values of discarding specific tiles in single-player mahjong.
/// Assumes riichi tsumo ippatsu if possible.
/// Does not calculate tewagari and shanten-down for 3+ shanten hands.
pub fn single_player_tables(state: &riichi::state::PlayerState) -> Option<Vec<riichi::algo::sp::Candidate>> {
    let shanten = state.real_time_shanten();
    if state.tiles_left < 4 {
        return None;
    }
    if shanten == -1 {
        return None;
    }
    let mut can_discard = state.last_cans.can_discard;
    let (tsumos_left, calc_haitei) = if can_discard {
        (state.tiles_left / 4, state.tiles_left % 4 == 0)
    } else {
        let target = state.rel(state.last_cans.target_actor) as u8;
        let tiles_left_at_next_tsumo = state.tiles_left.saturating_sub(4 - target);
        (tiles_left_at_next_tsumo / 4, tiles_left_at_next_tsumo % 4 == 0)
    };
    if tsumos_left < 1 {
        return None;
    }

    let num_doras_in_fuuro = if state.is_menzen && state.ankan_overview[0].is_empty() {
        0
    } else {
        let num_doras_in_tehai: u8 = state
            .dora_indicators
            .iter()
            .map(|ind| state.tehai[ind.next().as_usize()])
            .sum();
        let num_akas = state.akas_in_hand.iter().filter(|&&b| b).count() as u8;
        state.doras_owned[0] - num_doras_in_tehai - num_akas
    };
    let calc_double_riichi = can_discard && state.can_w_riichi;

    let mut tehai = state.tehai;
    let mut akas_in_hand = state.akas_in_hand;
    let is_discard_after_riichi = can_discard && state.riichi_accepted[0];
    if is_discard_after_riichi {
        let last_tsumo = state.last_self_tsumo?;
        tehai[last_tsumo.deaka().as_usize()] -= 1;
        match last_tsumo.as_u8() {
            riichi::tu8!(5mr) => akas_in_hand[0] = false,
            riichi::tu8!(5pr) => akas_in_hand[1] = false,
            riichi::tu8!(5sr) => akas_in_hand[2] = false,
            _ => (),
        }
        can_discard = false;
    }

    let init_state = riichi::algo::sp::InitState {
        tehai,
        akas_in_hand,
        tiles_seen: state.tiles_seen,
        akas_seen: state.akas_seen,
    };
    let sp_calc = riichi::algo::sp::SPCalculator {
        tehai_len_div3: state.tehai_len_div3,
        is_menzen: state.is_menzen,
        chis: &state.chis,
        pons: &state.pons,
        minkans: &state.minkans,
        ankans: &state.ankans,
        bakaze: state.bakaze.as_u8(),
        jikaze: state.jikaze.as_u8(),
        num_doras_in_fuuro,
        prefer_riichi: state.self_riichi_declared() || state.last_cans.can_riichi || shanten != 0,
        dora_indicators: &state.dora_indicators,
        calc_double_riichi,
        calc_haitei,
        sort_result: true,
        maximize_win_prob: false,
        calc_tegawari: shanten <= 2 && !state.self_riichi_declared(),
        calc_shanten_down: shanten <= 2 && !state.self_riichi_declared(),
    };

    let mut max_ev_table = sp_calc.calc(init_state, can_discard, tsumos_left, shanten).ok()?;
    if is_discard_after_riichi {
        max_ev_table[0].tile = state.last_self_tsumo?;
    }

    Some(max_ev_table)
}

/// Single player tables after possible actions.
pub fn single_player_tables_after_actions(
    state: &riichi::state::PlayerState,
) -> Vec<(Option<riichi::mjai::Event>, Vec<riichi::algo::sp::Candidate>)> {
    let mut candidates = vec![];
    if state.last_cans.can_riichi {
        // if can_riichi then no action is equivalent to an explicit deny of riichi
        let mut state = state.clone();
        state.last_cans.can_riichi = false;
        candidates.push((None, single_player_tables(&state).unwrap_or_default()));
    } else {
        candidates.push((None, single_player_tables(state).unwrap_or_default()));
    }
    for event in possible_events(state) {
        let mut state = state.clone();
        state.update(&event).unwrap();
        let mut tables = single_player_tables(&state).unwrap_or_default();
        match event {
            riichi::mjai::Event::Chi { pai, .. } | riichi::mjai::Event::Pon { pai, .. } => {
                tables.retain(|candidate| candidate.tile.deaka() != pai.deaka());
            }
            _ => {}
        };

        candidates.push((Some(event), tables))
    }
    candidates
}

pub trait CandidateExt {
    fn to_candidate_string(&self) -> String;
}

impl CandidateExt for riichi::algo::sp::Candidate {
    fn to_candidate_string(&self) -> String {
        format!(
            "{:<3} {:>5} {:>6} {:>6.2}% {:>6.2}% {} {} {}{}",
            self.tile.to_string(),
            self.exp_values.first().map(|v| *v as i32).unwrap_or(0),
            self.exp_values
                .first()
                .zip(self.win_probs.first())
                .map(|(v, w)| (v / w).round() as i32)
                .unwrap_or(0),
            self.win_probs.first().map(|w| w * 100.0).unwrap_or(0.0),
            self.tenpai_probs.first().map(|t| t * 100.0).unwrap_or(0.0),
            if self.shanten_down { '-' } else { '+' },
            self.num_required_tiles,
            self.required_tiles
                .iter()
                .map(|r| format!("{}[{}]", r.tile, r.count))
                .collect::<Vec<_>>()
                .join(" "),
            if !self.yaku_names.is_empty() {
                format!(
                    " | {}",
                    self.yaku_names[0]
                        .iter()
                        .filter(|(_, prob)| *prob > 0.01)
                        .map(|(yaku, prob)| format!("{} ({}%)", yaku, ((prob / self.win_probs[0]) * 100.0) as u8))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            } else {
                "".to_owned()
            }
        )
    }
}
