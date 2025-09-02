use crate::tile::Tile;
#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardTile {
    pub tile: Tile,
    pub shanten_diff: i8,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct DrawTile {
    pub tile: Tile,
    pub count: u8,
    pub shanten_diff: i8,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct RequiredTile {
    pub tile: Tile,
    pub count: u8,
}
