use cosmwasm_std::{
    generic_err, to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier,
    StdResult, Storage,
};

use crate::msg::{Credentials, HandleMsg, InitMsg, QueryMsg};
use crate::state::{Coords, Game, Pasture, Player};

pub fn init<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: InitMsg,
) -> StdResult<InitResponse> {
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::NewGame { name } => try_new_game(&mut deps.storage, name),
        HandleMsg::Join {
            pasture,
            credentials,
        } => try_join(&mut deps.storage, credentials, pasture),
        HandleMsg::Shoot {
            coords,
            credentials,
        } => try_shoot(&mut deps.storage, credentials, coords),
        HandleMsg::Confirm {
            coords,
            credentials,
        } => try_confirm(&mut deps.storage, credentials, coords),
    }
}

fn try_new_game<S: Storage>(storage: &mut S, name: String) -> StdResult<HandleResponse> {
    // As long as the storage isn't corrupted somehow, this `?` should always succeed.
    if Game::may_load(storage, name.clone())?.is_some() {
        return Err(generic_err(format!(
            "game with name {:?} already exists",
            name
        )));
    }

    Game::new(name).save(storage)?;

    Ok(HandleResponse::default())
}

fn try_join<S: Storage>(
    storage: &mut S,
    credentials: Credentials,
    pasture: Pasture,
) -> StdResult<HandleResponse> {
    let mut game = Game::load(storage, credentials.game.clone())?;
    let player = Player::new(credentials.username, credentials.password, pasture);
    game.add_player(player)?;

    game.save(storage)?;

    Ok(HandleResponse::default())
}

fn try_shoot<S: Storage>(
    storage: &mut S,
    credentials: Credentials,
    coords: Coords,
) -> StdResult<HandleResponse> {
    let mut game = Game::load(storage, credentials.game.clone())?.full()?;

    if game.player().matches_credentials(&credentials) {
        return Err(generic_err("It's not your turn".to_string()));
    }
    game.shoot(coords);

    game.save(storage)?;

    Ok(HandleResponse::default())
}

fn try_confirm<S: Storage>(
    storage: &mut S,
    credentials: Credentials,
    coords: Coords,
) -> StdResult<HandleResponse> {
    let mut game = Game::load(storage, credentials.game.clone())?.full()?;

    if game.opponent().matches_credentials(&credentials) {
        return Err(generic_err(
            "You do not have permissions to confirm this shot".to_string(),
        ));
    }
    game.confirm_shot(coords);
    game.end_turn();

    game.save(storage)?;

    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::MyPasture { credentials } => try_get_my_pasture(&deps.storage, credentials),
        QueryMsg::MyShots { credentials } => try_get_my_shots(&deps.storage, credentials),
        QueryMsg::LastShot { credentials } => try_get_last_shot(&deps.storage, credentials),
    }
}

fn try_get_my_pasture<S: Storage>(storage: &S, credentials: Credentials) -> StdResult<Binary> {
    let game = Game::load(storage, credentials.game.clone())?.full()?;

    let pasture = game
        .player()
        .pasture(&credentials)
        .ok_or_else(|| generic_err("You do not have permissions to get the shots".to_string()))?;

    to_binary(pasture)
}

pub fn try_get_my_shots<S: Storage>(storage: &S, credentials: Credentials) -> StdResult<Binary> {
    let game = Game::load(storage, credentials.game.clone())?.full()?;
    let player = game.player();
    let opponent = game.opponent();
    let shots = if player.matches_credentials(&credentials) {
        game.get_player_shots();
    } else if opponent.matches_credentials(&credentials) {
        game.get_opponent_shots();
    } else {
        return Err(generic_err(
            "You do not have permissions to get this information".to_string(),
        ));
    };

    to_binary(&shots)
}

pub fn try_get_last_shot<S: Storage>(storage: &S, credentials: Credentials) -> StdResult<Binary> {
    let game = Game::load(storage, credentials.game.clone())?.full()?;
    let player = game.player();
    let opponent = game.opponent();
    let last_shot =
        if player.matches_credentials(&credentials) || opponent.matches_credentials(&credentials) {
            game.next_shot();
        } else {
            return Err(generic_err(
                "You do not have permissions to get this information".to_string(),
            ));
        };

    to_binary(&last_shot)
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_binary, from_slice, to_vec, StdError};

    use crate::state::{Herd, Orientation};

    use super::*;

    #[test]
    fn test_herd_serialize() {
        let serialized =
            "{\"orientation\": \"horizontal\", \"length\": 3, \"coords\": {\"x\": 2, \"y\": 4}}"
                .as_bytes();
        let herd: Herd = from_slice(&serialized).unwrap();
        println!("{:?}", herd);

        let herd = Herd::new(4, 6, 3, Orientation::Vertical);
        let serialized = to_vec(&herd).unwrap();
        let serialized = String::from_utf8_lossy(&serialized);
        println!("{:?}", serialized);
    }
}
