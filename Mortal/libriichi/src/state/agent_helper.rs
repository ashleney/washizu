use super::{PlayerState, SinglePlayerTables};
use crate::algo::agari::AgariCalculator;
use crate::algo::point::Point;
use crate::algo::shanten;
use crate::algo::sp::{InitState, SPCalculator};
use crate::tile::Tile;
use crate::vec_ops::vec_add_assign;
use crate::{must_tile, t, tu8, tuz};
use anyhow::{Context, Result, ensure};
use tinyvec::array_vec;
impl PlayerState {
    /// Used by `BoardState` to check if a player is making 4 kans on his own.
    #[inline]
    #[must_use]
    pub fn kans_count(&self) -> usize {
        self.minkans.len() + self.ankans.len()
    }
    /// Used by `Agent` impls, must be called at 3n+2.
    #[must_use]
    pub fn discard_candidates(&self) -> [bool; 34] {
        let full = self.discard_candidates_aka();
        let mut ret = [false; 34];
        ret.copy_from_slice(&full[..34]);
        ret[tuz!(5m)] |= full[tuz!(5mr)];
        ret[tuz!(5s)] |= full[tuz!(5sr)];
        ret[tuz!(5p)] |= full[tuz!(5pr)];
        ret
    }
    /// Aka dora covered version of `discard_candidates`.
    #[must_use]
    pub fn discard_candidates_aka(&self) -> [bool; 37] {
        assert!(self.last_cans.can_discard, "tehai is not 3n+2");
        let mut ret = [false; 37];
        if self.riichi_accepted[0] {
            let last_self_tsumo = self
                .last_self_tsumo
                .expect("riichi accepted without last self tsumo");
            ret[last_self_tsumo.as_usize()] = true;
            return ret;
        }
        for (i, count) in self.tehai.iter().copied().enumerate() {
            if count == 0 {
                continue;
            }
            ret[i] = if self.riichi_declared[0] {
                if self.shanten == 1 {
                    self.next_shanten_discards[i]
                } else {
                    self.keep_shanten_discards[i]
                }
            } else {
                !self.forbidden_tiles[i]
            };
        }
        if ret[tuz!(5m)] && self.akas_in_hand[0] {
            ret[tuz!(5mr)] = true;
            ret[tuz!(5m)] = self.tehai[tuz!(5m)] > 1;
        }
        if ret[tuz!(5p)] && self.akas_in_hand[1] {
            ret[tuz!(5pr)] = true;
            ret[tuz!(5p)] = self.tehai[tuz!(5p)] > 1;
        }
        if ret[tuz!(5s)] && self.akas_in_hand[2] {
            ret[tuz!(5sr)] = true;
            ret[tuz!(5s)] = self.tehai[tuz!(5s)] > 1;
        }
        ret
    }
    /// Must be called at 3n+2.
    ///
    /// The return value indicates the tiles which can make the hand tenpai for
    /// real after being discarded, with the number of future tenpai tiles left
    /// and furiten considered, without depending on any incidental yaku, and is
    /// not affected by the riichi status of the player.
    #[must_use]
    pub fn discard_candidates_with_unconditional_tenpai(&self) -> [bool; 34] {
        let full = self.discard_candidates_with_unconditional_tenpai_aka();
        let mut ret = [false; 34];
        ret.copy_from_slice(&full[..34]);
        ret[tuz!(5m)] |= full[tuz!(5mr)];
        ret[tuz!(5s)] |= full[tuz!(5sr)];
        ret[tuz!(5p)] |= full[tuz!(5pr)];
        ret
    }
    /// Aka dora covered version of `discard_candidates_with_unconditional_tenpai`.
    #[must_use]
    pub fn discard_candidates_with_unconditional_tenpai_aka(&self) -> [bool; 37] {
        assert!(self.last_cans.can_discard, "tehai is not 3n+2");
        let mut ret = [false; 37];
        if self.tiles_left == 0 || self.shanten > 1
            || self.shanten == 1 && !self.has_next_shanten_discard
        {
            return ret;
        }
        if let Some(last_self_tsumo) = self.last_self_tsumo {
            if self.waits[last_self_tsumo.deaka().as_usize()] {
                return ret;
            }
            if self.riichi_accepted[0] {
                if !self.at_furiten {
                    ret[last_self_tsumo.as_usize()] = true;
                }
                return ret;
            }
        } else if shanten::calc_all(&self.tehai, self.tehai_len_div3) == -1 {
            return ret;
        }
        let tenpai_discards = if self.shanten == 1 {
            self.next_shanten_discards
        } else {
            self.keep_shanten_discards
        };
        tenpai_discards
            .iter()
            .copied()
            .enumerate()
            .filter(|&(tid, b)| b && !self.forbidden_tiles[tid])
            .for_each(|(discard, _)| {
                let mut tehai_3n1 = self.tehai;
                tehai_3n1[discard] -= 1;
                for (tsumo, seen) in self.tiles_seen.iter().copied().enumerate() {
                    if tsumo == discard || tehai_3n1[tsumo] == 4 {
                        continue;
                    }
                    let mut tehai_3n2 = tehai_3n1;
                    tehai_3n2[tsumo] += 1;
                    if shanten::calc_all(&tehai_3n2, self.tehai_len_div3) > -1 {
                        continue;
                    }
                    if self.discarded_tiles[tsumo] {
                        ret[discard] = false;
                        break;
                    }
                    if seen == 4 || ret[discard] {
                        continue;
                    }
                    let agari_calc = AgariCalculator {
                        tehai: &tehai_3n2,
                        is_menzen: self.is_menzen,
                        chis: &self.chis,
                        pons: &self.pons,
                        minkans: &self.minkans,
                        ankans: &self.ankans,
                        bakaze: self.bakaze.as_u8(),
                        jikaze: self.jikaze.as_u8(),
                        winning_tile: tsumo as u8,
                        is_ron: true,
                    };
                    ret[discard] = agari_calc.has_yaku();
                }
            });
        if ret[tuz!(5m)] && self.akas_in_hand[0] {
            ret[tuz!(5mr)] = true;
            ret[tuz!(5m)] = self.tehai[tuz!(5m)] > 1;
        }
        if ret[tuz!(5p)] && self.akas_in_hand[1] {
            ret[tuz!(5pr)] = true;
            ret[tuz!(5p)] = self.tehai[tuz!(5p)] > 1;
        }
        if ret[tuz!(5s)] && self.akas_in_hand[2] {
            ret[tuz!(5sr)] = true;
            ret[tuz!(5s)] = self.tehai[tuz!(5s)] > 1;
        }
        ret
    }
    #[inline]
    #[must_use]
    pub fn yaokyuu_kind_count(&self) -> u8 {
        tuz![1m, 9m, 1p, 9p, 1s, 9s, E, S, W, N, P, F, C]
            .iter()
            .map(|&i| self.tehai[i].min(1))
            .sum()
    }
    #[inline]
    #[must_use]
    pub fn rule_based_ryukyoku(&self) -> bool {
        if !self.last_cans.can_ryukyoku {
            return false;
        }
        self.rule_based_ryukyoku_slow()
    }
    pub fn rule_based_ryukyoku_slow(&self) -> bool {
        if shanten::calc_all(&self.tehai, self.tehai_len_div3) <= 2 {
            return false;
        }
        if self.bakaze == t!(W) {
            return true;
        }
        if self.is_all_last {
            if self.oya == 0 || self.rank < 3 {
                return true;
            }
            let mut scores = [-3000 - self.honba as i32 * 300; 4];
            scores[0] = 12000 + self.kyotaku as i32 * 1000 + self.honba as i32 * 300;
            scores[self.oya as usize] = -6000 - self.honba as i32 * 300;
            vec_add_assign(&mut scores, &self.scores);
            return self.get_rank(scores) < 3;
        }
        if self.yaokyuu_kind_count() >= 10 {
            return false;
        }
        if self.tehai[3 * 9..].iter().all(|&c| c > 0) {
            return false;
        }
        true
    }
    #[inline]
    #[must_use]
    pub fn rule_based_agari(&self) -> bool {
        if !self.last_cans.can_agari() {
            return false;
        }
        self.rule_based_agari_slow(
            self.last_cans.can_ron_agari,
            self.rel(self.last_cans.target_actor),
        )
    }
    pub fn rule_based_agari_slow(&self, is_ron: bool, target_rel: usize) -> bool {
        if !self.is_all_last || self.oya == 0 || self.rank < 3 {
            return true;
        }
        if self.bakaze == t!(W) {
            if self.kyoku < 3 {
                return true;
            }
        } else if self.scores.iter().all(|&s| s < 30000) {
            return true;
        }
        let max_win_point = if self.riichi_accepted[0] {
            let mut tehai_full = self.tehai;
            for t in &self.ankan_overview[0] {
                tehai_full[t.as_usize()] += 4;
            }
            let mut tehai_ordered_by_count: Vec<_> = tehai_full
                .iter()
                .enumerate()
                .filter(|&(_, &c)| c > 0)
                .collect();
            tehai_ordered_by_count.sort_unstable_by(|(_, l), (_, r)| r.cmp(l));
            let mut tiles_seen = self.tiles_seen;
            let mut ura_indicators = array_vec!([_; 5]);
            'outer: for (t, _) in tehai_ordered_by_count {
                let ura_ind = must_tile!(t).prev();
                loop {
                    if ura_indicators.len() >= self.dora_indicators.len() {
                        break 'outer;
                    }
                    if tiles_seen[ura_ind.as_usize()] >= 4 {
                        continue 'outer;
                    }
                    ura_indicators.push(ura_ind);
                    tiles_seen[ura_ind.as_usize()] += 1;
                }
            }
            self.agari_points(is_ron, &ura_indicators).unwrap()
        } else {
            self.agari_points(is_ron, &[]).unwrap()
        };
        let mut exp_scores = self.scores;
        if is_ron {
            exp_scores[0]
                += max_win_point.ron + self.kyotaku as i32 * 1000
                    + self.honba as i32 * 300;
            exp_scores[target_rel] -= max_win_point.ron + self.honba as i32 * 300;
        } else {
            exp_scores[0]
                += max_win_point.tsumo_total(false) + self.kyotaku as i32 * 1000
                    + self.honba as i32 * 300;
            exp_scores
                .iter_mut()
                .enumerate()
                .skip(1)
                .for_each(|(idx, s)| {
                    if idx as u8 == self.oya {
                        *s -= max_win_point.tsumo_oya + self.honba as i32 * 100;
                    } else {
                        *s -= max_win_point.tsumo_ko + self.honba as i32 * 100;
                    }
                });
        }
        if exp_scores.iter().all(|&s| s < 30000) {
            return true;
        }
        self.get_rank(exp_scores) < 3
    }
    /// Err is returned if the hand cannot agari, or cannot retrieve the winning
    /// tile.
    ///
    /// This function should be called immediately, otherwise the state may
    /// change.
    ///
    /// `ura_indicators` is used only when the actor has an accepted riichi.
    pub fn agari_points(&self, is_ron: bool, ura_indicators: &[Tile]) -> Result<Point> {
        ensure!(
            is_ron && self.last_cans.can_ron_agari || self.last_cans.can_tsumo_agari,
            "cannot agari"
        );
        if !is_ron && self.can_w_riichi {
            return Ok(Point::yakuman(self.oya == 0, 1));
        }
        let winning_tile = if is_ron {
            self.last_kawa_tile
        } else {
            self.last_self_tsumo
        }
            .context("cannot find the winning tile")?;
        let additional_hans = if is_ron {
            [
                self.riichi_accepted[0],
                self.is_w_riichi,
                self.at_ippatsu,
                self.tiles_left == 0,
                self.chankan_chance.is_some(),
            ]
                .iter()
                .filter(|&&b| b)
                .count() as u8
        } else {
            [
                self.riichi_accepted[0],
                self.is_w_riichi,
                self.at_ippatsu,
                self.is_menzen,
                self.tiles_left == 0 && !self.at_rinshan,
                self.at_rinshan,
            ]
                .iter()
                .filter(|&&b| b)
                .count() as u8
        };
        let mut tehai = self.tehai;
        let mut final_doras_owned = self.doras_owned[0];
        if is_ron {
            let tid = winning_tile.deaka().as_usize();
            tehai[tid] += 1;
            final_doras_owned += self.dora_factor[tid];
            if winning_tile.is_aka() {
                final_doras_owned += 1;
            }
        }
        if self.riichi_accepted[0] {
            final_doras_owned
                += ura_indicators
                    .iter()
                    .map(|&ura| {
                        let next = ura.next();
                        let mut count = tehai[next.as_usize()];
                        if self.ankan_overview[0].contains(&next) {
                            count += 4;
                        }
                        count
                    })
                    .sum::<u8>();
        }
        let agari_calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: self.is_menzen,
            chis: &self.chis,
            pons: &self.pons,
            minkans: &self.minkans,
            ankans: &self.ankans,
            bakaze: self.bakaze.as_u8(),
            jikaze: self.jikaze.as_u8(),
            winning_tile: winning_tile.deaka().as_u8(),
            is_ron,
        };
        let agari = agari_calc
            .agari(additional_hans, final_doras_owned)
            .context("not a hora hand")?;
        Ok(agari.point(self.oya == 0))
    }
    /// Calculate the actual shanten at this point. Unlike `self.shanten`, this
    /// function properly calculates the shanten at 3n+2, which follows the
    /// definition of shanten most people acknowledge.
    pub fn real_time_shanten(&self) -> i8 {
        if !self.last_cans.can_discard {
            return self.shanten;
        }
        if self.shanten > 0 {
            return if self.has_next_shanten_discard {
                self.shanten - 1
            } else {
                self.shanten
            };
        }
        if let Some(tile) = self.last_self_tsumo {
            return if self.waits[tile.deaka().as_usize()] { -1 } else { 0 };
        }
        shanten::calc_all(&self.tehai, self.tehai_len_div3)
    }
    /// Can be called at both 3n+1 and 3n+2, but `self.real_time_shanten` must
    /// be >= 0 and `self.tiles_left` must be >= 4.
    ///
    /// This function is currently highly internal.
    pub fn single_player_tables(&self) -> Result<SinglePlayerTables> {
        ensure!(self.tiles_left >= 4, "need at least one more tsumo");
        let cur_shanten = self.real_time_shanten();
        ensure!(cur_shanten >= 0, "can't calculate an agari hand");
        let mut can_discard = self.last_cans.can_discard;
        let (tsumos_left, calc_haitei) = if can_discard {
            (self.tiles_left / 4, self.tiles_left % 4 == 0)
        } else {
            let target = self.rel(self.last_cans.target_actor) as u8;
            let tiles_left_at_next_tsumo = self.tiles_left.saturating_sub(4 - target);
            (tiles_left_at_next_tsumo / 4, tiles_left_at_next_tsumo % 4 == 0)
        };
        ensure!(tsumos_left >= 1, "need at least one more tsumo");
        let num_doras_in_fuuro = if self.is_menzen && self.ankan_overview[0].is_empty() {
            0
        } else {
            let num_doras_in_tehai: u8 = self
                .dora_indicators
                .iter()
                .map(|ind| self.tehai[ind.next().as_usize()])
                .sum();
            let num_akas = self.akas_in_hand.iter().filter(|&&b| b).count() as u8;
            self.doras_owned[0] - num_doras_in_tehai - num_akas
        };
        let prefer_riichi = self.scores[0] >= 1000;
        let calc_double_riichi = can_discard && self.can_w_riichi;
        let mut tehai = self.tehai;
        let mut akas_in_hand = self.akas_in_hand;
        let is_discard_after_riichi = can_discard && self.riichi_accepted[0];
        if is_discard_after_riichi {
            let last_tsumo = self.last_self_tsumo.unwrap();
            tehai[last_tsumo.deaka().as_usize()] -= 1;
            match last_tsumo.as_u8() {
                tu8!(5mr) => akas_in_hand[0] = false,
                tu8!(5pr) => akas_in_hand[1] = false,
                tu8!(5sr) => akas_in_hand[2] = false,
                _ => {}
            }
            can_discard = false;
        }
        let init_state = InitState {
            tehai,
            akas_in_hand,
            tiles_seen: self.tiles_seen,
            akas_seen: self.akas_seen,
        };
        let sp_calc = SPCalculator {
            tehai_len_div3: self.tehai_len_div3,
            is_menzen: self.is_menzen,
            chis: &self.chis,
            pons: &self.pons,
            minkans: &self.minkans,
            ankans: &self.ankans,
            bakaze: self.bakaze.as_u8(),
            jikaze: self.jikaze.as_u8(),
            num_doras_in_fuuro,
            prefer_riichi,
            dora_indicators: &self.dora_indicators,
            calc_double_riichi,
            calc_haitei,
            sort_result: true,
            maximize_win_prob: false,
            calc_tegawari: false,
            calc_shanten_down: false,
        };
        let mut max_ev_table = sp_calc
            .calc(init_state, can_discard, tsumos_left, cur_shanten)?;
        if is_discard_after_riichi {
            max_ev_table[0].tile = self.last_self_tsumo.unwrap();
        }
        Ok(SinglePlayerTables { max_ev_table })
    }
}
