use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{Coords, Pasture};

/// Initialization doesn't take any parameters
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Start a game
    NewGame { name: String },
    /// Player joins the arena and sets a username and random password.
    Join {
        pasture: Pasture,
        credentials: Credentials,
    },
    /// Shoot at enemy pasture
    Shoot {
        coords: Coords,
        credentials: Credentials,
    },
    /// confirm the shot made by the previous player
    Confirm {
        coords: Coords,
        credentials: Credentials,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Get a description of my pasture
    MyPasture { credentials: Credentials },
    /// Get the list of shots that I've made so far, and which ones have hit enemy sheep.
    MyShots { credentials: Credentials },
    /// Get the coordinate of the last shot made by the opponent
    LastShot { credentials: Credentials },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Credentials {
    pub game: String,
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Shots {
    pub hits: Vec<Coords>,
    pub misses: Vec<Coords>,
}
