mod danger;
mod ekyumoecompat;
mod mortalcompat;
mod state;
use crate::ekyumoecompat::read_ekyumoe_log;
use crate::mortalcompat::agari::AgariCaculatorWithYaku;
use crate::mortalcompat::sp::CandidateExt;
use crate::state::ExpandedState;
use std::io::BufRead;

/// Provide live analysis, meant to be used alongside a mortal analysis tool
pub fn main_live_analysis(player_id: u8) {
    // TODO: Colored logs
    let mut state = riichi::state::PlayerState::new(player_id);
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(l) = line else {
            eprintln!("failed to read line");
            continue;
        };
        let Ok(event) = serde_json::from_str::<riichi::mjai::Event>(&l) else {
            eprintln!("failed to parse json");
            continue;
        };
        state.update(&event).unwrap();
        match event {
            riichi::mjai::Event::Tsumo { actor, .. } if actor != state.player_id => continue,
            riichi::mjai::Event::Hora { actor, .. } if actor == state.player_id => continue,
            riichi::mjai::Event::EndKyoku => continue,
            _ => {}
        }
        print!("\x1B[2J\x1B[1;1H");
        println!("{}", ExpandedState::from_state(state.clone(), None).to_log_string());
    }
}

/// Provide extra information to an ekyumoe analysis
pub fn main_ekyumoe_analysis(path: &str) {
    let log = read_ekyumoe_log(path);
    let mut state = riichi::state::PlayerState::new(log.player_id);
    let events_with_details = log.events_with_detail();

    let pb = if !console::user_attended() {
        Some(indicatif::ProgressBar::new(events_with_details.len() as u64))
    } else {
        None
    };
    for (event, details) in log.events_with_detail() {
        if let Some(ref pb) = pb {
            pb.inc(1);
        }
        state.update(&event).unwrap();
        println!("\n{event:?}");
        if !state.last_cans.can_act() {
            continue;
        }
        println!("{}", ExpandedState::from_state(state.clone(), details).to_log_string());
    }
    if let Some(ref pb) = pb {
        pb.finish();
    }
}

pub fn main_single_analysis(tehai: Vec<riichi::tile::Tile>) {
    let mut hand = [0; 34];
    let mut akas_in_hand = [false; 3];
    for tile in tehai.iter() {
        hand[tile.deaka().as_usize()] += 1;
        match tile.as_u8() {
            riichi::tu8!(5m) => akas_in_hand[0] = true,
            riichi::tu8!(5p) => akas_in_hand[1] = true,
            riichi::tu8!(5s) => akas_in_hand[2] = true,
            _ => {}
        }
    }
    let len_div3 = (tehai.len() / 3) as u8;
    let shanten = riichi::algo::shanten::calc_all(&hand, len_div3);
    if shanten == -1 {
        let agari_calc = riichi::algo::agari::AgariCalculator {
            tehai: &hand,
            is_menzen: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            bakaze: riichi::tu8!(E),
            jikaze: riichi::tu8!(E),
            winning_tile: tehai.last().unwrap().deaka().as_u8(),
            is_ron: true,
        };

        if let Some((agari, names)) = agari_calc.agari_with_names(0, 0) {
            println!("{} [{}]", agari.point(true).ron, names.join(", "))
        } else {
            println!("no-yaku")
        }
        return;
    }
    let init_state = riichi::algo::sp::InitState {
        tehai: hand,
        akas_in_hand,
        tiles_seen: hand,
        akas_seen: akas_in_hand,
    };
    let sp_calc = riichi::algo::sp::SPCalculator {
        tehai_len_div3: len_div3,
        is_menzen: true,
        chis: &[],
        pons: &[],
        minkans: &[],
        ankans: &[],
        bakaze: riichi::tu8!(E),
        jikaze: riichi::tu8!(E),
        num_doras_in_fuuro: 0,
        prefer_riichi: true,
        dora_indicators: &[],
        calc_double_riichi: false,
        calc_haitei: false,
        sort_result: true,
        maximize_win_prob: false,
        calc_tegawari: shanten <= 2,
        calc_shanten_down: shanten <= 2,
    };
    let max_ev_table = sp_calc.calc(init_state, (tehai.len() % 3) == 2, 17, shanten).unwrap();
    println!(
        "{}",
        max_ev_table
            .iter()
            .map(|candidate| candidate.to_candidate_string())
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args
        .first()
        .expect("usage: washizu live [player_id] | washizu ekyumoe [path]")
        .as_str()
    {
        "live" => {
            let player_id = args
                .get(1)
                .expect("missing player_id for live")
                .parse::<u8>()
                .expect("invalid player_id");
            main_live_analysis(player_id);
        }
        "ekyumoe" => {
            let path = args.get(1).expect("missing path for ekyumoe");
            main_ekyumoe_analysis(path);
        }
        "single" => {
            let hand = &riichi::hand::hand_with_aka(args.get(1).expect("Missing hand for single")).expect("Malformed hand");
            main_single_analysis(riichi::hand::tile37_to_vec(hand));
        }
        cmd => {
            panic!("unrecognized command: {cmd}");
        }
    }
}
