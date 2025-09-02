//! Agari calculation compatibility layer

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
