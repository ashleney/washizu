//! Compatibility layer with mortal's libriichi that provides more customized alternatives to internal functions.
//! Assumes 's/pub(super)/pub/g' was applied to Mortal's codebase.

/// types of actions a player can do that changes their sp tables
pub enum ActionType {
    Pass,
    Dahai,
    Reach,
    ChiLow,
    ChiMid,
    ChiHigh,
    Pon,
    Daiminkan,
    Kakan,
    Ankan,
}

impl ActionType {
    pub fn to_string(&self) -> &'static str {
        match self {
            ActionType::Pass => "pass",
            ActionType::Dahai => "dahai",
            ActionType::Reach => "reach",
            ActionType::ChiLow => "chi_low",
            ActionType::ChiMid => "chi_mid",
            ActionType::ChiHigh => "chi_high",
            ActionType::Pon => "pon",
            ActionType::Daiminkan => "daiminkan",
            ActionType::Kakan => "kakan",
            ActionType::Ankan => "ankan",
        }
    }
}

/// Expected values of discarding specific tiles in single-player mahjong.
/// Assumes riichi tsumo ippatsu if possible.
/// Does not calculate tewagari and shanten-down for 3+ shanten hands.
pub fn single_player_tables(state: &riichi::state::PlayerState, shanten: i8) -> Option<Vec<riichi::algo::sp::Candidate>> {
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
        calc_tegawari: shanten <= 2,
        calc_shanten_down: shanten <= 2,
    };

    let mut max_ev_table = sp_calc.calc(init_state, can_discard, tsumos_left, shanten).ok()?;
    if is_discard_after_riichi {
        max_ev_table[0].tile = state.last_self_tsumo?;
    }

    Some(max_ev_table)
}

/// Single player tables after possible calls.
/// Partially copied from BatchAgent::get_reaction.
pub fn single_player_tables_after_calls(
    state: &riichi::state::PlayerState,
) -> Vec<(ActionType, Vec<riichi::algo::sp::Candidate>)> {
    let mut candidates = vec![];
    if state.last_cans.can_discard {
        if state.last_cans.can_riichi {
            candidates.push((
                ActionType::Reach,
                single_player_tables(state, state.real_time_shanten()).unwrap_or_default(),
            ));
            let mut state = state.clone();
            state.last_cans.can_riichi = false;
            candidates.push((
                ActionType::Dahai,
                single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
            ));
        } else {
            candidates.push((
                ActionType::Dahai,
                single_player_tables(state, state.real_time_shanten()).unwrap_or_default(),
            ));
        }
    } else {
        candidates.push((
            ActionType::Pass,
            single_player_tables(state, state.real_time_shanten()).unwrap_or_default(),
        ))
    }
    if state.last_cans.can_chi_low {
        let pai = state.last_kawa_tile().unwrap();
        let first = pai.next();
        let can_akaize_consumed = match pai.as_u8() {
            riichi::tu8!(3m) | riichi::tu8!(4m) => state.akas_in_hand[0],
            riichi::tu8!(3p) | riichi::tu8!(4p) => state.akas_in_hand[1],
            riichi::tu8!(3s) | riichi::tu8!(4s) => state.akas_in_hand[2],
            _ => false,
        };
        let consumed = if can_akaize_consumed {
            [first.akaize(), first.next().akaize()]
        } else {
            [first, first.next()]
        };
        let mut state = state.clone();
        state
            .update(&riichi::mjai::Event::Chi {
                actor: state.player_id,
                target: state.last_cans.target_actor,
                pai,
                consumed,
            })
            .unwrap();
        candidates.push((
            ActionType::ChiLow,
            single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
        ));
    }
    if state.last_cans.can_chi_mid {
        let pai = state.last_kawa_tile().unwrap();
        let can_akaize_consumed = match pai.as_u8() {
            riichi::tu8!(4m) | riichi::tu8!(6m) => state.akas_in_hand[0],
            riichi::tu8!(4p) | riichi::tu8!(6p) => state.akas_in_hand[1],
            riichi::tu8!(4s) | riichi::tu8!(6s) => state.akas_in_hand[2],
            _ => false,
        };
        let consumed = if can_akaize_consumed {
            [pai.prev().akaize(), pai.next().akaize()]
        } else {
            [pai.prev(), pai.next()]
        };

        let mut state = state.clone();
        state
            .update(&riichi::mjai::Event::Chi {
                actor: state.player_id,
                target: state.last_cans.target_actor,
                pai,
                consumed,
            })
            .unwrap();
        candidates.push((
            ActionType::ChiMid,
            single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
        ));
    }
    if state.last_cans.can_chi_high {
        let pai = state.last_kawa_tile().unwrap();
        let last = pai.prev();
        let can_akaize_consumed = match pai.as_u8() {
            riichi::tu8!(6m) | riichi::tu8!(7m) => state.akas_in_hand[0],
            riichi::tu8!(6p) | riichi::tu8!(7p) => state.akas_in_hand[1],
            riichi::tu8!(6s) | riichi::tu8!(7s) => state.akas_in_hand[2],
            _ => false,
        };
        let consumed = if can_akaize_consumed {
            [last.prev().akaize(), last.akaize()]
        } else {
            [last.prev(), last]
        };

        let mut state = state.clone();
        state
            .update(&riichi::mjai::Event::Chi {
                actor: state.player_id,
                target: state.last_cans.target_actor,
                pai,
                consumed,
            })
            .unwrap();
        candidates.push((
            ActionType::ChiHigh,
            single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
        ));
    }
    if state.last_cans.can_pon {
        let pai = state.last_kawa_tile().unwrap();
        let can_akaize_consumed = match pai.as_u8() {
            riichi::tu8!(5m) => state.akas_in_hand[0],
            riichi::tu8!(5p) => state.akas_in_hand[1],
            riichi::tu8!(5s) => state.akas_in_hand[2],
            _ => false,
        };
        let consumed = if can_akaize_consumed {
            [pai.akaize(), pai.deaka()]
        } else {
            [pai.deaka(); 2]
        };

        let mut state = state.clone();
        state
            .update(&riichi::mjai::Event::Pon {
                actor: state.player_id,
                target: state.last_cans.target_actor,
                pai,
                consumed,
            })
            .unwrap();
        candidates.push((
            ActionType::Pon,
            single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
        ));
    }
    if state.last_cans.can_daiminkan {
        let tile = state.last_kawa_tile().unwrap();
        let consumed = if tile.is_aka() {
            [tile.deaka(); 3]
        } else {
            [tile.akaize(), tile, tile]
        };
        let mut state = state.clone();
        state
            .update(&riichi::mjai::Event::Daiminkan {
                actor: state.player_id,
                target: state.last_cans.target_actor,
                pai: tile,
                consumed,
            })
            .unwrap();
        candidates.push((
            ActionType::Daiminkan,
            single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
        ));
    }
    if state.last_cans.can_ankan {
        let ankan_candidates = state.ankan_candidates();
        if !ankan_candidates.is_empty() {
            let tile = ankan_candidates[0];
            let mut state = state.clone();
            state
                .update(&riichi::mjai::Event::Ankan {
                    actor: state.player_id,
                    consumed: [tile.akaize(), tile, tile, tile],
                })
                .unwrap();
            candidates.push((
                ActionType::Ankan,
                single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
            ));
        }
    }
    if state.last_cans.can_kakan {
        let kakan_candidates = state.kakan_candidates();
        if !kakan_candidates.is_empty() {
            let tile = kakan_candidates[0];
            let can_akaize_target = match tile.as_u8() {
                riichi::tu8!(5m) => state.akas_in_hand[0],
                riichi::tu8!(5p) => state.akas_in_hand[1],
                riichi::tu8!(5s) => state.akas_in_hand[2],
                _ => false,
            };
            let (pai, consumed) = if can_akaize_target {
                (tile.akaize(), [tile.deaka(); 3])
            } else {
                (tile.deaka(), [tile.akaize(), tile.deaka(), tile.deaka()])
            };
            let mut state = state.clone();
            state
                .update(&riichi::mjai::Event::Kakan {
                    actor: state.player_id,
                    pai,
                    consumed,
                })
                .unwrap();
            candidates.push((
                ActionType::Kakan,
                single_player_tables(&state, state.real_time_shanten()).unwrap_or_default(),
            ));
        }
    }

    candidates
}

/// Calculate the agari of a given winning tile, assuming no ura-dora.
pub fn calculate_agari(
    state: &riichi::state::PlayerState,
    winning_tile: riichi::tile::Tile,
    is_ron: bool,
) -> Option<riichi::algo::agari::Agari> {
    if !is_ron && state.can_w_riichi {
        return Some(riichi::algo::agari::Agari::Yakuman(1));
    }

    let additional_hans = if is_ron {
        [
            state.self_riichi_declared(),
            state.is_w_riichi,
            state.at_ippatsu,
            state.tiles_left == 0,
            state.chankan_chance.is_some(),
        ]
        .iter()
        .filter(|&&b| b)
        .count() as u8
    } else {
        [
            state.self_riichi_declared(),
            state.is_w_riichi,
            state.at_ippatsu,
            state.is_menzen,
            state.tiles_left == 0 && !state.at_rinshan,
            state.at_rinshan,
        ]
        .iter()
        .filter(|&&b| b)
        .count() as u8
    };

    let mut tehai = state.tehai;
    let mut final_doras_owned = state.doras_owned[0];
    if tehai.iter().sum::<u8>() % 3 != 2 {
        let tid = winning_tile.deaka().as_usize();
        tehai[tid] += 1;
        final_doras_owned += state.dora_factor[tid];
        if winning_tile.is_aka() {
            final_doras_owned += 1;
        };
    }

    let agari_calc = riichi::algo::agari::AgariCalculator {
        tehai: &tehai,
        is_menzen: state.is_menzen,
        chis: &state.chis,
        pons: &state.pons,
        minkans: &state.minkans,
        ankans: &state.ankans,
        bakaze: state.bakaze.as_u8(),
        jikaze: state.jikaze.as_u8(),
        winning_tile: winning_tile.deaka().as_u8(),
        is_ron,
    };

    agari_calc.agari(additional_hans, final_doras_owned)
}

pub trait CandidateExt {
    fn to_candidate_string(&self) -> String;
}

impl CandidateExt for riichi::algo::sp::Candidate {
    fn to_candidate_string(&self) -> String {
        format!(
            "{:<3} {:>6} {:>6.2}% {:>6.2}% {} {} {}",
            self.tile.to_string(),
            if !self.exp_values.is_empty() {
                (self.exp_values[0] / self.win_probs[0]).round() as i32
            } else {
                0
            },
            if !self.win_probs.is_empty() {
                self.win_probs[0] * 100.0
            } else {
                0.0
            },
            if !self.tenpai_probs.is_empty() {
                self.tenpai_probs[0] * 100.0
            } else {
                0.0
            },
            if self.shanten_down { '-' } else { '+' },
            self.num_required_tiles,
            self.required_tiles
                .iter()
                .map(|r| format!("{}[{}]", r.tile, r.count))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}
