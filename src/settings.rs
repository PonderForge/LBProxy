use bincode::{Decode, Encode};
use openlb::img_filter::ImgThresholds;
use std::{collections::HashMap, env::current_exe};
use std::path::{PathBuf, Path};

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct LBSettings {
    pub port: u16,
    pub ip: String,
    pub human_thres: ImgThresholds,
    pub overall_thres: ImgThresholds,
    pub autoconnect: bool,
    pub def_reaction: Reaction,
    pub web_reactions: HashMap<String, Reaction>,
    pub smallest_scan: u32,
    pub human_min_scan: u32
}

#[derive(Encode, Decode, PartialEq, Copy, Clone, Debug)]
pub enum Reaction {
    Human,
    Overall,
    Combination,
    Allow,
    Deny
}

#[cfg(target_family = "unix")] 
pub fn from_exe_dir (file: &str) -> PathBuf {
    #[cfg(debug_assertions)]
    return std::env::current_dir().unwrap().join(Path::new(file));
    #[cfg(not(debug_assertions))]
    return current_exe().unwrap().parent().unwrap().join(file);
}

#[cfg(target_family = "windows")]
pub fn from_exe_dir (file: &str) -> PathBuf {
    #[cfg(debug_assertions)]
    return std::env::current_dir().unwrap().join(Path::new(file));
    #[cfg(not(debug_assertions))]
    return current_exe().unwrap().parent().unwrap().join(file);
}