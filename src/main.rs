mod danger;
mod ekyumoecompat;
mod mortalcompat;
mod state;
use crate::ekyumoecompat::read_ekyumoe_log;
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
        cmd => {
            panic!("unrecognized command: {cmd}");
        }
    }
}
