#![windows_subsystem = "windows"]
use std::{io::{Cursor, Read}, path::{Path, PathBuf}};
use bytes::BytesMut;
use clap::Args;
use data::Data;
use http_body_util::BodyExt;
use http_mitm_proxy::{moka::sync::Cache, DefaultClient, MitmProxy};
use hyper::{StatusCode, body::{Incoming}, header::{CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue}, service::service_fn};
use hyper::Response;
use hyper::body::Bytes;
use dirs_next;
use image::{DynamicImage, ImageFormat, ImageReader};
use flate2::read::GzDecoder;
use crate::settings::{LBSettings, Reaction, from_exe_dir};
use ftail::Ftail;
use log::{{info, debug}, LevelFilter};
use openlb::img_filter::*;

mod data;
#[path = "../settings.rs"]
mod settings;

// Structs for Sanity
#[derive(Args, Debug)]
struct ExternalCert {
    #[arg(required = false)]
    cert: PathBuf,
    #[arg(required = false)]
    private_key: PathBuf,
}


#[tokio::main]
async fn main() {
    let configdir = format!("{}/LBProxy/", dirs_next::config_dir().unwrap().to_str().unwrap());
    std::fs::create_dir_all(&configdir).unwrap();
    Ftail::new()
        .console(LevelFilter::Info)
        .single_file(format!("{}logs.log", configdir).as_str(), true, LevelFilter::Info)
        .max_file_size(1)
        .init().unwrap();
    info!("Initializing LBProxy...");

    // Read Config File
    let public_key = std::fs::read(Path::new(format!("{}pub.crt", configdir).as_str()));
    let private_key = std::fs::read_to_string(Path::new(format!("{}priv.crt", configdir).as_str()));
    let config: LBSettings = bincode::decode_from_std_read(&mut std::fs::File::open(format!("{}config.bin", configdir).as_str()).unwrap(), bincode::config::standard()).unwrap();
    // Check if the HTTPS keys exist
    let root_cert = if !public_key.is_err() && !private_key.is_err() {
        // If so, use the existing keys
        let param = rcgen::CertificateParams::from_ca_cert_der(
            &public_key.unwrap().into(),
        )
        .unwrap();
        let key_pair = rcgen::KeyPair::from_pem(&private_key.unwrap()).unwrap();
        let cert = param.self_signed(&key_pair).unwrap();

        rcgen::CertifiedKey { cert, key_pair }
    } else {
        // Else, make and save them
        debug!("Creating new Certficate...");
        make_root_cert(&configdir)
    };
    let mut image_class_vec: Vec<u8> = Vec::new();
    std::fs::File::open(from_exe_dir("models/img_classifier.onnx")).unwrap().read_to_end(&mut image_class_vec).unwrap();
    let mut human_det_vec: Vec<u8> = Vec::new();
    std::fs::File::open(from_exe_dir("models/human_detector.onnx")).unwrap().read_to_end(&mut human_det_vec).unwrap();
    let cleaner = ImgCleaner::builder().with_human_detector(human_det_vec).with_image_classifier(image_class_vec).with_human_thres(config.human_thres).with_overall_thres(config.overall_thres).with_min_human_size(config.human_min_scan).commit();
    let root_der = root_cert.cert.der().clone();
    let proxy = MitmProxy::new(
        // This is the root cert that will be used to sign the fake certificates
        Some(root_cert),
        Some(Cache::new(256)),
    );
    let client = DefaultClient::new();
    let reactions_ref = Data::new(config.web_reactions);
    let cleaner_ref = Data::new(cleaner);
    let server = proxy
        .bind(
            (config.ip.clone(), config.port),
            service_fn(move |req| {
                let client = client.clone();
                let site_reactions = reactions_ref.clone();
                let cleaner = cleaner_ref.clone();
                async move {
                    let uri = req.uri().clone();
                    let (res, _upgrade) = client.send_request(req).await?;
                    let uri_domain = uri.host().unwrap();
                    let default_content_type: HeaderValue = HeaderValue::from_str("application/octet-stream").unwrap();
                    let default_content_encoding: HeaderValue = HeaderValue::from_str("none").unwrap();
                    // Check if the data is a image via Content-Type Header
                    let content_type = res.headers().get(CONTENT_TYPE).unwrap_or_else(||{&default_content_type}).clone();
                    // Check if the data is compressed via Content-Encoding Header
                    let content_encoding = res.headers().get(CONTENT_ENCODING).unwrap_or_else(||{&default_content_encoding}).clone();
                    // Grab original response HTTP version for Spoofing
                    let http_v = res.version().clone();
                    // Grab original response HTTP version for Spoofing
                    let res_code = res.status().clone();
                    // Get site domain's settings 
                    let decoded_reaction = site_reactions.get(uri_domain);
                    let reaction = if decoded_reaction.is_none() {
                        config.def_reaction
                    } else {
                        *decoded_reaction.unwrap()
                    };
                    let (mut input, output) = Incoming::channel();
                    // Convert Body Stream into bytes
                    let (mut parts, mut data) = res.into_parts();
                    // Scan image if it is an image
                    if (content_type == "image/jpeg" || content_type == "image/png" || content_type == "image/webp" || content_type == "image/tiff") && (reaction != Reaction::Allow && reaction != Reaction::Deny) {
                        let mut body = BytesMut::new();
                        while let Some(Ok(chunk)) = data.frame().await {
                            body.extend(chunk.into_data().unwrap());
                        }
                        // Process the img
                        let input_img: DynamicImage = if content_encoding == "gzip" {
                            let mut stor: Vec<u8> = Vec::new();
                            let indat = &body.to_vec()[..];
                            let mut decoder = GzDecoder::new(indat);
                            decoder.read_to_end(&mut stor).unwrap();
                            ImageReader::new(Cursor::new(&stor)).with_guessed_format().unwrap().decode().unwrap()
                        } else {
                            ImageReader::new(Cursor::new(&body.to_vec())).with_guessed_format().unwrap().decode().unwrap()
                        };
                        let mut output_img: Option<DynamicImage> = None;
                        if input_img.height() > config.smallest_scan || input_img.width() > config.smallest_scan {
                            //input_img.clone().save(format!("out/{}.jpg", rand::thread_rng().gen_range(0..500))).unwrap();
                            if reaction == Reaction::Combination {
                                output_img = cleaner.clean_image(input_img.clone(), ImgCleanLevel::Human);
                                if output_img.is_none() {
                                    output_img = cleaner.clean_image(input_img, ImgCleanLevel::Overall);
                                }
                            } else if reaction == Reaction::Human {
                                output_img = cleaner.clean_image(input_img, ImgCleanLevel::Human);
                            } else if reaction == Reaction::Overall {
                                output_img = cleaner.clean_image(input_img, ImgCleanLevel::Overall);
                            }
                        }
                        let img_code: &str = if output_img.is_some() {
                            let mut bytes: Vec<u8> = Vec::new();
                            output_img.unwrap().write_to(&mut Cursor::new(&mut bytes), if content_type == "image/png" {ImageFormat::Png} else if content_type == "image/webp" {ImageFormat::WebP} else {ImageFormat::Jpeg}).unwrap();
                            parts.headers.insert(CONTENT_LENGTH, bytes.len().into());
                            // Finish the recognition by printing the buffered stats
                            body = BytesMut::from(bytes.as_slice());
                            "NSFW"
                        } else {
                            "Clean"
                        };
                        input.try_send_data(body.freeze()).unwrap();
                        info!("{} -> {}", uri_domain, img_code);
                        return Ok::<_, http_mitm_proxy::default_client::Error>(Response::<Incoming>::from_parts(parts, output));
                    }
                    info!("{} -> {}", uri_domain, res_code);
                    //Reconstruct and return response 
                    if reaction == Reaction::Deny {
                        input.try_send_data(Bytes::from("YOU SHALL NOT SEE!")).unwrap();
                        let mut after = Response::<Incoming>::from_parts(parts, output);
                        *after.version_mut() = http_v;
                        *after.status_mut() = StatusCode::FORBIDDEN;
                        Ok::<_, http_mitm_proxy::default_client::Error>(after)
                    } else {
                        let mut after = Response::<Incoming>::from_parts(parts, data);
                        *after.version_mut() = http_v;
                        Ok::<_, http_mitm_proxy::default_client::Error>(after)
                    }
                }
            }),
        )
        .await
        .unwrap();

    #[cfg(target_family = "windows")]
    {  
        if config.autoconnect {
            use windows_registry::CURRENT_USER;
            let proxy_set = CURRENT_USER.create("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Internet Settings").unwrap();
            let proxy_url = format!("{}:{}", &config.ip, config.port);
            let proxy_register = proxy_set.get_string("ProxyServer");
            let _ = proxy_set.set_u32("ProxyEnable", 1);
            if proxy_register.is_err() || proxy_register.unwrap() != proxy_url {
                install_cert(&root_der);
            }
            let _ = proxy_set.set_string("ProxyServer", &proxy_url);
            info!("Proxy added to Computer Network Stack");
            ctrlc::set_handler(move || {
                info!("Stopping LBProxy...");
                let proxy_set = CURRENT_USER.create("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Internet Settings").unwrap();
                let _ = proxy_set.set_u32("ProxyEnable", 0);
                std::process::exit(0);
            }).expect("Error setting Ctrl-C handler");
        }
    }
    info!("LBProxy is running on {}:{}", &config.ip, config.port);
    server.await;

}

// Create Certificate for MITM
fn make_root_cert(configdir: &String) -> rcgen::CertifiedKey {
    let mut param = rcgen::CertificateParams::default();

    param.distinguished_name = rcgen::DistinguishedName::new();
    param.distinguished_name.push(
        rcgen::DnType::CommonName,
        rcgen::DnValue::Utf8String("LBMITM".to_string()),
    );
    param.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    param.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);

    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = param.self_signed(&key_pair).unwrap();
    let _ = std::fs::write(Path::new(format!("{}pub.crt", configdir).as_str()), cert.der());
    let _ = std::fs::write(Path::new(format!("{}priv.crt", configdir).as_str()), key_pair.serialize_pem());
    rcgen::CertifiedKey { cert, key_pair }
}

// fn image_type_scan (packet: &[u8]) -> bool {
//     let png = packet[0..8] == [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
//     let jpeg = packet[0..3] == [0xff, 0xd8, 0xff];
//     let webp = packet[0..4] == *b"RIFF" && packet[8..16] == *b"WEBPVP8 " ;
//     let tiff = packet[0..5] == [0x49, 0x49, 0x2A, 0x00] || packet[0..5] == [0x4D, 0x4D, 0x00, 0x2A];
//     if png || jpeg || webp || tiff {
//         return true
//     }
//     return false;
// }

#[cfg(target_family = "windows")]
fn install_cert(der: &[u8]) {
    info!("Adding to Root Certificate Store");
    use windows::Win32::Security::Cryptography::*;
    unsafe {
        let store: HCERTSTORE = CertOpenSystemStoreA(None, windows::core::PCSTR::from_raw("ROOT".as_ptr())).unwrap();
        CertAddEncodedCertificateToStore(store, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, &der, CERT_STORE_ADD_NEWER, None).unwrap();
        CertCloseStore(store, 0).unwrap();
    }
}
