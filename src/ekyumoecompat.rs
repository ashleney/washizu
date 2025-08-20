#[derive(serde::Deserialize)]
pub struct EkyuMoeReview {
    pub player_id: u8,
    pub review: Review,
    pub mjai_log: Vec<riichi::mjai::Event>,
}

#[derive(serde::Deserialize)]
pub struct Review {
    pub kyokus: Vec<KyokuReview>,
}

#[derive(serde::Deserialize)]
pub struct KyokuReview {
    pub entries: Vec<Entry>,
}

#[derive(serde::Deserialize, Debug)]
pub struct Entry {
    pub junme: u8,
    pub last_actor: u8,
    pub tile: riichi::tile::Tile,
    pub details: Vec<Detail>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Detail {
    pub action: riichi::mjai::Event,
    pub q_value: f32,
    pub prob: f32,
}

pub fn read_ekyumoe_log(path: &str) -> EkyuMoeReview {
    serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path).unwrap())).unwrap()
}

impl EkyuMoeReview {
    pub fn events_with_detail(&self) -> Vec<(riichi::mjai::Event, Option<Vec<Detail>>)> {
        let mut kyoku = 0;
        let mut index = 0;
        let mut events_with_details = vec![];
        for event in self.mjai_log.iter() {
            if matches!(event, riichi::mjai::event::Event::EndGame) {
                events_with_details.push((event.clone(), None));
                continue;
            }
            if matches!(event, riichi::mjai::Event::EndKyoku) {
                index = 0;
                kyoku += 1;
                events_with_details.push((event.clone(), None));
                continue;
            }
            let Some(entry) = self.review.kyokus[kyoku].entries.get(index) else {
                events_with_details.push((event.clone(), None));
                continue;
            };
            if matches!(event, riichi::mjai::Event::Tsumo { actor, .. } if *actor != self.player_id)
                || matches!(event, riichi::mjai::Event::Dahai { actor, .. } if *actor == self.player_id)
            {
                events_with_details.push((event.clone(), None));
                continue;
            }
            let self_riichi_discard = matches!(event, riichi::mjai::Event::Reach { actor } if *actor == self.player_id)
                && self.review.kyokus[kyoku].entries.get(index - 1).map(|x| x.junme) == Some(entry.junme);
            let last_tsumo_or_discard = match event {
                riichi::mjai::Event::Tsumo { actor, pai } if *actor == self.player_id => Some(*pai),
                riichi::mjai::Event::Dahai { pai, .. } => Some(*pai),
                riichi::mjai::Event::Kakan { pai, .. } => Some(*pai),
                _ => None,
            };
            if event.actor() == Some(entry.last_actor) && (last_tsumo_or_discard == Some(entry.tile) || self_riichi_discard) {
                events_with_details.push((event.clone(), Some(entry.details.clone())));
                index += 1;
                continue;
            }
            events_with_details.push((event.clone(), None));
        }

        events_with_details
    }
}
