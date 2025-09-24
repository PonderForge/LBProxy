use bincode::{Decode, Encode};
use openlb::img_filter::ImgThresholds;
use std::collections::HashMap;
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct LBSettings {
    pub port: u16,
    pub ip: String,
    pub human_thres: ImgThresholds,
    pub overall_thres: ImgThresholds,
    pub autoconnect: bool,
    pub def_reaction: Reaction,
    pub web_reactions: HashMap<String, Reaction>,
    pub smallest_scan: i32,
}

#[derive(Encode, Decode, PartialEq, Copy, Clone, Debug)]
pub enum Reaction {
    Human,
    Overall,
    Combination,
    Allow,
    Deny
}