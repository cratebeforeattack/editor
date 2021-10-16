use glam::IVec2;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Tunnel {
    pub points: Vec<TunnelPoint>,
    pub edges: Vec<(usize, usize)>,
    pub value: u8,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct TunnelPoint {
    pos: IVec2,
    radius: usize,
}
