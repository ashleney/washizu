//! Rust port of EndlessCheng's Go port of 山岡忠夫's Java implementation of his
//! agari algorithm.
//!
//! Source:
//! * Go: <https://github.com/EndlessCheng/mahjong-helper/blob/master/util/agari.go>
//! * Java: <http://hp.vector.co.jp/authors/VA046927/mjscore/AgariIndex.java>
//! * Algorithm: <http://hp.vector.co.jp/authors/VA046927/mjscore/mjalgorism.html>
use super::point::Point;
use super::shanten;
use crate::tile::Tile;
use crate::{matches_tu8, must_tile, tu8};
use std::cmp::Ordering;
use std::iter;
use std::sync::LazyLock;
use boomphf::hashmap::BoomHashMap;
use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::GzDecoder;
use tinyvec::ArrayVec;
const AGARI_TABLE_SIZE: usize = 9_362;
pub static AGARI_TABLE: LazyLock<BoomHashMap<u32, ArrayVec<[Div; 4]>>> = LazyLock::new(||
{
    let mut raw = GzDecoder::new(include_bytes!("data/agari.bin.gz").as_slice());
    let (keys, values): (Vec<_>, Vec<_>) = (0..AGARI_TABLE_SIZE)
        .map(|_| {
            let key = raw.read_u32::<LittleEndian>().unwrap();
            let v_size = raw.read_u8().unwrap();
            let value = (0..v_size)
                .map(|_| raw.read_u32::<LittleEndian>().unwrap())
                .map(Div::from)
                .collect();
            (key, value)
        })
        .unzip();
    if cfg!(test) {
        let mut k = keys.clone();
        k.sort_unstable();
        k.dedup();
        assert_eq!(k.len(), keys.len());
        raw.read_u8().unwrap_err();
    }
    BoomHashMap::new(keys, values)
});
#[derive(Debug, Default)]
pub struct Div {
    pub pair_idx: u8,
    pub kotsu_idxs: ArrayVec<[u8; 4]>,
    pub shuntsu_idxs: ArrayVec<[u8; 4]>,
    pub has_chitoi: bool,
    pub has_chuuren: bool,
    pub has_ittsuu: bool,
    pub has_ryanpeikou: bool,
    pub has_ipeikou: bool,
}
#[derive(Debug, Clone, Copy, Eq)]
pub enum Agari {
    /// `fu` may be 0 if `han` is greater than 4.
    Normal { fu: u8, han: u8 },
    Yakuman(u8),
}
#[derive(Debug)]
pub struct AgariCalculator<'a> {
    /// Must include the winning tile (i.e. must be 3n+2)
    pub tehai: &'a [u8; 34],
    /// `self.chis.is_empty() && self.pons.is_empty() && self.minkans.is_empty()`
    pub is_menzen: bool,
    pub chis: &'a [u8],
    pub pons: &'a [u8],
    pub minkans: &'a [u8],
    pub ankans: &'a [u8],
    pub bakaze: u8,
    pub jikaze: u8,
    /// Must be deakaized
    pub winning_tile: u8,
    /// For consistency reasons, `is_ron` is only used to calculate fu and check
    /// ankou/ankan-related yakus like 三/四暗刻. It will not be used to
    /// determine 門前清自摸和.
    pub is_ron: bool,
}
pub struct DivWorker<'a> {
    pub sup: &'a AgariCalculator<'a>,
    pub tile14: &'a [u8; 14],
    pub div: &'a Div,
    pub pair_tile: u8,
    pub menzen_kotsu: ArrayVec<[u8; 4]>,
    pub menzen_shuntsu: ArrayVec<[u8; 4]>,
    /// Used in fu calc and sanankou condition, indicating whether or not the
    /// winning tile should build a minkou instead of shuntsu in an ambiguous
    /// pattern.
    ///
    /// The winning tile should try its best to fit into a shuntsu, because that
    /// always gives a higher score than using that winning tile to turn an
    /// existing ankou into a minkou, because a shuntsu can only add at most 2
    /// fu (penchan or kanchan) and does not bring extra yaku (except for pinfu,
    /// but since we have ankou it can never be pinfu), but an ankou adds at
    /// least 2 fu and can bring extra yakus like sanankou.
    ///
    /// An example of this is 45556 + 5, which could be either 456 + (55 + 5) or
    /// 555 + (46 + 5). `menzen_kotsu` will contain 555 while `menzen_shuntsu`
    /// will also contain 456, making it ambiguous whether the winning tile 5
    /// should be a part of either the minkou 55 + 5 or the shuntsu 46 + 5. In
    /// practice, the latter should be preferred because it preserves the ankou.
    /// A test case covers this.
    pub winning_tile_makes_minkou: bool,
}
impl From<u32> for Div {
    fn from(v: u32) -> Self {
        let pair_idx = ((v >> 6) & 0b1111) as u8;
        let kotsu_count = v & 0b111;
        let kotsu_idxs = (0..kotsu_count)
            .map(|i| ((v >> (10 + i * 4)) & 0b1111) as u8)
            .collect();
        let shuntsu_count = (v >> 3) & 0b111;
        let shuntsu_idxs = (kotsu_count..kotsu_count + shuntsu_count)
            .map(|i| ((v >> (10 + i * 4)) & 0b1111) as u8)
            .collect();
        let has_chitoi = (v >> 26) & 0b1 == 0b1;
        let has_chuuren = (v >> 27) & 0b1 == 0b1;
        let has_ittsuu = (v >> 28) & 0b1 == 0b1;
        let has_ryanpeikou = (v >> 29) & 0b1 == 0b1;
        let has_ipeikou = (v >> 30) & 0b1 == 0b1;
        Self {
            pair_idx,
            kotsu_idxs,
            shuntsu_idxs,
            has_chitoi,
            has_chuuren,
            has_ittsuu,
            has_ryanpeikou,
            has_ipeikou,
        }
    }
}
impl PartialEq for Agari {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Yakuman(l), Self::Yakuman(r)) => l == r,
            (Self::Normal { fu: lf, han: lh }, Self::Normal { fu: rf, han: rh }) => {
                lf == rf && lh == rh
            }
            _ => false,
        }
    }
}
impl PartialOrd for Agari {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Agari {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Yakuman(l), Self::Yakuman(r)) => l.cmp(r),
            (Self::Yakuman(_), Self::Normal { .. }) => Ordering::Greater,
            (Self::Normal { .. }, Self::Yakuman(..)) => Ordering::Less,
            (Self::Normal { fu: lf, han: lh }, Self::Normal { fu: rf, han: rh }) => {
                match lh.cmp(rh) {
                    Ordering::Equal => lf.cmp(rf),
                    v => v,
                }
            }
        }
    }
}
impl Agari {
    #[must_use]
    pub fn point(self, is_oya: bool) -> Point {
        match self {
            Self::Normal { fu, han } => Point::calc(is_oya, fu, han),
            Self::Yakuman(n) => Point::yakuman(is_oya, n as i32),
        }
    }
}
impl AgariCalculator<'_> {
    #[inline]
    #[must_use]
    pub fn has_yaku(&self) -> bool {
        self.search_yakus_impl(true).is_some()
    }
    #[inline]
    #[must_use]
    pub fn search_yakus(&self) -> Option<Agari> {
        self.search_yakus_impl(false)
    }
    /// `additional_hans` includes 門前清自摸和, (両)立直, 槍槓, 嶺上開花, 海底
    /// 摸月 and 河底撈魚. 天和 and 地和 are supposed to be checked somewhere
    /// else other than here.
    ///
    /// `None` is returned iff `!self.has_yaku() && additional_hans == 0` holds.
    ///
    /// This function is only supposed to be called by callers who have the
    /// knowledge of the ura doras.
    #[must_use]
    pub fn agari(&self, additional_hans: u8, doras: u8) -> Option<Agari> {
        if let Some(agari) = self.search_yakus() {
            Some(
                match agari {
                    Agari::Normal { fu, han } => {
                        Agari::Normal {
                            fu,
                            han: han + additional_hans + doras,
                        }
                    }
                    _ => agari,
                },
            )
        } else if additional_hans == 0 {
            None
        } else if additional_hans + doras >= 5 {
            Some(Agari::Normal {
                fu: 0,
                han: additional_hans + doras,
            })
        } else {
            let (tile14, key) = get_tile14_and_key(self.tehai);
            let divs = AGARI_TABLE.get(&key)?;
            let fu = divs
                .iter()
                .map(|div| DivWorker::new(self, &tile14, div))
                .map(|w| w.calc_fu(false))
                .max()?;
            Some(Agari::Normal {
                fu,
                han: additional_hans + doras,
            })
        }
    }
    pub fn search_yakus_impl(&self, return_if_any: bool) -> Option<Agari> {
        assert_eq!(
            self.is_menzen, self.chis.is_empty() && self.pons.is_empty() && self.minkans
            .is_empty(),
        );
        if self.is_menzen && shanten::calc_kokushi(self.tehai) == -1 {
            return Some(Agari::Yakuman(1));
        }
        let (tile14, key) = get_tile14_and_key(self.tehai);
        let divs = AGARI_TABLE.get(&key)?;
        if return_if_any {
            divs.iter()
                .map(|div| DivWorker::new(self, &tile14, div))
                .find_map(|w| w.search_yakus::<true>())
        } else {
            divs.iter()
                .map(|div| DivWorker::new(self, &tile14, div))
                .filter_map(|w| w.search_yakus::<false>())
                .max()
        }
    }
}
impl<'a> DivWorker<'a> {
    pub fn new(
        calc: &'a AgariCalculator<'a>,
        tile14: &'a [u8; 14],
        div: &'a Div,
    ) -> Self {
        let pair_tile = tile14[div.pair_idx as usize];
        let menzen_kotsu = div
            .kotsu_idxs
            .iter()
            .map(|&idx| tile14[idx as usize])
            .collect();
        let menzen_shuntsu = div
            .shuntsu_idxs
            .iter()
            .map(|&idx| tile14[idx as usize])
            .collect();
        let mut ret = Self {
            sup: calc,
            tile14,
            div,
            pair_tile,
            menzen_kotsu,
            menzen_shuntsu,
            winning_tile_makes_minkou: false,
        };
        ret.winning_tile_makes_minkou = ret.winning_tile_makes_minkou();
        ret
    }
    /// For init only.
    pub fn winning_tile_makes_minkou(&self) -> bool {
        if !self.sup.is_ron {
            return false;
        }
        if !self.menzen_kotsu.contains(&self.sup.winning_tile) {
            return false;
        }
        if self.sup.winning_tile >= 3 * 9 {
            return true;
        }
        let kind = self.sup.winning_tile / 9;
        let num = self.sup.winning_tile % 9;
        let low = kind * 9 + num.saturating_sub(2);
        let high = kind * 9 + num.min(6);
        !(low..=high).any(|t| self.menzen_shuntsu.contains(&t))
    }
    /// The caller must assure `self.div.has_chitoi` holds.
    pub fn chitoi_pairs(&self) -> impl Iterator<Item = u8> + '_ {
        self.tile14.iter().take(7).copied()
    }
    pub fn all_kotsu_and_kantsu(&self) -> impl Iterator<Item = u8> + '_ {
        self.menzen_kotsu
            .iter()
            .chain(self.sup.pons)
            .chain(self.sup.minkans)
            .chain(self.sup.ankans)
            .copied()
    }
    pub fn all_shuntsu(&self) -> impl Iterator<Item = u8> + '_ {
        self.menzen_shuntsu.iter().chain(self.sup.chis).copied()
    }
    pub fn all_mentsu(&self) -> impl Iterator<Item = u8> + '_ {
        self.all_kotsu_and_kantsu().chain(self.all_shuntsu())
    }
    pub fn calc_fu(&self, has_pinfu: bool) -> u8 {
        if self.div.has_chitoi {
            return 25;
        }
        let mut fu = 20;
        fu
            += self
                .menzen_kotsu
                .iter()
                .map(|&t| {
                    let is_minkou = self.winning_tile_makes_minkou
                        && t == self.sup.winning_tile;
                    match (is_minkou, must_tile!(t).is_yaokyuu()) {
                        (false, true) => 8,
                        (false, false) | (true, true) => 4,
                        (true, false) => 2,
                    }
                })
                .sum::<u8>();
        fu
            += self
                .sup
                .pons
                .iter()
                .map(|&t| if must_tile!(t).is_yaokyuu() { 4 } else { 2 })
                .sum::<u8>();
        fu
            += self
                .sup
                .ankans
                .iter()
                .map(|&t| if must_tile!(t).is_yaokyuu() { 32 } else { 16 })
                .sum::<u8>();
        fu
            += self
                .sup
                .minkans
                .iter()
                .map(|&t| if must_tile!(t).is_yaokyuu() { 16 } else { 8 })
                .sum::<u8>();
        if matches_tu8!(self.pair_tile, P | F | C) {
            fu += 2;
        } else {
            if self.pair_tile == self.sup.bakaze {
                fu += 2;
            }
            if self.pair_tile == self.sup.jikaze {
                fu += 2;
            }
        }
        if fu == 20 {
            return if !self.sup.is_menzen {
                30
            } else if has_pinfu {
                if self.sup.is_ron { 30 } else { 20 }
            } else if self.sup.is_ron {
                40
            } else {
                30
            };
        }
        if !self.sup.is_ron {
            fu += 2;
        } else if self.sup.is_menzen {
            fu += 10;
        }
        if !self.winning_tile_makes_minkou {
            if self.pair_tile == self.sup.winning_tile {
                fu += 2;
            } else {
                let is_kanchan_penchan = self
                    .menzen_shuntsu
                    .iter()
                    .any(|&s| {
                        s + 1 == self.sup.winning_tile
                            || s % 9 == 0 && s + 2 == self.sup.winning_tile
                            || s % 9 == 6 && s == self.sup.winning_tile
                    });
                if is_kanchan_penchan {
                    fu += 2;
                }
            }
        }
        ((fu - 1) / 10 + 1) * 10
    }
    pub fn search_yakus<const RETURN_IF_ANY: bool>(&self) -> Option<Agari> {
        let mut han = 0;
        let mut yakuman = 0;
        let has_pinfu = self.menzen_shuntsu.len() == 4
            && !matches_tu8!(self.pair_tile, P | F | C)
            && self.pair_tile != self.sup.bakaze && self.pair_tile != self.sup.jikaze
            && self
                .menzen_shuntsu
                .iter()
                .any(|&s| {
                    let num = s % 9 + 1;
                    num <= 6 && s == self.sup.winning_tile
                        || num >= 2 && s + 2 == self.sup.winning_tile
                });
        macro_rules! make_return {
            () => {
                return if yakuman > 0 { Some(Agari::Yakuman(yakuman)) } else if han > 0 {
                let fu = if RETURN_IF_ANY || han >= 5 { 0 } else { self
                .calc_fu(has_pinfu) }; Some(Agari::Normal { fu, han }) } else { None };
            };
        }
        macro_rules! check_early_return {
            ($($block:tt)*) => {
                { $($block)*; if RETURN_IF_ANY { make_return!(); } }
            };
        }
        if has_pinfu {
            check_early_return! {
                han += 1
            };
        }
        if self.div.has_chitoi {
            check_early_return! {
                han += 2
            };
        }
        if self.div.has_ryanpeikou {
            check_early_return! {
                han += 3
            };
        }
        if self.div.has_chuuren {
            check_early_return! {
                yakuman += 1
            };
        }
        let has_tanyao = if self.div.has_chitoi {
            self.chitoi_pairs()
                .all(|t| {
                    let kind = t / 9;
                    let num = t % 9;
                    kind < 3 && num > 0 && num < 8
                })
        } else {
            self
                .all_shuntsu()
                .all(|s| {
                    let num = s % 9;
                    num > 0 && num < 6
                })
                && self
                    .all_kotsu_and_kantsu()
                    .chain(iter::once(self.pair_tile))
                    .all(|k| {
                        let kind = k / 9;
                        let num = k % 9;
                        kind < 3 && num > 0 && num < 8
                    })
        };
        if has_tanyao {
            check_early_return! {
                han += 1
            };
        }
        let has_toitoi = !self.div.has_chitoi && self.menzen_shuntsu.is_empty()
            && self.sup.chis.is_empty();
        if has_toitoi {
            check_early_return! {
                han += 2
            };
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
                .chain(iter::once(self.pair_tile))
                .take_while(iter_fn)
                .for_each(drop);
        }
        if isou_kind.is_none() {
            check_early_return! {
                yakuman += 1
            };
        } else if is_chinitsu_or_honitsu {
            let n = if has_jihai { 2 } else { 5 } + self.sup.is_menzen as u8;
            check_early_return! {
                han += n
            };
        }
        if !self.div.has_chitoi {
            if self.div.has_ipeikou {
                check_early_return! {
                    han += 1
                };
            } else if !self.sup.ankans.is_empty() && self.sup.is_menzen
                && self.menzen_shuntsu.len() >= 2
            {
                let mut shuntsu_marks = [0_u8; 3];
                let has_ipeikou = self
                    .menzen_shuntsu
                    .iter()
                    .any(|&t| {
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
                    check_early_return! {
                        han += 1
                    };
                }
            }
            if self.sup.is_menzen && self.div.has_ittsuu {
                check_early_return! {
                    han += 2
                };
            } else if self.sup.chis.is_empty() && self.div.has_ittsuu {
                check_early_return! {
                    han += 1
                };
            } else if self.menzen_shuntsu.len() + self.sup.chis.len() >= 3 {
                let mut kinds = [0; 3];
                for s in self.all_shuntsu() {
                    let kind = s as usize / 9;
                    let num = s % 9;
                    match num {
                        0 => kinds[kind] |= 0b001,
                        3 => kinds[kind] |= 0b010,
                        6 => kinds[kind] |= 0b100,
                        _ => {}
                    };
                }
                if kinds.contains(&0b111) {
                    check_early_return! {
                        han += 1
                    };
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
                check_early_return! {
                    han += n
                };
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
                    check_early_return! {
                        han += 2
                    };
                }
            }
            let ankous_count = self.sup.ankans.len() + self.menzen_kotsu.len()
                - self.winning_tile_makes_minkou as usize;
            match ankous_count {
                4 => {
                    check_early_return! {
                        yakuman += 1
                    }
                }
                3 => {
                    check_early_return! {
                        han += 2
                    }
                }
                _ => {}
            };
            let kans_count = self.sup.ankans.len() + self.sup.minkans.len();
            match kans_count {
                4 => {
                    check_early_return! {
                        yakuman += 1
                    }
                }
                3 => {
                    check_early_return! {
                        han += 2
                    }
                }
                _ => {}
            };
            let has_ryuisou = self
                .all_kotsu_and_kantsu()
                .chain(iter::once(self.pair_tile))
                .all(|k| matches_tu8!(k, 2s | 3s | 4s | 6s | 8s | F))
                && self.all_shuntsu().all(|s| s == tu8!(2s));
            if has_ryuisou {
                check_early_return! {
                    yakuman += 1
                };
            }
            if !has_tanyao {
                let mut has_jihai = [false; 7];
                for k in self.all_kotsu_and_kantsu() {
                    if k >= 3 * 9 {
                        has_jihai[k as usize - 3 * 9] = true;
                    }
                }
                if has_jihai[self.sup.bakaze as usize - 3 * 9] {
                    check_early_return! {
                        han += 1
                    };
                }
                if has_jihai[self.sup.jikaze as usize - 3 * 9] {
                    check_early_return! {
                        han += 1
                    };
                }
                let saneins = (4..7).filter(|&i| has_jihai[i]).count() as u8;
                if saneins > 0 {
                    check_early_return! {
                        han += saneins
                    };
                    if saneins == 3 {
                        check_early_return! {
                            yakuman += 1
                        };
                    } else if saneins == 2 && matches_tu8!(self.pair_tile, P | F | C) {
                        check_early_return! {
                            han += 2
                        };
                    }
                }
                let winds = (0..4).filter(|&i| has_jihai[i]).count();
                #[allow(clippy::if_same_then_else)]
                if winds == 4 {
                    check_early_return! {
                        yakuman += 1
                    };
                } else if winds == 3 && matches_tu8!(self.pair_tile, E | S | W | N) {
                    check_early_return! {
                        yakuman += 1
                    };
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
            let is_junchan_or_chanta_or_chinroutou_or_honroutou = if self.div.has_chitoi
            {
                self.chitoi_pairs().all(is_yaokyuu)
            } else {
                self.all_kotsu_and_kantsu()
                    .chain(iter::once(self.pair_tile))
                    .all(is_yaokyuu)
            };
            if is_junchan_or_chanta_or_chinroutou_or_honroutou {
                if self.div.has_chitoi || has_toitoi {
                    if has_jihai {
                        check_early_return! {
                            han += 2
                        };
                    } else {
                        check_early_return! {
                            yakuman += 1
                        };
                    }
                } else {
                    let is_junchan_or_chanta = self
                        .all_shuntsu()
                        .all(|s| {
                            let num = s % 9;
                            num == 0 || num == 6
                        });
                    if is_junchan_or_chanta {
                        let n = if has_jihai { 1 } else { 2 } + self.sup.is_menzen as u8;
                        check_early_return! {
                            han += n
                        };
                    }
                }
            }
        }
        make_return!();
    }
}
pub fn ensure_init() {
    assert_eq!(AGARI_TABLE.len(), AGARI_TABLE_SIZE);
}
pub fn get_tile14_and_key(tiles: &[u8; 34]) -> ([u8; 14], u32) {
    let mut tile14 = [0; 14];
    let mut tile14_iter = tile14.iter_mut();
    let mut key = 0;
    let mut bit_idx = -1;
    let mut prev_in_hand = None;
    for (kind, chunk) in tiles.chunks_exact(9).enumerate() {
        for (num, c) in chunk.iter().copied().enumerate() {
            if c > 0 {
                prev_in_hand = Some(());
                *tile14_iter.next().unwrap() = (kind * 9 + num) as u8;
                bit_idx += 1;
                match c {
                    2 => {
                        key |= 0b11 << bit_idx;
                        bit_idx += 2;
                    }
                    3 => {
                        key |= 0b1111 << bit_idx;
                        bit_idx += 4;
                    }
                    4 => {
                        key |= 0b11_1111 << bit_idx;
                        bit_idx += 6;
                    }
                    _ => {}
                }
            } else if prev_in_hand.take().is_some() {
                key |= 0b1 << bit_idx;
                bit_idx += 1;
            }
        }
        if prev_in_hand.take().is_some() {
            key |= 0b1 << bit_idx;
            bit_idx += 1;
        }
    }
    tiles
        .iter()
        .enumerate()
        .skip(3 * 9)
        .filter(|&(_, &c)| c > 0)
        .for_each(|(tile_id, &c)| {
            *tile14_iter.next().unwrap() = tile_id as u8;
            bit_idx += 1;
            match c {
                2 => {
                    key |= 0b11 << bit_idx;
                    bit_idx += 2;
                }
                3 => {
                    key |= 0b1111 << bit_idx;
                    bit_idx += 4;
                }
                4 => {
                    key |= 0b11_1111 << bit_idx;
                    bit_idx += 6;
                }
                _ => {}
            }
            key |= 0b1 << bit_idx;
            bit_idx += 1;
        });
    (tile14, key)
}
/// `tehai` must already contain `tile`. `true` is returned if making an ankan
/// with the tile is legal under the riichi'd `tehai`.
///
/// If `strict` is `false`, it is the same as [Tenhou's
/// rule](https://tenhou.net/man/#RULE):
///
/// > リーチ後の暗槓は待ちが変わらない場合のみ。送り槓不可、牌姿や役の増減は不
/// > 問。
///
/// If `strict` is `true`, it will also check the shape of tenpai and agari, but
/// will not check yaku anyways.
///
/// The behavior is undefined if `tehai` is not tenpai.
#[must_use]
pub fn check_ankan_after_riichi(
    tehai: &[u8; 34],
    len_div3: u8,
    tile: Tile,
    strict: bool,
) -> bool {
    let tile_id = tile.deaka().as_usize();
    if tehai[tile_id] != 4 {
        return false;
    }
    if tile_id >= 3 * 9 {
        return true;
    }
    let mut tehai_before_tsumo = *tehai;
    tehai_before_tsumo[tile_id] -= 1;
    (0..34)
        .filter(|&t| {
            if tehai_before_tsumo[t] == 4 {
                return false;
            }
            let mut tmp = tehai_before_tsumo;
            tmp[t] += 1;
            shanten::calc_all(&tmp, len_div3) == -1
        })
        .all(|wait| {
            if wait == tile_id {
                return false;
            }
            let mut tehai_after = *tehai;
            tehai_after[tile_id] = 0;
            tehai_after[wait] += 1;
            let (_, key) = get_tile14_and_key(&tehai_after);
            let Some(divs_after) = AGARI_TABLE.get(&key) else {
                return false;
            };
            if strict {
                let mut tehai_before = tehai_before_tsumo;
                tehai_before[wait] += 1;
                let (_, key) = get_tile14_and_key(&tehai_before);
                let divs_before = AGARI_TABLE
                    .get(&key)
                    .expect("invalid riichi detected when testing ankan after riichi");
                if divs_after.len() != divs_before.len() {
                    return false;
                }
            }
            true
        })
}
#[cfg(test)]
pub mod test {
    use super::*;
    use crate::hand::hand;
    #[test]
    pub fn ankan_after_riichi() {
        let test_one = |tehai_str, tile_str: &str, len_div3, strict, expected| {
            let mut tehai = hand(tehai_str).unwrap();
            let tile: Tile = tile_str.parse().unwrap();
            tehai[tile.as_usize()] += 1;
            assert_eq!(
                check_ankan_after_riichi(& tehai, len_div3, tile, strict), expected,
                "failed for {tehai_str} + {tile_str}, expected {expected}",
            );
        };
        test_one("12345m 567s 11222z", "S", 4, true, true);
        test_one("12345m 444567s 11z", "4s", 4, true, true);
        test_one("22m 11112356p 444s", "4s", 4, true, true);
        test_one("123456m 4445s 111z", "4s", 4, true, false);
        test_one("123456m 4445s 111z", "4s", 4, false, false);
        test_one("1113444p 222z", "1p", 3, true, false);
        test_one("1113444p 222z", "1p", 3, false, true);
        test_one("1113444p 222z", "4p", 3, true, false);
        test_one("1113444p 222z", "S", 3, true, true);
        test_one("23m 999p 33345666s", "3s", 4, true, false);
        test_one("23m 999p 33345666s", "6s", 4, true, false);
        test_one("23m 999p 33345666s", "6s", 4, false, true);
        test_one("23m 999p 33345666s", "9p", 4, true, true);
        test_one("1113445678999m", "1m", 4, true, true);
        test_one("1113445678999m", "9m", 4, true, false);
    }
    #[test]
    pub fn agari_calc() {
        let tehai = hand("2234455m 234p 234s 3m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(S),
            winning_tile: tu8!(3m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 40, han : 4 });
        let tehai = hand("12334m 345p 22s 777z 2m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(3m),
            is_ron: false,
        };
        let points = calc.agari(2, 0).unwrap().point(true);
        assert_eq!(points, Point { ron : 7700, tsumo_oya : 0, tsumo_ko : 2600 });
        let tehai = hand("2255m 445p 667788s 5p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(S),
            winning_tile: tu8!(5p),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 25, han : 3 });
        assert_eq!(yaku.point(false).ron, 3200);
        let tehai = hand("22334m 33p 4m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &tu8![2s, 2s],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(S),
            winning_tile: tu8!(4m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 30, han : 1 });
        let tehai = hand("223344p 667788s 3m 3m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(S),
            jikaze: tu8!(N),
            winning_tile: tu8!(3m),
            is_ron: false,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 30, han : 4 });
        let tehai = hand("234678m 1123488p 8p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(8p),
            is_ron: true,
        };
        assert_eq!(calc.search_yakus(), None);
        let tehai = hand("223344999m 1188p 8p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(8p),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 40, han : 1 });
        let tehai = hand("223344m 1188p 8p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &tu8![9m,],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(8p),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 70, han : 1 });
        let tehai = hand("55566677m 11p 7m").unwrap();
        let mut calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &tu8![9s,],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(7m),
            is_ron: false,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Yakuman(1));
        calc.is_ron = true;
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 80, han : 4 });
        let tehai = hand("666677778888m 99p").unwrap();
        let mut calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(8m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 30, han : 4 });
        calc.winning_tile = tu8!(7m);
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 40, han : 3 });
        let tehai = hand("12345678m 11p 9m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &tu8![9p,],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(9m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 70, han : 2 });
        let tehai = hand("12345678m 11p 9m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &[],
            pons: &tu8![9p,],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(9m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 30, han : 1 });
        let tehai = hand("111222333m 67p 88s 8p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(8p),
            is_ron: false,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 40, han : 2 });
        let tehai = hand("1112223334447z 7z").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(C),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Yakuman(3));
        let tehai = hand("1m 789p 789s 1m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &tu8![7m, 1s],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(1m),
            is_ron: false,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 30, han : 3 });
        let tehai = hand("111444m 45556s 22z 5s").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(S),
            jikaze: tu8!(S),
            winning_tile: tu8!(5s),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 60, han : 2 });
        let tehai = hand("999s 1777z 1z").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &tu8![1p,],
            pons: &tu8![N,],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(S),
            jikaze: tu8!(S),
            winning_tile: tu8!(E),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 50, han : 2 });
        let tehai = hand("1119m 9m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &[],
            pons: &tu8![S, C],
            minkans: &[],
            ankans: &tu8![N,],
            bakaze: tu8!(S),
            jikaze: tu8!(N),
            winning_tile: tu8!(9m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert!(matches!(yaku, Agari::Normal { han : 9, .. }));
        let (tile14, key) = get_tile14_and_key(&tehai);
        let divs = AGARI_TABLE.get(&key).unwrap();
        let fu = divs
            .iter()
            .map(|div| DivWorker::new(&calc, &tile14, div))
            .map(|w| w.calc_fu(false))
            .max()
            .unwrap();
        assert_eq!(fu, 70);
        let tehai = hand("1233334567888m 9m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(9m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert!(matches!(yaku, Agari::Normal { han : 8, .. }));
        let tehai = hand("2344445666678p 5p").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(5p),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert!(matches!(yaku, Agari::Normal { han : 7, .. }));
        let tehai = hand("2223445566s 1s").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: false,
            chis: &tu8![7s,],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(1s),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert!(matches!(yaku, Agari::Normal { han : 6, .. }));
        let tehai = hand("1123444m 111p 111s 1m").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(E),
            jikaze: tu8!(E),
            winning_tile: tu8!(1m),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert_eq!(yaku, Agari::Normal { fu : 60, han : 2 });
        let tehai = hand("111s 2225556677z 7z").unwrap();
        let calc = AgariCalculator {
            tehai: &tehai,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: tu8!(S),
            jikaze: tu8!(S),
            winning_tile: tu8!(C),
            is_ron: true,
        };
        let yaku = calc.search_yakus().unwrap();
        assert!(matches!(yaku, Agari::Normal { han : 15, .. }));
    }
}


/// Calculate the agari of a given winning tile, assuming no ura-dora.
pub fn calculate_agari_with_names(
    state: &crate::state::PlayerState,
    winning_tile: crate::tile::Tile,
    is_ron: bool,
) -> Option<(crate::algo::agari::Agari, Vec<String>)> {
    if !is_ron && state.can_w_riichi {
        return Some((
            crate::algo::agari::Agari::Yakuman(1),
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

    let agari_calc = crate::algo::agari::AgariCalculator {
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
    fn agari_with_names(&self, additional_hans: u8, doras: u8) -> Option<(crate::algo::agari::Agari, Vec<String>)>;
    fn search_yakus_with_names(&self) -> Option<(crate::algo::agari::Agari, Vec<String>)>;
}

impl AgariCaculatorWithYaku for crate::algo::agari::AgariCalculator<'_> {
    fn agari_with_names(&self, additional_hans: u8, doras: u8) -> Option<(crate::algo::agari::Agari, Vec<String>)> {
        if let Some((agari, names)) = self.search_yakus_with_names() {
            Some(match agari {
                crate::algo::agari::Agari::Normal { fu, han } => (
                    crate::algo::agari::Agari::Normal {
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
                crate::algo::agari::Agari::Normal {
                    fu: 0,
                    han: additional_hans + doras,
                },
                vec![],
            ))
        } else {
            let (tile14, key) = crate::algo::agari::get_tile14_and_key(self.tehai);
            let divs = crate::algo::agari::AGARI_TABLE.get(&key)?;

            let fu = divs
                .iter()
                .map(|div| crate::algo::agari::DivWorker::new(self, &tile14, div))
                .map(|w| w.calc_fu(false))
                .max()?;
            Some((
                crate::algo::agari::Agari::Normal {
                    fu,
                    han: additional_hans + doras,
                },
                vec![],
            ))
        }
    }

    fn search_yakus_with_names(&self) -> Option<(crate::algo::agari::Agari, Vec<String>)> {
        if self.is_menzen && crate::algo::shanten::calc_kokushi(self.tehai) == -1 {
            if self.tehai[self.winning_tile as usize] == 2 {
                return Some((
                    crate::algo::agari::Agari::Yakuman(2),
                    vec!["Thirteen-Orphans-Juusanmen".to_string()],
                ));
            } else {
                return Some((crate::algo::agari::Agari::Yakuman(1), vec!["Thirteen-Orphans".to_string()]));
            }
        }

        let (tile14, key) = crate::algo::agari::get_tile14_and_key(self.tehai);
        let divs = crate::algo::agari::AGARI_TABLE.get(&key)?;

        divs.iter()
            .map(|div| crate::algo::agari::DivWorker::new(self, &tile14, div))
            .filter_map(|w| w.search_yakus_with_names())
            .max_by_key(|(agari, _)| *agari)
    }
}

trait DivWorkerWithNames {
    fn search_yakus_with_names(&self) -> Option<(crate::algo::agari::Agari, Vec<String>)>;
}

impl DivWorkerWithNames for crate::algo::agari::DivWorker<'_> {
    fn search_yakus_with_names(&self) -> Option<(crate::algo::agari::Agari, Vec<String>)> {
        let mut han = 0;
        let mut yakuman = 0;
        let mut names = vec![];

        let has_pinfu = self.menzen_shuntsu.len() == 4
            && !crate::matches_tu8!(self.pair_tile, P | F | C)
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
                .all(|k| crate::matches_tu8!(k, 2s | 3s | 4s | 6s | 8s | F))
                && self.all_shuntsu().all(|s| s == crate::tu8!(2s));
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
                    } else if saneins == 2 && crate::matches_tu8!(self.pair_tile, P | F | C) {
                        han += 2;
                        names.push("Shousangen".to_string());
                    }
                }

                let winds = (0..4).filter(|&i| has_jihai[i]).count();
                if winds == 4 {
                    yakuman += 2;
                    names.push("Daisuushii".to_string());
                } else if winds == 3 && crate::matches_tu8!(self.pair_tile, E | S | W | N) {
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
            Some((crate::algo::agari::Agari::Yakuman(yakuman), names))
        } else if han > 0 {
            let fu = self.calc_fu(has_pinfu);
            Some((crate::algo::agari::Agari::Normal { fu, han }, names))
        } else {
            None
        }
    }
}
