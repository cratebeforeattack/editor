mod material;
mod markup;

pub use material::*;
pub use markup::*;
use serde_derive::{Deserialize, Serialize};


#[derive(Deserialize, Clone)]
pub struct MapLook {
    #[serde(default = "MapLook::default_background_color")]
    pub background_color: [u8; 3],

    #[serde(default = "MapLook::default_water_opacity")]
    pub water_opacity: u8,

    #[serde(default = "MapLook::default_water_color_outline")]
    pub water_color_outline: [u8; 3],

    #[serde(default = "MapLook::default_water_color_shallow")]
    pub water_color_shallow: [u8; 3],

    #[serde(default = "MapLook::default_water_color_deep")]
    pub water_color_deep: [u8; 3],

    /// Areas where pawn spawn is prohibited
    #[serde(default)]
    pub no_player_spawn: Vec<[i32; 4]>,

    #[serde(default)]
    pub markup: Option<MapMarkup>,
}

impl MapLook {
    fn default_background_color() -> [u8; 3] {
        [0, 0, 0]
    }
    fn default_water_opacity() -> u8 {
        127
    }
    fn default_water_color_shallow() -> [u8; 3] {
        [0, 0, 64]
    }
    fn default_water_color_deep() -> [u8; 3] {
        [0, 0, 64]
    }
    fn default_water_color_outline() -> [u8; 3] {
        [76, 76, 255]
    }
}

impl Default for MapLook {
    fn default() -> MapLook {
        MapLook {
            background_color: Self::default_background_color(),
            water_opacity: Self::default_water_opacity(),
            water_color_shallow: Self::default_water_color_shallow(),
            water_color_deep: Self::default_water_color_deep(),
            water_color_outline: Self::default_water_color_outline(),
            no_player_spawn: Vec::new(),
            markup: None,
        }
    }
}