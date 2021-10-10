use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub enum BuiltinMaterial {
    Steel,
    Ice,
    Grass,
    Mat,
    Bumper,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MaterialJson {
    pub map_rect: [i32; 4],
    pub slots: Vec<MaterialSlot>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Material {
    pub fill_color: [u8; 3],
    pub outline_color: [u8; 3],
    pub custom_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum MaterialSlot {
    None,
    BuiltIn(BuiltinMaterial),
    Custom(Material),
}

impl MaterialSlot {
    pub fn to_material(&self) -> Option<Material> {
        match self {
            MaterialSlot::None => None,
            MaterialSlot::BuiltIn(mat) => Some(mat.to_material()),
            MaterialSlot::Custom(mat) => Some(mat.clone()),
        }
    }
}

impl BuiltinMaterial {
    fn to_material(&self) -> Material {
        match self {
            BuiltinMaterial::Steel => Material {
                fill_color: [30, 34, 41],
                outline_color: [78, 92, 106],
                custom_name: String::new(),
            },
            BuiltinMaterial::Ice => Material {
                fill_color: [27, 73, 107],
                outline_color: [136, 182, 242],
                custom_name: String::new(),
            },
            BuiltinMaterial::Grass => Material {
                fill_color: [16, 104, 51],
                outline_color: [60, 173, 11],
                custom_name: String::new(),
            },
            BuiltinMaterial::Mat => Material {
                fill_color: [90, 90, 90],
                outline_color: [190, 190, 190],
                custom_name: String::new(),
            },
            BuiltinMaterial::Bumper => Material {
                fill_color: [58, 28, 12],
                outline_color: [223, 117, 11],
                custom_name: String::new(),
            },
        }
    }
}

impl MaterialSlot {
    pub fn label(&self) -> &str {
        match self {
            MaterialSlot::None => "None",
            MaterialSlot::BuiltIn(builtin) => match builtin {
                BuiltinMaterial::Steel => "Steel",
                BuiltinMaterial::Ice => "Ice",
                BuiltinMaterial::Grass => "Grass",
                BuiltinMaterial::Mat => "Mat",
                BuiltinMaterial::Bumper => "Bumper",
            },
            MaterialSlot::Custom(material) => &material.custom_name,
        }
    }
}
