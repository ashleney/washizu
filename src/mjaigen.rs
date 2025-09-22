#![allow(dead_code)]
//! Generate mjai logs from the current gamestate
//! Information is lost by not knowing when a tile was called.
use anyhow::{Context, Result, bail, ensure};
use riichi::{mjai::Event, must_tile, t, tile::Tile, tu8, tuz};
use std::{array::from_fn, iter::once, str::FromStr};
use tinyvec::ArrayVec;

/// read an ordered tile string
pub fn parse_tiles(s: &str) -> Result<Vec<Tile>> {
    ensure!(s.is_ascii(), "hand {s} contains non-ascii content");

    let mut tiles = vec![];
    let mut stack = vec![];

    for b in s.as_bytes() {
        match b {
            b'0'..=b'9' => stack.push(b - b'0'),
            b'm' | b'p' | b's' | b'z' => {
                for t in stack.drain(..) {
                    let tile = if t == 0 {
                        match b {
                            b'm' => t!(5mr),
                            b'p' => t!(5pr),
                            b's' => t!(5sr),
                            _ => bail!("unexpected byte {b}"),
                        }
                    } else {
                        let kind = match b {
                            b'm' => 0,
                            b'p' => 1,
                            b's' => 2,
                            b'z' => 3,
                            _ => unreachable!(),
                        };
                        must_tile!(kind * 9 + t - 1)
                    };
                    tiles.push(tile);
                }
            }
            _ if b.is_ascii_whitespace() => (),
            _ => bail!("unexpected byte {b}"),
        };
    }

    Ok(tiles)
}

fn parse_tile(s: &str) -> Result<Tile> {
    let tiles = parse_tiles(s)?;
    ensure!(tiles.len() == 1, "too many tiles");
    tiles.into_iter().next().context("missing tile")
}

/// Open meld
#[derive(Debug, Clone, Copy)]
pub struct Naki {
    /// Called tile
    pub pai: Tile,
    /// Consumed tiles from player's own hand, empty for kakan
    pub consumed: ArrayVec<[Tile; 4]>,
    /// player that discarded tile relative to us
    pub target: u8,
}

/// Discarded tile
#[derive(Debug, Clone, Copy)]
pub struct Sutehai {
    /// Discarded tile
    pub pai: Tile,
    /// Whether tile was discarded from hand
    pub tedashi: bool,
    /// Whether this tile is the riichi declaration tile
    pub riichi: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Board {
    /// Round wind
    pub bakaze: Tile,
    /// Seat wind
    pub jikaze: Tile,
    /// Kyoku in the current round wind
    pub kyoku: u8,
    /// Repeat counters
    pub honba: u8,
    /// Riichi stick counters
    pub kyotaku: u8,
    /// Points in the current state
    pub scores: [i32; 4],
    /// Dora indicators
    pub dora_indicators: Vec<Tile>,
    /// discarded tiles, relative
    pub kawa: [Vec<Sutehai>; 4],
    /// Melds of players from earliest to latest, relative
    pub fuuro: [Vec<Naki>; 4],
    /// Tehai in the current state
    pub tehai: Vec<Tile>,
}

pub fn naki_to_event(naki: Naki, actor: u8, player_abs: impl Fn(usize) -> u8) -> Event {
    if naki.consumed.len() == 2 && naki.consumed[0].deaka() != naki.consumed[1].deaka() {
        Event::Chi {
            actor,
            target: player_abs(naki.target as usize),
            pai: naki.pai,
            consumed: naki.consumed.as_slice().try_into().unwrap(),
        }
    } else if naki.consumed.len() == 2 {
        Event::Pon {
            actor,
            target: player_abs(naki.target as usize),
            pai: naki.pai,
            consumed: naki.consumed.as_slice().try_into().unwrap(),
        }
    } else if naki.consumed.len() == 3 && actor != naki.target {
        Event::Daiminkan {
            actor,
            target: player_abs(naki.target as usize),
            pai: naki.pai,
            consumed: naki.consumed.as_slice().try_into().unwrap(),
        }
    } else if naki.consumed.len() == 4 {
        Event::Ankan {
            actor,
            consumed: naki.consumed.as_slice().try_into().unwrap(),
        }
    } else if naki.consumed.is_empty() {
        Event::Kakan {
            actor,
            pai: naki.pai,
            consumed: [naki.pai; 3],
        }
    } else {
        panic!("Unknown meld combination {naki:?}");
    }
}

pub fn generate_mjai_logs(board: Board) -> Result<Vec<Event>> {
    let oya = board.kyoku - 1;
    let player_id = (4 + oya + board.jikaze.as_u8() - tu8!(E)) % 4;

    let player_abs = |player| (player as u8 + player_id) % 4;
    let player_rel = |player| (4 + player - player_id) % 4;
    let mut scores = board.scores;
    scores.rotate_right(player_id as usize);

    // information about some discards is lost, we'll estimate them to be tedashi non-riichi for simplicity
    let mut called_sutehais: [Vec<(Sutehai, u8)>; 4] = from_fn(|_| vec![]); // rel
    for (rel_player, fuuro) in board.fuuro.iter().enumerate() {
        for naki in fuuro.iter() {
            if naki.target != rel_player as u8 {
                let sutehai = Sutehai {
                    pai: naki.pai,
                    tedashi: true,
                    riichi: false,
                };
                called_sutehais[naki.target as usize].push((sutehai, player_abs(rel_player)));
            }
        }
    }

    let at_discard = board.tehai.len() % 3 == 2;

    // construct player turns by attaching calls to the first viable dahai
    // contains: tsumo or call replacing tsumo, ankan/kakan calls, riichi declaration, dahai, dora reveal
    // included is the player that will want to call after the last event
    // the last turn may not have a dahai action if the provided Board is right after a call
    // unknown tsumo tiles and dora indicators will be filled in later based on what tiles could possibly be in there
    let mut turns: [Vec<(Vec<Event>, Option<u8>)>; 4] = from_fn(|_| vec![]);
    for (rel_player, kawa) in board.kawa.iter().enumerate() {
        let actor = player_abs(rel_player);
        let mut fuuro_iter = board.fuuro[rel_player]
            .iter()
            .map(|naki| naki_to_event(*naki, actor, player_abs))
            .peekable();
        let mut kakan_candidates = vec![];
        for (sutehai, next_player) in called_sutehais[rel_player]
            .iter()
            .map(|(called_sutehai, next_player)| (Some(called_sutehai), Some(next_player)))
            .chain(kawa.iter().map(|sutehai| (Some(sutehai), None)))
            .chain(once((None, None)))
        {
            let mut reveal_dora_at_discard = false;
            let mut events = vec![];

            let is_first_oya_act = actor == oya && turns[actor as usize].is_empty();
            let draw_event = if !is_first_oya_act
                && let Some(next_event) = fuuro_iter.peek()
                && (matches!(next_event, Event::Chi { .. } | Event::Pon { .. } if sutehai.is_none_or(|sutehai| sutehai.tedashi))
                    || matches!(next_event, Event::Daiminkan { .. }))
            {
                match next_event {
                    Event::Pon { pai, .. } => kakan_candidates.push(*pai),
                    Event::Daiminkan { .. } => reveal_dora_at_discard = true,
                    _ => {}
                }
                fuuro_iter.next().unwrap()
            } else {
                Event::Tsumo { actor, pai: t!(?) }
            };
            events.push(draw_event.clone());

            if !matches!(draw_event, Event::Chi { .. } | Event::Pon { .. }) {
                while let Some(next_naki) = fuuro_iter.peek() {
                    match next_naki {
                        Event::Ankan { .. } => {
                            if reveal_dora_at_discard {
                                events.push(Event::Dora { dora_marker: t!(?) });
                                reveal_dora_at_discard = false;
                            }
                            events.push(fuuro_iter.next().unwrap());
                            events.push(Event::Dora { dora_marker: t!(?) });
                            events.push(Event::Tsumo { actor, pai: t!(?) });
                        }
                        Event::Kakan { pai, .. } if kakan_candidates.contains(pai) => {
                            events.push(fuuro_iter.next().unwrap());
                            if reveal_dora_at_discard {
                                events.push(Event::Dora { dora_marker: t!(?) });
                            }
                            events.push(Event::Tsumo { actor, pai: t!(?) });
                            reveal_dora_at_discard = true;
                        }
                        _ => break,
                    }
                }
            }

            if let Some(sutehai) = sutehai {
                if sutehai.riichi {
                    events.push(Event::Reach { actor });
                    scores[actor as usize] += 1000;
                }
                events.push(Event::Dahai {
                    actor,
                    pai: sutehai.pai,
                    tsumogiri: !sutehai.tedashi,
                });
                if reveal_dora_at_discard {
                    events.push(Event::Dora { dora_marker: t!(?) });
                }
            }

            if !(events.len() == 1 && matches!(events[0], Event::Tsumo { .. }) && !(rel_player == 0 && at_discard)) {
                turns[actor as usize].push((events, next_player.copied()));
            }
        }
        if fuuro_iter.next().is_some() {
            bail!("more calls than tsumos");
        }
    }

    // remaining tiles which have not been witnessed and could therefore be in someone's tehai
    let mut remaining_tiles = [4i8; 37]; // may go negative in case it's not 3aka
    remaining_tiles[tuz!(5m)] = 3;
    remaining_tiles[tuz!(5p)] = 3;
    remaining_tiles[tuz!(5s)] = 3;
    remaining_tiles[tuz!(5mr)] = 1;
    remaining_tiles[tuz!(5pr)] = 1;
    remaining_tiles[tuz!(5sr)] = 1;
    for tile in board.dora_indicators.iter().chain(board.tehai.iter()) {
        remaining_tiles[tile.as_usize()] -= 1;
    }
    for &Sutehai { pai, .. } in board.kawa.iter().flatten() {
        remaining_tiles[pai.as_usize()] -= 1;
    }
    for &Naki { pai, .. } in board.fuuro.iter().flatten() {
        remaining_tiles[pai.as_usize()] -= 1;
    }

    // reverse pass to fill in tehai and tsumo tiles
    let mut tehais: [Vec<Tile>; 4] = from_fn(|_| vec![t!(?); 13]);
    tehais[player_id as usize] = board.tehai;
    for player in (0..=3).filter(|player| *player != player_id) {
        // TODO: merge shouminkan and pon
        let fuuro_size = board.fuuro[player_rel(player) as usize]
            .iter()
            .filter(|naki| !naki.consumed.is_empty())
            .count();
        let expected_tehai_size = 13 - 3 * fuuro_size;
        let mut tehai = vec![];
        'outer: for (tile, count) in remaining_tiles.iter_mut().enumerate() {
            for _ in 0..*count {
                if tehai.len() >= expected_tehai_size {
                    break 'outer;
                }
                tehai.push(must_tile!(tile));
                *count -= 1;
            }
        }
        tehais[player as usize] = tehai;
    }

    for (player, turns) in turns.iter_mut().enumerate() {
        for (turn, _) in turns.iter_mut().rev() {
            for event in turn.iter_mut().rev() {
                match event {
                    Event::Tsumo { .. } => {
                        *event = Event::Tsumo {
                            actor: player as u8,
                            pai: tehais[player].pop().unwrap(),
                        }
                    }
                    Event::Dahai { tsumogiri, pai, .. } => {
                        if *tsumogiri {
                            tehais[player].push(*pai);
                        } else {
                            tehais[player].insert(tehais[player].len() - 2, *pai);
                        }
                    }
                    Event::Chi { consumed, .. } | Event::Pon { consumed, .. } => {
                        tehais[player].extend(consumed.iter());
                    }
                    Event::Daiminkan { consumed, .. } => {
                        tehais[player].extend(consumed.iter());
                    }
                    Event::Kakan { pai, .. } => {
                        tehais[player].push(*pai);
                    }
                    Event::Ankan { consumed, .. } => {
                        tehais[player].extend(consumed.iter());
                    }
                    _ => {}
                }
            }
        }
    }

    let mut events = vec![];
    events.push(Event::StartGame { id: Some(player_id) });
    events.push(Event::StartKyoku {
        bakaze: board.bakaze,
        dora_marker: board.dora_indicators[0],
        kyoku: board.kyoku,
        honba: board.honba,
        kyotaku: board.kyotaku,
        oya,
        scores,
        tehais: tehais.map(|tehai| tehai.try_into().expect("Incorrect tehai size")),
    });

    let mut current_player = oya;
    let mut revealed_doras = 1;
    let mut turns_iter = turns.map(|turns| turns.into_iter());
    loop {
        let Some((mut turn_events, next_player)) = turns_iter[current_player as usize].next() else {
            break;
        };
        for event in turn_events.iter_mut() {
            if matches!(event, Event::Dora { .. }) {
                // this is the only place we can be sure the events are well ordered
                *event = Event::Dora {
                    dora_marker: board.dora_indicators[revealed_doras],
                };
                revealed_doras += 1;
            }
        }
        events.append(&mut turn_events);
        current_player = next_player.unwrap_or((current_player + 1) % 4);
    }
    if turns_iter.iter_mut().any(|turns| turns.len() != 0) {
        bail!(
            "Incorrect number of turns, remaining {:?}",
            turns_iter.map(|turns| turns.len())
        );
    }

    Ok(events)
}

/// Parse a string representation of a board
pub fn parse_board(args: Vec<&str>) -> Result<Vec<Event>> {
    let mut parts = args.into_iter();

    let mut board = Board::default();

    let kyoku = parts.next().context("missing kyoku")?;
    ensure!(kyoku.len() == 2, "kyoku must be <bakaze><honba> (e.g. S3)");
    board.bakaze = Tile::from_str(&kyoku[0..1]).context("incorrect bakaze")?;
    board.kyoku = kyoku[1..2].parse().context("incorrect kyoku")?;

    board.jikaze = parts.next().context("missing jikaze")?.parse().context("incorrect jikaze")?;
    board.kyotaku = parts.next().context("missing kyotaku")?.parse().context("incorrectkyotaku")?;
    board.honba = parts.next().context("missing honba")?.parse().context("incorrect honba")?;
    board.dora_indicators = parse_tiles(parts.next().context("missing dora")?).context("incorrect dora")?;
    for score in board.scores.iter_mut() {
        *score = parts.next().context("missing score")?.parse().context("incorrect score")?;
    }
    board.tehai = parse_tiles(parts.next().context("missing self tehai")?).context("incorrect tehai")?;

    for kawa in board.kawa.iter_mut() {
        let mut chars = parts.next().context("missing kawa")?.chars().peekable();
        if matches!(chars.peek(), Some('/')) {
            continue;
        }
        while chars.peek().is_some() {
            let tile_string = format!("{}{}", chars.next().unwrap(), chars.next().context("incorrect kawa")?);
            let (tedashi, riichi) = match chars.peek() {
                Some('.') => {
                    chars.next();
                    (true, false)
                }
                Some('-') => {
                    chars.next();
                    (true, true)
                }
                _ => (false, false),
            };
            kawa.push(Sutehai {
                pai: parse_tile(&tile_string)?,
                tedashi,
                riichi,
            });
        }
    }
    for (player, fuuro) in board.fuuro.iter_mut().enumerate() {
        let mut fuuro_iter = parts.next().context("missing fuuro")?.chars().peekable();
        if matches!(fuuro_iter.peek(), Some('/')) {
            continue;
        }
        // chi (1p)2p3p, pon (1p)1p1p, daiminkan (1p)1p1p1p, ankan 1p1p1p1p, kakan (1p)
        for naki_chars in fuuro_iter.collect::<Vec<_>>().split(|char| *char == ',') {
            let mut naki_iter = naki_chars.iter().peekable();
            let mut pai: Option<(Tile, u8)> = None;
            let mut consumed: Vec<Tile> = vec![];
            loop {
                match naki_iter.peek() {
                    Some('(') => {
                        _ = naki_iter.next();
                        let tile_string = format!(
                            "{}{}",
                            naki_iter.next().unwrap(),
                            naki_iter.next().context("incorrect fuuro")?
                        );
                        ensure!(naki_iter.next() == Some(&')'), "missing closing parenthesis");
                        pai = Some((parse_tile(&tile_string)?, consumed.len() as u8))
                    }
                    Some(_) => {
                        let tile_string = format!(
                            "{}{}",
                            naki_iter.next().unwrap(),
                            naki_iter.next().context("incorrect fuuro")?
                        );
                        consumed.push(parse_tile(&tile_string)?);
                    }
                    None => break,
                }
            }
            // TODO: Make Naki more accurate to what is inputted
            if let Some((pai, pai_pos)) = pai {
                let rel_target = match pai_pos {
                    0 => 3,
                    p if p == consumed.len() as u8 => 1,
                    _ => 2,
                };
                fuuro.push(Naki {
                    pai,
                    consumed: consumed.into_iter().collect(),
                    target: (rel_target + player as u8) % 4,
                });
            } else {
                fuuro.push(Naki {
                    pai: t!(?),
                    consumed: consumed.into_iter().collect(),
                    target: 0,
                });
            }
        }
    }

    generate_mjai_logs(board)
}
