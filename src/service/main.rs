//#![windows_subsystem = "windows"]
use std::{cfg, io::Read, path::{Path, PathBuf}, time::Instant};
use bytes::BytesMut;
use clap::Args;
use data::Data;
use http_body_util::{BodyExt, Full};
use http_mitm_proxy::{moka::sync::Cache, DefaultClient, MitmProxy};
use hyper::{header::{HeaderValue, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE}, service::service_fn, StatusCode};
use hyper::Response;
use hyper::body::Bytes;
use dirs_next;
use opencv::imgcodecs::IMREAD_UNCHANGED;
use opencv::core::*;
use flate2::read::GzDecoder;
use settings::{LBSettings, Reaction};
use ftail::Ftail;
use log::{{info, debug}, LevelFilter};
use openlb::img_filter::*;

mod data;
#[path = "../settings.rs"]
mod settings;

//Structs for Sanity
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

    //Read Config File
    let public_key = std::fs::read(Path::new(format!("{}pub.crt", configdir).as_str()));
    let private_key = std::fs::read_to_string(Path::new(format!("{}priv.crt", configdir).as_str()));
    let config: LBSettings = bincode::decode_from_std_read(&mut std::fs::File::open(format!("{}config.bin", configdir).as_str()).unwrap(), bincode::config::standard()).unwrap();
    //Check if the HTTPS keys exist
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
        //Else, make and save them
        debug!("Creating new Certficate...");
        make_root_cert(&configdir)
    };
    let der = root_cert.cert.der().clone();
    let cleaner = ImgCleaner::init(Some(config.human_thres), Some(config.overall_thres), None);
    debug!("Warming Up AI Models...");
    cleaner.warmup(10);
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
                    info!("{} -> {}", uri_domain, res.status());
                    let default_content_type: HeaderValue = HeaderValue::from_str("application/octet-stream").unwrap();
                    let default_content_encoding: HeaderValue = HeaderValue::from_str("none").unwrap();
                    //Check if the data is a image via Content-Type Header
                    let content_type = res.headers().get(CONTENT_TYPE).unwrap_or_else(||{&default_content_type}).clone();
                    //Check if the data is compressed via Content-Encoding Header
                    let content_encoding = res.headers().get(CONTENT_ENCODING).unwrap_or_else(||{&default_content_encoding}).clone();
                    //Get site domain's settings 
                    let decoded_reaction = site_reactions.get(uri_domain);
                    let reaction = if decoded_reaction.is_none() {
                        config.def_reaction
                    } else {
                        *decoded_reaction.unwrap()
                    };
                    //Grab original response HTTP version for Spoofing
                    let http_v = res.version().clone();
                    //Convert Body Stream into bytes
                    let (mut parts, mut data) = res.into_parts();
                    //Provide LBProxy PAC File
                    // if uri.path().contains("lbserver.pac") {
                    //     println!("LBProxy PAC File Requested");
                    //     let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::<Bytes>::from("function FindProxyForURL(url, host) { alert('PAC file executed for URL: ' + url + ' and host: ' + host);return 'PROXY 127.0.0.1:3003; DIRECT';}"));
                    //     *after.version_mut() = http_v;
                    //     return Ok::<_, http_mitm_proxy::default_client::Error>(after);
                    // }
                    //Scan image if it is an image
                    if (content_type == "image/jpeg" || content_type == "image/png" || content_type == "image/webp" || content_type == "image/tiff") && (reaction != Reaction::Allow && reaction != Reaction::Deny) {
                        let mut body = BytesMut::new();
                        while let Some(Ok(chunk)) = data.frame().await {
                            body.extend(chunk.into_data().unwrap());
                        }
                        let mut  end_stats = String::new();
                        println_buffer(&mut end_stats, "Image Detected");

                        //Process the img
                        let input_img: Mat = if content_encoding == "gzip" {
                            println_buffer(&mut end_stats, "  Compression");
                            let mut stor: Vec<u8> = Vec::new();
                            let indat = &body.to_vec()[..];
                            let mut decoder = GzDecoder::new(indat);
                            let decode_result = decoder.read_to_end(&mut stor);
                            if decode_result.is_err() {
                                panic!("Compressed Image is not extractable! File an issue pls.");
                            }
                            opencv::imgcodecs::imdecode(
                                &opencv::core::Mat::from_slice(&stor).unwrap(),
                                IMREAD_UNCHANGED,
                            ).unwrap()
                        } else {
                            opencv::imgcodecs::imdecode(
                                &opencv::core::Mat::from_slice(&body.to_vec()).unwrap(),
                                IMREAD_UNCHANGED,
                            ).unwrap()
                        };
                        let now = Instant::now();
                        let mut output_img: Option<Mat> = None;
                        if input_img.rows() > config.smallest_scan || input_img.cols() > config.smallest_scan {
                            if reaction == Reaction::Combination {
                                output_img = cleaner.clean_mat(&input_img, ImgCleanLevel::Human);
                                if output_img.is_none() {
                                    output_img = cleaner.clean_mat(&input_img, ImgCleanLevel::Overall);
                                }
                            } else if reaction == Reaction::Human {
                                output_img = cleaner.clean_mat(&input_img, ImgCleanLevel::Human);
                            } else if reaction == Reaction::Overall {
                                output_img = cleaner.clean_mat(&input_img, ImgCleanLevel::Overall);
                            }
                            println_buffer(&mut end_stats, &format!("  Time: {:?}", now.elapsed()));
                        } else {
                            println_buffer(&mut end_stats, "  Image is Too Small: Allowed");
                            parts.headers.insert("LBProxy-Tagged", 0.into());
                        }
                        if output_img.is_some() {
                            let mut bytes = Vector::new();
                            let _ = opencv::imgcodecs::imencode(if content_type == "image/png" {".png"} else if content_type == "image/webp" {".webp"} else {".jpg"}, &output_img.unwrap(), &mut bytes, &opencv::core::Vector::new());
                            parts.headers.insert(CONTENT_LENGTH, bytes.len().into());
                            //Finish the recognition by printing the buffered stats
                            body = BytesMut::from(bytes.as_slice());
                            info!("{}", &end_stats);
                        }
                        let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::new(body.into()));
                        *after.version_mut() = http_v;
                        return Ok::<_, http_mitm_proxy::default_client::Error>(after);
                    }
                    //Reconstruct and return response 
                    if reaction == Reaction::Deny {
                        let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::<Bytes>::from("YOU SHALL NOT SEE!"));
                        *after.version_mut() = http_v;
                        *after.status_mut() = StatusCode::FORBIDDEN;
                        Ok::<_, http_mitm_proxy::default_client::Error>(after)
                    } else {
                        let mut body = BytesMut::new();
                        while let Some(Ok(chunk)) = data.frame().await {
                            body.extend(chunk.into_data().unwrap());
                        }
                        let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::new(body.into()));
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
            if proxy_register.is_err() {
                install_cert(&der);
            } else if proxy_register.unwrap() != proxy_url {
                install_cert(&der);
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

//Create Certificate for MITM
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


//Adds on to console buffer to be released all at once for comprehensible output 
fn println_buffer (buffer: &mut String, print: &str) {
    buffer.push_str(print);
    buffer.push_str("\n");
}
fn install_cert(der: &[u8]) {
    #[cfg(target_family = "windows")]
    {  
        info!("Adding to Root Certificate Store");
        use windows::Win32::Security::Cryptography::*;
        unsafe {
            let store: HCERTSTORE = CertOpenSystemStoreA(None, windows::core::PCSTR::from_raw("ROOT".as_ptr())).unwrap();
            CertAddEncodedCertificateToStore(store, X509_ASN_ENCODING | PKCS_7_ASN_ENCODING, &der, CERT_STORE_ADD_NEWER, None).unwrap();
            CertCloseStore(store, 0).unwrap();
        }
    }
}