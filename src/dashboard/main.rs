#![windows_subsystem = "windows"]
use std::{ffi::OsStr, io::BufRead, sync::{Arc, Mutex}};
use openlb::LBThresholds;
use slint::{Model, ModelRc, SharedString};
use sysinfo::System;
use settings::{LBSettings, Reaction};
use auto_launch::*;

#[path = "../settings.rs"]
mod settings;

slint::include_modules!();
fn main () {
    let configdir = format!("{}/LustBlock/", dirs_next::config_dir().unwrap().to_str().unwrap());
    std::fs::create_dir_all(&configdir).unwrap();
    println!("Loading Config from config.bin...");
    let config: LBSettings = bincode::decode_from_std_read(&mut std::fs::File::open(format!("{}config.bin", configdir).as_str()).unwrap(), bincode::config::standard()).unwrap();

    println!("Lustblock Dashboard started.");
    let s = System::new_all();
    let ui = Dashboard::new().unwrap();
    check_service(s, &ui);
    Status::get(&ui).on_start_service({
        let ui_handle = ui.as_weak();
        move || {
            println!("Starting lb-service.exe...");
            std::process::Command::new("cmd").args(["/C", "start ./lb-service.exe "]).spawn().unwrap();
            let ui = ui_handle.unwrap();  
            let s = System::new_all();
            check_service(s, &ui);
        }
    });
    Status::get(&ui).on_poll_service({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            let s = System::new_all();
            check_service(s, &ui);
        }
    });
    Sensitivity::get(&ui).set_human(convert_threstype_to(config.human_thres));
    Sensitivity::get(&ui).set_overall(convert_threstype_to(config.overall_thres));
    Sensitivity::get(&ui).set_segment(convert_threstype_to(config.segment_thres));
    Status::get(&ui).set_logs(get_last_lines_of_log(&configdir));
    Reactions::get(&ui).set_default(
        SharedString::from(match config.def_reaction {
            Reaction::Allow => "Allowed",
            Reaction::Deny => "Denied",
            Reaction::CleanLevel(openlb::CleanLevel::Human) => "Human-Scan",
            Reaction::CleanLevel(openlb::CleanLevel::OriginalLB) => "Combination-Scan",
            Reaction::CleanLevel(openlb::CleanLevel::Overall) => "Image-Scan",
            _ => { panic!("Not possible"); },
        })
    );
    let mut items: Vec<WebReaction> = Vec::new();
    for (website, reaction) in config.web_reactions.iter() {
        items.push(WebReaction {website: website.into(), aireaction: match reaction {
            Reaction::Allow => SharedString::from("Allowed"),
            Reaction::Deny => SharedString::from("Denied"),
            Reaction::CleanLevel(openlb::CleanLevel::Human) => SharedString::from("Human-Scan"),
            Reaction::CleanLevel(openlb::CleanLevel::OriginalLB) => SharedString::from("Combination-Scan"),
            Reaction::CleanLevel(openlb::CleanLevel::Overall) => SharedString::from("Image-Scan"),
            _ => { panic!("Not possible"); },
        }});
    }
    Reactions::get(&ui).set_list_items(std::rc::Rc::new(slint::VecModel::from(items)).into());
    #[cfg(not(target_os = "windows"))]
    AdvancedSettings::get(&ui).set_windows(false.into());
    AdvancedSettings::get(&ui).set_autoconnect(config.autoconnect);
    let ipsplit = config.ip.split(".").collect::<Vec<&str>>();
    AdvancedSettings::get(&ui).set_ip1(ipsplit[0].parse::<i32>().unwrap());
    AdvancedSettings::get(&ui).set_ip2(ipsplit[1].parse::<i32>().unwrap());
    AdvancedSettings::get(&ui).set_ip3(ipsplit[2].parse::<i32>().unwrap());
    AdvancedSettings::get(&ui).set_ip4(ipsplit[3].parse::<i32>().unwrap());
    let auto = AutoLaunchBuilder::new()
        .set_app_name("Lustblock")
        .set_app_path(&format!("{}\\lb-service.exe", std::env::current_dir().unwrap().to_str().unwrap()))
        .build()
        .unwrap();
    AdvancedSettings::get(&ui).set_startonboot({
        #[cfg(target_family = "windows")]
        {
            auto.is_enabled().unwrap()
        }
        #[cfg(not(target_family = "windows"))]
        {
            false
        }
    });
    AdvancedSettings::get(&ui).set_port(config.port as i32);
    let config = Arc::new(Mutex::new(config));
    Status::get(&ui).on_stop_service({
        let ui_handle = ui.as_weak();
        let config = config.clone();
        move || {
            let config = config.lock().unwrap();
            println!("Stopping lb-service.exe...");
            let s = System::new_all();
            for process in s.processes_by_name(OsStr::new("lb-service.exe")) {
                if process.name() == "lb-service.exe" {
                    let _ = process.kill();
                    if config.autoconnect {
                        use windows_registry::CURRENT_USER;
                        let proxy_set = CURRENT_USER.create("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Internet Settings").unwrap();
                        let _ = proxy_set.set_u32("ProxyEnable", 0);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            let ui = ui_handle.unwrap();
            let s = System::new_all();
            check_service(s, &ui);
        }
    });
    Sensitivity::get(&ui).on_apply({
        let ui_handle = ui.as_weak();
        let config = config.clone();
        move || {
            let mut config = config.lock().unwrap();
            let ui = ui_handle.unwrap();
            config.human_thres = convert_threstype_from(Sensitivity::get(&ui).get_human());
            config.overall_thres = convert_threstype_from(Sensitivity::get(&ui).get_overall());
            config.segment_thres = convert_threstype_from(Sensitivity::get(&ui).get_segment());
            if bincode::encode_into_std_write(&*config, &mut std::fs::File::create("config.bin").unwrap(), bincode::config::standard()).is_err() {
                panic!("Error writing to config.bin, please check if the file is open in another program.");
            } else {
                println!("Config saved to config.bin.");
            }
        }
    });
    Reactions::get(&ui).on_apply_default({
        let ui_handle = ui.as_weak();
        let config = config.clone();
        move || {
            let mut config = config.lock().unwrap();
            let ui = ui_handle.unwrap();
            match Reactions::get(&ui).get_default().as_str() {
                "Allowed" => { config.def_reaction = Reaction::Allow; },
                "Denied" => { config.def_reaction = Reaction::Deny; },
                "Human-Scan" => { config.def_reaction = Reaction::CleanLevel(openlb::CleanLevel::Human); },
                "Combination-Scan" => { config.def_reaction = Reaction::CleanLevel(openlb::CleanLevel::OriginalLB); },
                "Image-Scan" => { config.def_reaction = Reaction::CleanLevel(openlb::CleanLevel::Overall); },
                &_ => { panic!("Not possible"); },
            }
            if bincode::encode_into_std_write(&*config, &mut std::fs::File::create("config.bin").unwrap(), bincode::config::standard()).is_err() {
                panic!("Error writing to config.bin, please check if the file is open in another program.");
            } else {
                println!("Config saved to config.bin.");
            }
        }
    });
    Status::get(&ui).on_get_logs({
        let configdir = configdir.clone();
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            Status::get(&ui).set_logs(get_last_lines_of_log(&configdir));
            println!("Getting logs...");
        }
    });
    Reactions::get(&ui).on_add_reaction({
        let ui_handle = ui.as_weak();
        let config = config.clone();
        move |website: SharedString, aireaction: SharedString| {
            let mut config = config.lock().unwrap();
            let ui = ui_handle.unwrap();

            let mut items: Vec<WebReaction> = Reactions::get(&ui).get_list_items().iter().collect();
            items.push(WebReaction {website: website.clone(), aireaction: aireaction.clone()});
            let items_model = std::rc::Rc::new(slint::VecModel::from(items));
            Reactions::get(&ui).set_list_items(items_model.into());

            let reaction: Reaction = match &aireaction as &str {
                "Allowed" => { Reaction::Allow },
                "Denied" => { Reaction::Deny },
                "Human-Scan" => { Reaction::CleanLevel(openlb::CleanLevel::Human) },
                "Combination-Scan" => { Reaction::CleanLevel(openlb::CleanLevel::OriginalLB) },
                "Image-Scan" => { Reaction::CleanLevel(openlb::CleanLevel::Overall) },
                &_ => { panic!("Not possible"); },
            };
            config.web_reactions.insert(website.into(), reaction);
            if bincode::encode_into_std_write(&*config, &mut std::fs::File::create("config.bin").unwrap(), bincode::config::standard()).is_err() {
                panic!("Error writing to config.bin, please check if the file is open in another program.");
            } else {
                println!("Config saved to config.bin.");
            }
        }
    });
    Reactions::get(&ui).on_delete_reaction({
        let ui_handle = ui.as_weak();
        let config = config.clone();
        move |id: i32| {
            let mut config = config.lock().unwrap();
            let ui = ui_handle.unwrap();

            let mut items: Vec<WebReaction> = Reactions::get(&ui).get_list_items().iter().collect();
            config.web_reactions.remove(items[id as usize].website.as_str());
            items.remove(id as usize);
            let items_model = std::rc::Rc::new(slint::VecModel::from(items));
            Reactions::get(&ui).set_list_items(items_model.into());
        }
    });
    AdvancedSettings::get(&ui).on_apply({
        let config = config.clone();
        move |ip: SharedString, port: SharedString, autoconnect: bool, startonboot: bool| {
            let mut config = config.lock().unwrap();

            config.ip = ip.clone().into();
            config.port = port.parse::<u16>().unwrap();
            config.autoconnect = autoconnect;
            #[cfg(target_family = "windows")]
            {
                if startonboot {
                    println!("Setting lb-service.exe to start on boot.");
                    auto.enable().unwrap();
                } else {
                    auto.disable().unwrap();
                }
            }
            if bincode::encode_into_std_write(&*config, &mut std::fs::File::create("config.bin").unwrap(), bincode::config::standard()).is_err() {
                panic!("Error writing to config.bin, please check if the file is open in another program.");
            } else {
                println!("Config saved to config.bin.");
            }
        }
    });
    ui.run().unwrap();
}

fn check_service (s: System, window: &Dashboard) {
    for process in s.processes_by_name(OsStr::new("lb-service.exe")) {
        if process.name() == "lb-service.exe" {
            Status::get(&window).set_service_running(true.into());
            return;
        }
    }
    Status::get(&window).set_service_running(false.into());
    return;
}

fn convert_threstype_to (in_thres: LBThresholds) -> ModelThresholds  {
    ModelThresholds { hentai: in_thres.hentai, porn: in_thres.porn, sexy: in_thres.sexy }
}
fn convert_threstype_from (in_thres: ModelThresholds) -> LBThresholds  {
    LBThresholds { hentai: in_thres.hentai, porn: in_thres.porn, sexy: in_thres.sexy }
}

fn get_last_lines_of_log (configdir: &str) -> ModelRc<SharedString> {
    let logfile = std::fs::File::open(format!("{}logs.log", configdir).as_str()).unwrap();
    let buf = rev_buf_reader::RevBufReader::new(logfile);
    let mut items: Vec<SharedString> = buf.lines().take(25).map(|l| l.expect("Could not parse line").into()).collect::<Vec<SharedString>>();
    items.reverse();
    std::rc::Rc::new(slint::VecModel::from(items)).into()
}