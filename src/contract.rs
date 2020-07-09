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
        return Err(generic_err(format!("I'ts not your turn")));
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
        return Err(generic_err(format!(
            "You do not have permissions to confirm this shot"
        )));
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
        .ok_or_else(|| generic_err(format!("You do not have permissions to get the shots")))?;

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
        return Err(generic_err(format!(
            "You do not have permissions to get this information"
        )));
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
            return Err(generic_err(format!(
                "You do not have permissions to get this information"
            )));
        };

    to_binary(&last_shot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::{coins, from_binary, StdError};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies(20, &[]);

        let msg = InitMsg { count: 17 };
        let env = mock_env(&deps.api, "creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, env, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies(20, &coins(2, "token"));

        let msg = InitMsg { count: 17 };
        let env = mock_env(&deps.api, "creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // beneficiary can release it
        let env = mock_env(&deps.api, "anyone", &coins(2, "token"));
        let msg = HandleMsg::Increment {};
        let _res = handle(&mut deps, env, msg).unwrap();

        // should increase counter by 1
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies(20, &coins(2, "token"));

        let msg = InitMsg { count: 17 };
        let env = mock_env(&deps.api, "creator", &coins(2, "token"));
        let _res = init(&mut deps, env, msg).unwrap();

        // beneficiary can release it
        let unauth_env = mock_env(&deps.api, "anyone", &coins(2, "token"));
        let msg = HandleMsg::Reset { count: 5 };
        let res = handle(&mut deps, unauth_env, msg);
        match res {
            Err(StdError::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_env = mock_env(&deps.api, "creator", &coins(2, "token"));
        let msg = HandleMsg::Reset { count: 5 };
        let _res = handle(&mut deps, auth_env, msg).unwrap();

        // should now be 5
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
