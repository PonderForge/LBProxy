use bincode::{Decode, Encode};
use openlb::{LBThresholds, CleanLevel};
use std::collections::HashMap;
#[derive(Encode, Decode, PartialEq, Debug)]
pub struct LBSettings {
    pub port: u16,
    pub ip: String,
    pub human_thres: LBThresholds,
    pub overall_thres: LBThresholds,
    pub segment_thres: LBThresholds,
    pub autoconnect: bool,
    pub def_reaction: Reaction,
    pub web_reactions: HashMap<String, Reaction>,
}

#[derive(Encode, Decode, PartialEq, Copy, Clone, Debug)]
pub enum Reaction {
    CleanLevel(CleanLevel),
    Allow,
    Deny
}