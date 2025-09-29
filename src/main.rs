mod ekyumoe;
mod mjaigen;
mod state;

use clap::{Parser, Subcommand};
use riichi::algo::shanten::calc_all;
use riichi::hand::tile37_to_vec;
use riichi::state::{ActionCandidate, PlayerState};
use riichi::tile::Tile;
use riichi::{hand::hand_with_aka, mjai::Event};
use riichi::{must_tile, t};
use tinyvec::array_vec;

use crate::ekyumoe::read_ekyumoe_log;
use crate::mjaigen::parse_board;
use crate::state::ExpandedState;
use std::io::BufRead;

use anyhow::{Context, Result};

fn single_tile_hand(s: &str) -> Result<Tile> {
    Ok(*hand_with_aka_vec(s)?.first().context("Hand must contain one tile")?)
}

fn hand_with_aka_vec(s: &str) -> Result<Vec<Tile>> {
    Ok(tile37_to_vec(&hand_with_aka(s)?))
}

fn nested_hand_with_aka_vec(s: &str) -> Result<Vec<Vec<Tile>>> {
    s.split_whitespace().map(hand_with_aka_vec).collect()
}

#[derive(Parser, Debug)]
#[command(name = "washizu")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Hand(HandArgs),
    Board { args: Vec<String> },
    Parse { args: Vec<String> },
    Live { player_id: u8 },
    Ekyumoe { path: String },
}

// clap is insanely annoying with builtin custom parsers, so we parse later
#[derive(Parser, Debug)]
pub struct HandArgs {
    tehai: String,
    #[arg(long)]
    fuuro: Option<String>,
    #[arg(long)]
    ankan: Option<String>,
    #[arg(long)]
    dora_indicators: Option<String>,
    #[arg(long)]
    bakaze: Option<String>,
    #[arg(long)]
    jikaze: Option<String>,
    #[arg(long)]
    tiles_left: Option<u8>,
}

pub fn state_from_hand_args(args: HandArgs) -> Result<PlayerState> {
    let parsed_tehai = hand_with_aka(&args.tehai)?;
    let mut tehai = [0; 34];
    tehai.copy_from_slice(&parsed_tehai[..34]);

    let mut tiles_seen = tehai;

    let mut akas_in_hand = [false; 3];
    for (i, count) in parsed_tehai[34..].iter().enumerate() {
        if *count >= 1 {
            akas_in_hand[i] = true;
            tehai[must_tile!(34 + i).deaka().as_usize()] += 1;
        }
    }
    let mut doras_owned = parsed_tehai[34..].iter().sum();

    let mut chis = array_vec![];
    let mut pons = array_vec![];
    let mut minkans = array_vec![];
    let mut ankans = array_vec![];
    for fuuro in nested_hand_with_aka_vec(&args.fuuro.unwrap_or_default())?.iter() {
        if fuuro.len() == 3 {
            if fuuro[0] != fuuro[1] {
                chis.push(fuuro[0].as_u8());
            } else {
                pons.push(fuuro[0].as_u8());
            }
        } else {
            minkans.push(fuuro[0].as_u8());
        }
        for tile in fuuro {
            tiles_seen[tile.as_usize()] += 1;
            if tile.is_aka() {
                doras_owned += 1;
            }
        }
    }
    for ankan in nested_hand_with_aka_vec(&args.ankan.unwrap_or_default())?.iter() {
        tiles_seen[ankan[0].as_usize()] += 4;
        ankans.push(ankan[0].as_u8());
    }

    let dora_indicators = if let Some(dora_indicators_string) = &args.dora_indicators {
        hand_with_aka_vec(dora_indicators_string)?
    } else {
        vec![t!(E)]
    };
    for tile in &dora_indicators {
        tiles_seen[tile.as_usize()] += 1;
    }

    let tehai_len: u8 = parsed_tehai.iter().sum();
    let tehai_len_div3 = tehai_len / 3;
    let is_menzen = chis.is_empty() && pons.is_empty() && minkans.is_empty();
    let shanten = calc_all(&tehai, tehai_len_div3);
    let can_discard = tehai_len % 3 == 2;
    let can_riichi = can_discard && is_menzen && shanten == 0;
    let target_actor = if can_discard { 0 } else { 3 };

    Ok(PlayerState {
        player_id: 0,
        tehai,
        tiles_left: args.tiles_left.unwrap_or(70),
        tehai_len_div3,
        akas_in_hand,
        akas_seen: akas_in_hand,
        tiles_seen,
        chis,
        pons,
        minkans,
        ankans,
        doras_owned: [doras_owned, 0, 0, 0],
        is_menzen,
        shanten,
        bakaze: single_tile_hand(&args.bakaze.unwrap_or_default()).unwrap_or(t!(E)),
        jikaze: single_tile_hand(&args.jikaze.unwrap_or_default()).unwrap_or(t!(E)),
        dora_indicators: dora_indicators.into_iter().collect(),
        last_cans: ActionCandidate {
            can_discard,
            can_riichi,
            target_actor,
            ..Default::default()
        },
        ..Default::default()
    })
}

pub fn single_hand_analysis(args: HandArgs) {
    let state = state_from_hand_args(args).unwrap();
    println!("{}", ExpandedState::from_state(state.clone(), None).to_log_string());
}

pub fn board_analysis(args: Vec<String>) {
    let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let mut events = parse_board(args).unwrap().into_iter();
    let Event::StartGame { id, .. } = events.next().unwrap() else {
        panic!("first event must be StartGame")
    };
    let mut state = PlayerState::new(id.unwrap());
    for event in events {
        state.update(&event).unwrap();
    }

    println!("{}", ExpandedState::from_state(state.clone(), None).to_log_string());
}

pub fn main_live_analysis(player_id: u8) {
    let mut state = PlayerState::new(player_id);
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(l) = line else {
            eprintln!("failed to read line");
            continue;
        };
        let Ok(event) = serde_json::from_str::<Event>(&l) else {
            eprintln!("failed to parse json");
            continue;
        };
        state.update(&event).unwrap();
        match event {
            Event::Tsumo { actor, .. } if actor != state.player_id => continue,
            Event::Hora { actor, .. } if actor == state.player_id => continue,
            Event::EndKyoku => continue,
            _ => {}
        }
        print!("\x1B[2J\x1B[1;1H");
        println!("{}", ExpandedState::from_state(state.clone(), None).to_log_string());
    }
}

pub fn main_ekyumoe_analysis(path: &str) {
    let log = read_ekyumoe_log(path);
    let mut state = PlayerState::new(log.player_id);
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
    let cli = Cli::parse();
    match cli.command {
        Commands::Live { player_id } => {
            main_live_analysis(player_id);
        }
        Commands::Ekyumoe { path } => {
            main_ekyumoe_analysis(&path);
        }
        Commands::Hand(args) => {
            single_hand_analysis(args);
        }
        Commands::Board { args } => {
            board_analysis(args);
        }
        Commands::Parse { args } => {
            let args = args.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            let events = parse_board(args).unwrap();
            for event in events {
                println!("{}", serde_json::to_string(&event).unwrap());
            }
        }
    }
}
