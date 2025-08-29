//! Compatibility layer with mortal's libriichi that provides more customized alternatives to internal functions.
//! Assumes 's/pub(super)/pub/g' was applied to Mortal's codebase.

/// Possible events for the current state, excluding dahai
pub fn possible_events(state: &riichi::state::PlayerState) -> Vec<riichi::mjai::Event> {
    let mut events: Vec<riichi::mjai::Event> = vec![];

    if state.last_cans.can_riichi {
        events.push(riichi::mjai::Event::Reach { actor: state.player_id });
    }
    if state.last_cans.can_chi_low {
        let pai = state.last_kawa_tile.unwrap();
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
        events.push(riichi::mjai::Event::Chi {
            actor: state.player_id,
            target: state.last_cans.target_actor,
            pai,
            consumed,
        });
    }
    if state.last_cans.can_chi_mid {
        let pai = state.last_kawa_tile.unwrap();
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
        events.push(riichi::mjai::Event::Chi {
            actor: state.player_id,
            target: state.last_cans.target_actor,
            pai,
            consumed,
        });
    }
    if state.last_cans.can_chi_high {
        let pai = state.last_kawa_tile.unwrap();
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
        events.push(riichi::mjai::Event::Chi {
            actor: state.player_id,
            target: state.last_cans.target_actor,
            pai,
            consumed,
        });
    }
    if state.last_cans.can_pon {
        let pai = state.last_kawa_tile.unwrap();
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
        events.push(riichi::mjai::Event::Pon {
            actor: state.player_id,
            target: state.last_cans.target_actor,
            pai,
            consumed,
        });
    }
    if state.last_cans.can_daiminkan {
        let tile = state.last_kawa_tile.unwrap();
        let consumed = if tile.is_aka() {
            [tile.deaka(); 3]
        } else {
            [tile.akaize(), tile, tile]
        };
        events.push(riichi::mjai::Event::Daiminkan {
            actor: state.player_id,
            target: state.last_cans.target_actor,
            pai: tile,
            consumed,
        });
    }
    if state.last_cans.can_ankan {
        for tile in &state.ankan_candidates {
            events.push(riichi::mjai::Event::Ankan {
                actor: state.player_id,
                consumed: [tile.akaize(), *tile, *tile, *tile],
            });
        }
    }
    if state.last_cans.can_kakan {
        for tile in &state.kakan_candidates {
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
            events.push(riichi::mjai::Event::Kakan {
                actor: state.player_id,
                pai,
                consumed,
            });
        }
    }

    events
}

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
        calc_tegawari: shanten <= 2,
        calc_shanten_down: shanten <= 2,
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

/// Calculate the agari of a given winning tile, assuming no ura-dora.
pub fn calculate_agari_with_names(
    state: &riichi::state::PlayerState,
    winning_tile: riichi::tile::Tile,
    is_ron: bool,
) -> Option<(riichi::algo::agari::Agari, Vec<String>)> {
    if !is_ron && state.can_w_riichi {
        return Some((
            riichi::algo::agari::Agari::Yakuman(1),
            vec![if state.is_oya() {
                "Tenhou".to_owned()
            } else {
                "Chiihou".to_owned()
            }],
        ));
    }

    let mut additional_names = vec![];
    let additional_hans = if is_ron {
        [
            (state.self_riichi_declared(), "Riichi"),
            (state.is_w_riichi, "Double-Riichi"),
            (state.at_ippatsu, "Ippatsu"),
            (state.tiles_left == 0, "Under-River"),
            (state.chankan_chance.is_some(), "Robbing-Kan"),
        ]
        .iter()
        .map(|&(b, n)| {
            if b {
                additional_names.push(n.to_string());
                1
            } else {
                0
            }
        })
        .sum::<u8>()
    } else {
        [
            (state.self_riichi_declared(), "Riichi"),
            (state.is_w_riichi, "Double-Riichi"),
            (state.at_ippatsu, "Ippatsu"),
            (state.is_menzen, "Menzen-Tsumo"),
            (state.tiles_left == 0 && !state.at_rinshan, "Under-Sea"),
            (state.at_rinshan, "After-Kan"),
        ]
        .iter()
        .map(|&(b, n)| {
            if b {
                additional_names.push(n.to_string());
                1
            } else {
                0
            }
        })
        .sum::<u8>()
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
    if final_doras_owned > 0 {
        additional_names.push(format!("Dora-{final_doras_owned}"));
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

    if let Some((agari, mut names)) = agari_calc.agari_with_names(additional_hans, final_doras_owned) {
        names.append(&mut additional_names);
        Some((agari, names))
    } else {
        None
    }
}

pub trait CandidateExt {
    fn to_candidate_string(&self) -> String;
}

impl CandidateExt for riichi::algo::sp::Candidate {
    fn to_candidate_string(&self) -> String {
        format!(
            "{:<3} {:>5} {:>6} {:>6.2}% {:>6.2}% {} {} {}",
            self.tile.to_string(),
            if !self.exp_values.is_empty() {
                self.exp_values[0] as i32
            } else {
                0
            },
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

/// Actionable event to snake_case string
pub fn event_to_string(event: &riichi::mjai::Event) -> String {
    match event {
        riichi::mjai::Event::Dahai { pai, .. } => pai.to_string(),
        riichi::mjai::Event::None => "pass".to_string(),
        riichi::mjai::Event::Chi { pai, consumed, .. } => {
            if pai.next() == consumed[0] {
                "chi_low".to_string()
            } else if consumed[1] == pai.prev() {
                "chi_high".to_string()
            } else {
                "chi_mid".to_string()
            }
        }
        riichi::mjai::Event::Pon { .. } => "pon".to_string(),
        riichi::mjai::Event::Daiminkan { .. } => "kan".to_string(),
        riichi::mjai::Event::Kakan { .. } => "kan".to_string(),
        riichi::mjai::Event::Ankan { .. } => "kan".to_string(),
        riichi::mjai::Event::Reach { .. } => "reach".to_string(),
        riichi::mjai::Event::Hora { .. } => "hora".to_string(),
        riichi::mjai::Event::Ryukyoku { .. } => "ryukyoku".to_string(),
        _ => "".to_string(),
    }
}

pub trait AgariCaculatorWithYaku {
    /// Returns both agari and the names of yaku
    fn agari_with_names(&self, additional_hans: u8, doras: u8) -> Option<(riichi::algo::agari::Agari, Vec<String>)>;
    fn search_yakus_with_names(&self) -> Option<(riichi::algo::agari::Agari, Vec<String>)>;
}

impl AgariCaculatorWithYaku for riichi::algo::agari::AgariCalculator<'_> {
    fn agari_with_names(&self, additional_hans: u8, doras: u8) -> Option<(riichi::algo::agari::Agari, Vec<String>)> {
        if let Some((agari, names)) = self.search_yakus_with_names() {
            Some(match agari {
                riichi::algo::agari::Agari::Normal { fu, han } => (
                    riichi::algo::agari::Agari::Normal {
                        fu,
                        han: han + additional_hans + doras,
                    },
                    names,
                ),
                _ => (agari, names),
            })
        } else if additional_hans == 0 {
            None
        } else if additional_hans + doras >= 5 {
            Some((
                riichi::algo::agari::Agari::Normal {
                    fu: 0,
                    han: additional_hans + doras,
                },
                vec![],
            ))
        } else {
            let (tile14, key) = riichi::algo::agari::get_tile14_and_key(self.tehai);
            let divs = riichi::algo::agari::AGARI_TABLE.get(&key)?;

            let fu = divs
                .iter()
                .map(|div| riichi::algo::agari::DivWorker::new(self, &tile14, div))
                .map(|w| w.calc_fu(false))
                .max()?;
            Some((
                riichi::algo::agari::Agari::Normal {
                    fu,
                    han: additional_hans + doras,
                },
                vec![],
            ))
        }
    }

    fn search_yakus_with_names(&self) -> Option<(riichi::algo::agari::Agari, Vec<String>)> {
        if self.is_menzen && riichi::algo::shanten::calc_kokushi(self.tehai) == -1 {
            if self.tehai[self.winning_tile as usize] == 2 {
                return Some((
                    riichi::algo::agari::Agari::Yakuman(2),
                    vec!["Thirteen-Orphans-Juusanmen".to_string()],
                ));
            } else {
                return Some((riichi::algo::agari::Agari::Yakuman(1), vec!["Thirteen-Orphans".to_string()]));
            }
        }

        let (tile14, key) = riichi::algo::agari::get_tile14_and_key(self.tehai);
        let divs = riichi::algo::agari::AGARI_TABLE.get(&key)?;

        divs.iter()
            .map(|div| riichi::algo::agari::DivWorker::new(self, &tile14, div))
            .filter_map(|w| w.search_yakus_with_names())
            .max_by_key(|(agari, _)| *agari)
    }
}

trait DivWorkerWithNames {
    fn search_yakus_with_names(&self) -> Option<(riichi::algo::agari::Agari, Vec<String>)>;
}

impl DivWorkerWithNames for riichi::algo::agari::DivWorker<'_> {
    fn search_yakus_with_names(&self) -> Option<(riichi::algo::agari::Agari, Vec<String>)> {
        let mut han = 0;
        let mut yakuman = 0;
        let mut names = vec![];

        let has_pinfu = self.menzen_shuntsu.len() == 4
            && !riichi::matches_tu8!(self.pair_tile, P | F | C)
            && self.pair_tile != self.sup.bakaze
            && self.pair_tile != self.sup.jikaze
            && self.menzen_shuntsu.iter().any(|&s| {
                let num = s % 9 + 1;
                num <= 6 && s == self.sup.winning_tile || num >= 2 && s + 2 == self.sup.winning_tile
            });

        if has_pinfu {
            han += 1;
            names.push("Pinfu".to_string());
        }
        if self.div.has_chitoi {
            han += 2;
            names.push("Chiitoitsu".to_string());
        }
        if self.div.has_ryanpeikou {
            han += 3;
            names.push("Ryanpeikou".to_string());
        }
        if self.div.has_chuuren {
            if matches!(self.sup.tehai[self.sup.winning_tile as usize], 2 | 4) {
                yakuman += 2;
                names.push("True-Nine-Gates".to_string());
            } else {
                yakuman += 1;
                names.push("Nine-Gates".to_string());
            }
        }

        let has_tanyao = if self.div.has_chitoi {
            self.chitoi_pairs().all(|t| {
                let kind = t / 9;
                let num = t % 9;
                kind < 3 && num > 0 && num < 8
            })
        } else {
            self.all_shuntsu().all(|s| {
                let num = s % 9;
                num > 0 && num < 6
            }) && self.all_kotsu_and_kantsu().chain(std::iter::once(self.pair_tile)).all(|k| {
                let kind = k / 9;
                let num = k % 9;
                kind < 3 && num > 0 && num < 8
            })
        };
        if has_tanyao {
            han += 1;
            names.push("Tanyao".to_string());
        }

        let has_toitoi = !self.div.has_chitoi && self.menzen_shuntsu.is_empty() && self.sup.chis.is_empty();
        if has_toitoi {
            han += 2;
            names.push("Toitoi".to_string());
        }

        let mut isou_kind = None;
        let mut has_jihai = false;
        let mut is_chinitsu_or_honitsu = true;
        let iter_fn = |&m: &u8| {
            let kind = m / 9;
            if kind >= 3 {
                has_jihai = true;
                return true;
            }
            if let Some(prev_kind) = isou_kind {
                if prev_kind != kind {
                    is_chinitsu_or_honitsu = false;
                    return false;
                }
            } else {
                isou_kind = Some(kind);
            }
            true
        };
        if self.div.has_chitoi {
            self.chitoi_pairs().take_while(iter_fn).for_each(drop);
        } else {
            self.all_mentsu()
                .chain(std::iter::once(self.pair_tile))
                .take_while(iter_fn)
                .for_each(drop);
        }
        if isou_kind.is_none() {
            yakuman += 1;
            names.push("All-Honors".to_string());
        } else if is_chinitsu_or_honitsu {
            let n = if has_jihai { 2 } else { 5 } + self.sup.is_menzen as u8;
            han += n;
            names.push(if has_jihai {
                "Honitsu".to_string()
            } else {
                "Chinitsu".to_string()
            });
        }

        if !self.div.has_chitoi {
            if self.div.has_ipeikou {
                han += 1;
                names.push("Iipeikou".to_string());
            } else if !self.sup.ankans.is_empty() && self.sup.is_menzen && self.menzen_shuntsu.len() >= 2 {
                let mut shuntsu_marks = [0_u8; 3];
                let has_ipeikou = self.menzen_shuntsu.iter().any(|&t| {
                    let kind = t as usize / 9;
                    let num = t % 9;
                    let mark = &mut shuntsu_marks[kind];
                    if (*mark >> num) & 0b1 == 0b1 {
                        true
                    } else {
                        *mark |= 0b1 << num;
                        false
                    }
                });
                if has_ipeikou {
                    han += 1;
                    names.push("Iipeikou".to_string());
                }
            }

            if self.sup.is_menzen && self.div.has_ittsuu {
                han += 2;
                names.push("Ittsuu".to_string());
            } else if self.sup.chis.is_empty() && self.div.has_ittsuu {
                han += 1;
                names.push("Ittsuu".to_string());
            } else if self.menzen_shuntsu.len() + self.sup.chis.len() >= 3 {
                let mut kinds = [0; 3];
                for s in self.all_shuntsu() {
                    let kind = s as usize / 9;
                    let num = s % 9;
                    match num {
                        0 => kinds[kind] |= 0b001,
                        3 => kinds[kind] |= 0b010,
                        6 => kinds[kind] |= 0b100,
                        _ => (),
                    };
                }
                if kinds.contains(&0b111) {
                    han += 1;
                    names.push("Ittsuu".to_string());
                }
            }

            let mut s_counter = [0; 9];
            for s in self.all_shuntsu() {
                let kind = s / 9;
                let num = s % 9;
                s_counter[num as usize] |= 0b1 << kind;
            }
            if s_counter.contains(&0b111) {
                let n = if self.sup.is_menzen { 2 } else { 1 };
                han += n;
                names.push("Sanshoku".to_string());
            } else {
                let mut k_counter = [0; 9];
                for k in self.all_kotsu_and_kantsu() {
                    let kind = k / 9;
                    if kind < 3 {
                        let num = k % 9;
                        k_counter[num as usize] |= 1 << kind;
                    }
                }
                if k_counter.contains(&0b111) {
                    han += 2;
                    names.push("Sanshoku-Doukou".to_string());
                }
            }

            let ankous_count = self.sup.ankans.len() + self.menzen_kotsu.len() - self.winning_tile_makes_minkou as usize;
            match ankous_count {
                4 => {
                    if self.sup.tehai[self.sup.winning_tile as usize] == 2 {
                        yakuman += 2;
                        names.push("Suuankou-Tanki".to_string());
                    } else {
                        yakuman += 1;
                        names.push("Suuankou".to_string());
                    }
                }
                3 => {
                    han += 2;
                    names.push("Sanankou".to_string());
                }
                _ => (),
            };

            let kans_count = self.sup.ankans.len() + self.sup.minkans.len();
            match kans_count {
                4 => {
                    yakuman += 1;
                    names.push("Suukantsu".to_string());
                }
                3 => {
                    han += 2;
                    names.push("Sankantsu".to_string());
                }
                _ => (),
            };

            let has_ryuisou = self
                .all_kotsu_and_kantsu()
                .chain(std::iter::once(self.pair_tile))
                .all(|k| riichi::matches_tu8!(k, 2s | 3s | 4s | 6s | 8s | F))
                && self.all_shuntsu().all(|s| s == riichi::tu8!(2s));
            if has_ryuisou {
                yakuman += 1;
                names.push("All-Green".to_string());
            }

            if !has_tanyao {
                let mut has_jihai = [false; 7];
                for k in self.all_kotsu_and_kantsu() {
                    if k >= 3 * 9 {
                        has_jihai[k as usize - 3 * 9] = true;
                    }
                }
                if has_jihai[self.sup.bakaze as usize - 3 * 9] {
                    han += 1;
                    names.push("Bakaze".to_string());
                }
                if has_jihai[self.sup.jikaze as usize - 3 * 9] {
                    han += 1;
                    names.push("Jikaze".to_string());
                }

                let saneins = (4..7).filter(|&i| has_jihai[i]).count() as u8;
                if saneins > 0 {
                    han += saneins;
                    names.push("Yakuhai".to_string());
                    if saneins == 3 {
                        yakuman += 1;
                        names.push("Daisangen".to_string());
                    } else if saneins == 2 && riichi::matches_tu8!(self.pair_tile, P | F | C) {
                        han += 2;
                        names.push("Shousangen".to_string());
                    }
                }

                let winds = (0..4).filter(|&i| has_jihai[i]).count();
                if winds == 4 {
                    yakuman += 2;
                    names.push("Daisuushii".to_string());
                } else if winds == 3 && riichi::matches_tu8!(self.pair_tile, E | S | W | N) {
                    yakuman += 1;
                    names.push("Shousuushii".to_string());
                }
            }
        }

        if !has_tanyao {
            let mut has_jihai = false;
            let is_yaokyuu = |k| {
                let kind = k / 9;
                if kind >= 3 {
                    has_jihai = true;
                    true
                } else {
                    let num = k % 9;
                    num == 0 || num == 8
                }
            };
            let is_junchan_or_chanta_or_chinroutou_or_honroutou = if self.div.has_chitoi {
                self.chitoi_pairs().all(is_yaokyuu)
            } else {
                self.all_kotsu_and_kantsu()
                    .chain(std::iter::once(self.pair_tile))
                    .all(is_yaokyuu)
            };
            if is_junchan_or_chanta_or_chinroutou_or_honroutou {
                if self.div.has_chitoi || has_toitoi {
                    if has_jihai {
                        han += 2;
                        names.push("All-Terminals-Honors".to_string());
                    } else {
                        yakuman += 1;
                        names.push("All-Terminals".to_string());
                    }
                } else {
                    let is_junchan_or_chanta = self.all_shuntsu().all(|s| {
                        let num = s % 9;
                        num == 0 || num == 6
                    });
                    if is_junchan_or_chanta {
                        let n = if has_jihai { 1 } else { 2 } + self.sup.is_menzen as u8;
                        han += n;
                        names.push(if has_jihai {
                            "Half-Outside".to_string()
                        } else {
                            "Fully-Outside".to_string()
                        });
                    }
                }
            }
        }

        if yakuman > 0 {
            Some((riichi::algo::agari::Agari::Yakuman(yakuman), names))
        } else if han > 0 {
            let fu = self.calc_fu(has_pinfu);
            Some((riichi::algo::agari::Agari::Normal { fu, han }, names))
        } else {
            None
        }
    }
}
