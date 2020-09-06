use derive_more::Display;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{Credentials, Shots};
use cosmwasm_std::{generic_err, StdResult, Storage};
use cosmwasm_storage::{prefixed, prefixed_read, singleton, singleton_read};
use std::collections::HashMap;
use std::ops::{AddAssign, Deref, DerefMut};

const GAMES: &[u8] = b"games";

const PASTURE_SIZE: u8 = 10;

/// This type represents a game that has been correctly configured and has two players.
#[derive(Clone, Debug)]
pub struct FullGame {
    game: Game,
}

impl FullGame {
    pub fn player(&self) -> &Player {
        let state = &self.game.state;
        &state.players[state.turn as usize]
    }

    pub fn player_mut(&mut self) -> &mut Player {
        let state = &mut self.state;
        &mut state.players[state.turn as usize]
    }

    pub fn opponent(&self) -> &Player {
        let state = &self.state;
        let turn = (state.turn + 1) % 2;
        &state.players[turn as usize]
    }

    pub fn opponent_mut(&mut self) -> &mut Player {
        let state = &mut self.state;
        let turn = (state.turn + 1) % 2;
        &mut state.players[turn as usize]
    }

    pub fn shoot(&mut self, coords: Coords) {
        self.state.next_shot = Some(coords);
    }

    pub fn next_shot(&self) -> Option<Coords> {
        self.state.next_shot
    }

    /// Confirm the shot performed previously.
    ///
    /// We have to add this step to prevent players from running the game offline and checking all the slots themselves.
    pub fn confirm_shot(&mut self, coords: Coords) {
        self.player_mut().pasture.shots.push(coords);
    }

    pub fn get_player_shots(&self) -> Shots {
        let pasture = &self.opponent().pasture;
        let all_shots: &[Coords] = &pasture.shots;
        let (hits, misses) = all_shots
            .into_iter()
            .partition(|shot| pasture.herds.iter().any(|herd| herd.is_at(**shot)));

        Shots { hits, misses }
    }

    pub fn get_opponent_shots(&self) -> Shots {
        let pasture = &self.player().pasture;
        let all_shots: &[Coords] = &pasture.shots;
        let (hits, misses) = all_shots
            .into_iter()
            .partition(|shot| pasture.herds.iter().any(|herd| herd.is_at(**shot)));

        Shots { hits, misses }
    }

    /// End the running turn.
    ///
    /// This will always be called by the opponent of the current player, after confirming the shot.
    pub fn end_turn(&mut self) {
        self.state.next_shot = None;
        self.state.turn = (self.state.turn + 1) % 2;
    }
}

impl Deref for FullGame {
    type Target = Game;
    fn deref(&self) -> &Self::Target {
        &self.game
    }
}

impl DerefMut for FullGame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.game
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    name: String,
    state: GameState,
}

impl Game {
    pub fn new(name: String) -> Self {
        Self {
            name,
            state: GameState::default(),
        }
    }

    pub fn full(self) -> StdResult<FullGame> {
        if self.state.players.len() != 2 {
            return Err(generic_err(format!("Not enough players in game!")));
        }
        Ok(FullGame { game: self })
    }

    pub fn save<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(&mut prefixed(GAMES, storage), self.name.as_bytes()).save(&self.state)
    }

    pub fn load<S: Storage>(storage: &S, name: String) -> StdResult<Self> {
        let state = singleton_read(&prefixed_read(GAMES, storage), name.as_bytes()).may_load()?;
        if let Some(state) = state {
            Ok(Self { name, state })
        } else {
            Err(generic_err(format!("Game named {:?} doesn't exist", name)))
        }
    }

    pub fn may_load<S: Storage>(storage: &S, name: String) -> StdResult<Option<Self>> {
        singleton_read(&prefixed_read(GAMES, storage), name.as_bytes())
            .may_load()
            .map(|maybe| maybe.map(|state| Self { name, state }))
    }

    pub fn add_player(&mut self, player: Player) -> StdResult<()> {
        if self.state.players.len() == 1 {
            if self.state.players[0].username == player.username {
                return Err(generic_err(format!(
                    "username {} is already taken!",
                    player.username
                )));
            }
        }
        if self.state.players.len() > 2 {
            return Err(generic_err(String::from("Game already full!")));
        }

        player.pasture.verify()?;
        // TODO add minimum limit on password strength?

        self.state.players.push(player);

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct GameState {
    /// The two players in the game
    players: Vec<Player>,
    /// The index of the next player to shoot. 0 or 1.
    turn: u8,
    /// The coordinate of the next shot. pending confirmation. None means no shot is pending confirmation.
    next_shot: Option<Coords>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct Player {
    username: String,
    password: String,
    pasture: Pasture,
}

impl Player {
    pub fn new(username: String, password: String, pasture: Pasture) -> Self {
        Self {
            username,
            password,
            pasture,
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    // TODO make this more constant time to prevent side-channel attacks on the credentials
    pub fn matches_credentials(&self, credentials: &Credentials) -> bool {
        self.username == credentials.username && self.password == credentials.password
    }

    pub fn pasture(&self, credentials: &Credentials) -> Option<&Pasture> {
        if !self.matches_credentials(credentials) {
            None
        } else {
            Some(&self.pasture)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct Pasture {
    herds: Vec<Herd>,
    shots: Vec<Coords>,
}

fn expected_herd_count_of_length(length: u8) -> u32 {
    match length {
        2 => 1,
        3 => 2,
        4 => 1,
        5 => 1,
        _ => 0,
    }
}

impl Pasture {
    pub fn new(herds: Vec<Herd>, shots: Vec<Coords>) -> Self {
        Self { herds, shots }
    }

    fn verify(&self) -> StdResult<()> {
        // Check that the amount of herds is correct
        // this is a mapping of herd length to count of herds with that length
        let mut herds = HashMap::<u8, u32>::new();

        for herd in self.herds.iter() {
            herd.verify()?;
            herds
                .entry(herd.length)
                .and_modify(|count| count.add_assign(1_u32))
                .or_insert(1);
        }

        for (length, count) in herds.into_iter() {
            let expected_count = expected_herd_count_of_length(length);
            if expected_count > count {
                return Err(generic_err(format!(
                    "Too many herds of length {}. You should only have {} but you have {}",
                    length, expected_count, count
                )));
            }
            if expected_count < count {
                return Err(generic_err(format!(
                    "You need {} herds of length {}. Found only {}",
                    count, length, expected_count
                )));
            }
        }

        // Check that herds do not collide
        for (index_1, herd_1) in self.herds.iter().enumerate() {
            for (index_2, herd_2) in self.herds.iter().enumerate() {
                if index_1 == index_2 {
                    continue;
                }
                if herd_1.intersects(herd_2) {
                    return Err(generic_err(format!(
                        "Herd {} from {} to {} intersects with herd {} from {} to {}",
                        index_1,
                        herd_1.coords,
                        herd_1.end(),
                        index_2,
                        herd_2.coords,
                        herd_2.end()
                    )));
                }
            }
        }

        Ok(())
    }
}

/// A group of sheep
///
/// This represents a line of sheep following each other.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Herd {
    /// Coordinate of the north-west-most sheep
    coords: Coords,
    /// Amount of sheep
    length: u8,
    /// What way is the herd oriented
    orientation: Orientation,
}

impl Herd {
    pub fn new(x: u8, y: u8, length: u8, orientation: Orientation) -> Self {
        Self {
            coords: Coords { x, y },
            length,
            orientation,
        }
    }

    pub fn is_at(&self, coord: Coords) -> bool {
        let my_x = self.coords.x;
        let my_y = self.coords.y;
        let end = self.end();
        let end_x = end.x;
        let end_y = end.y;
        let x = coord.x;
        let y = coord.y;
        match self.orientation {
            Orientation::Horizontal => my_y == y && x >= my_x && x <= end_x,
            Orientation::Vertical => my_x == x && y >= my_y && y <= end_y,
        }
    }

    fn intersects(&self, other: &Herd) -> bool {
        let self_end = self.end();
        let other_end = other.end();

        ranges_intersect(self.coords.x, self_end.x, other.coords.x, other_end.x)
            && ranges_intersect(self.coords.y, self_end.y, other.coords.y, other_end.y)
    }

    /// location of last sheep
    fn end(&self) -> Coords {
        match self.orientation {
            Orientation::Horizontal => Coords {
                x: self.coords.x.saturating_add(self.length - 1),
                y: self.coords.y,
            },
            Orientation::Vertical => Coords {
                x: self.coords.x,
                y: self.coords.y.saturating_add(self.length - 1),
            },
        }
    }

    fn verify(&self) -> StdResult<()> {
        if self.length == 0 {
            return Err(generic_err(
                format!("Herd at {} has no sheep", self.coords,),
            ));
        }
        let end = self.end();
        if end.x >= PASTURE_SIZE || end.y >= PASTURE_SIZE {
            return Err(generic_err(format!(
                "Herd at {} isn't contained in the pasture",
                self.coords,
            )));
        }

        Ok(())
    }
}

/// This answers the questions of whether two segments of the integer space intersect.
///
/// Start and end are inclusive.
fn ranges_intersect(s1: u8, e1: u8, s2: u8, e2: u8) -> bool {
    s1 >= s2 && s1 <= e2 || s1 < s2 && e1 >= s2
}

/// Coordinates
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Display, PartialEq, JsonSchema)]
#[display(fmt = "({}, {})", x, y)]
pub struct Coords {
    /// x-coordinate of northwest sheep
    x: u8,
    /// y-coordinate of northwest sheep
    y: u8,
}

/// Orientation of a herd
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Orientation {
    /// east to west
    Horizontal,
    /// north to south
    Vertical,
}
