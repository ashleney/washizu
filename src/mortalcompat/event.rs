//! event compatbility layer
//!
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
