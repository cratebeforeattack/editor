mod markup;
mod material;

pub use markup::*;
pub use material::*;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct MapJson {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<[u8; 3]>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water_opacity: Option<u8>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water_color_outline: Option<[u8; 3]>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water_color_shallow: Option<[u8; 3]>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub water_color_deep: Option<[u8; 3]>,

    /// Areas where pawn spawn is prohibited
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub no_player_spawn: Vec<[i32; 4]>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markup: Option<MapMarkup>,
}

impl Default for MapJson {
    fn default() -> MapJson {
        MapJson {
            background_color: None,
            water_opacity: None,
            water_color_shallow: None,
            water_color_deep: None,
            water_color_outline: None,
            no_player_spawn: Vec::new(),
            markup: None,
        }
    }
}
