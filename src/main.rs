#![windows_subsystem = "windows"]
use std::{cfg, env, io::Read, path::{Path, PathBuf}, process::Command, str::FromStr, time::Instant};
use bytes::BytesMut;
use clap::Args;
use http_body_util::{BodyExt, Full};
use http_mitm_proxy::{DefaultClient, MitmProxy};
use hyper::{header::{HeaderValue, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE}, StatusCode};
use hyper::Response;
use hyper::body::Bytes;
use tracing_subscriber::EnvFilter;
use dirs_next;
use opencv::imgcodecs::{imwrite, IMREAD_COLOR};
use opencv::core::*;
use yaml_rust2::{yaml::{self, Hash}, YamlLoader};
use flate2::read::GzDecoder;



//Structs for Sanity
#[derive(Args, Debug)]
struct ExternalCert {
    #[arg(required = false)]
    cert: PathBuf,
    #[arg(required = false)]
    private_key: PathBuf,
}

//Main Config from main.conf
struct Config {
    mode: bool,
    ip: String,
    port: u16,
    threasholds: Threasholds,
    optimizations: Optimizations,
    default_reaction: u8,
    site_reaction_settings: Hash
}

//Threasholds for each of the NSFW classes
struct Threasholds {
    porn: f64,
    sexy: f64,
    hentai: f64,
    humans: f64
}

struct Optimizations {
    cuda: bool,
    tensorrt: bool,
    threads: usize,
}


#[tokio::main]
async fn main() {

    println!("Initializing LustBlock...");
    let execpath = format!("{}/",env::current_exe().unwrap().parent().unwrap().to_str().unwrap().to_owned()).leak();
    let execdir: &str = if !cfg!(debug_assertions) {execpath} else {"./"} ;
    let configdir = format!("{}/LustBlock/", dirs_next::config_dir().unwrap().to_str().unwrap());
    std::fs::create_dir_all(&configdir).unwrap();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    //Read Config File
    let config_file: String = std::fs::read_to_string(Path::new(format!("{}config.yaml", execdir).as_str())).unwrap();
    let config_deser = &YamlLoader::load_from_str(&config_file).unwrap()[0];
    let mode: bool = if String::from_str(config_deser["mode"].as_str().unwrap()).unwrap() == "app"{true} else {false};
    if mode == true && !cfg!(target_os = "windows") {
        panic!("Client mode is only supported on Windows! Please set 'mode' to server in config.json.");
    }
    let config: Config = Config {
        mode: mode,
        //IP Address of the Proxy Server
        ip: if mode {String::from_str("127.0.0.1").unwrap()} else {String::from_str(config_deser["server_settings"]["ip"].as_str().unwrap_or("127.0.0.1")).unwrap()},
        //Port of the Proxy Server
        port: if mode {3003} else {config_deser["server_settings"]["port"].as_i64().unwrap_or(3003) as u16},
        //Prediction Threasholds
        threasholds: Threasholds {
            porn: config_deser["detect_threasholds"]["porn"].as_f64().unwrap(),
            sexy: config_deser["detect_threasholds"]["sexual"].as_f64().unwrap(),
            hentai: config_deser["detect_threasholds"]["hentai"].as_f64().unwrap(),
            humans: config_deser["detect_threasholds"]["humans"].as_f64().unwrap(),
        },
        optimizations: Optimizations {
            //Number of Threads to use 
            threads: config_deser["optimizations"]["threads"].as_i64().unwrap() as usize,
            //Use the CUDA Execution Provider?
            cuda: config_deser["optimizations"]["cuda"].as_bool().unwrap(),
            //Use the TensorRT Execution Provider?
            tensorrt: config_deser["optimizations"]["tensorrt"].as_bool().unwrap(),
        },
        default_reaction: encode_reaction(config_deser["default_reaction"].as_str().unwrap()),
        site_reaction_settings: config_deser["site_reaction_settings"].as_hash().unwrap().clone()
    };
    let public_key = std::fs::read_to_string(Path::new(format!("{}pub.crt", configdir).as_str()));
    let private_key = std::fs::read_to_string(Path::new(format!("{}priv.crt", configdir).as_str()));
    //Check if the HTTPS keys exist
    let root_cert = if !public_key.is_err() && !private_key.is_err() {
        // If so, Use existing key
        let param = rcgen::CertificateParams::from_ca_cert_pem(
            &public_key.unwrap(),
        )
        .unwrap();
        let key_pair = rcgen::KeyPair::from_pem(&private_key.unwrap()).unwrap();
        let cert = param.self_signed(&key_pair).unwrap();

        rcgen::CertifiedKey { cert, key_pair }
    } else {
        //Else, make and save them
        println!("Creating new Certficate...");
        make_root_cert(&configdir, &config)
    };
    let proxy = MitmProxy::new(
        // This is the root cert that will be used to sign the fake certificates
        Some(root_cert),
        // This is the main Session for the NSFW detector
        openlb::init_with_defaults(),
        config.site_reaction_settings,
        execdir
    );
    let client = DefaultClient::new(
        tokio_native_tls::native_tls::TlsConnector::builder()
            // You must set ALPN if you want to support HTTP/2
            .request_alpns(&["h2", "http/1.1"])
            .build()
            .unwrap(),
    );
    let server = proxy
        .bind((config.ip.clone(), config.port), move |_client_addr, req, cleaner, site_reactions, execdir| {
            let client = client.clone();
            async move {
                let uri = req.uri().clone();

                // TODO: Check what the user is sending out... 

                let (res, _upgrade) = client.send_request(req).await?;
                let uri_domain = uri.host().unwrap();
                println!("{} -> {}", uri_domain, res.status());
                let default_content_type: HeaderValue = HeaderValue::from_str("application/octet-stream").unwrap();
                let default_content_encoding: HeaderValue = HeaderValue::from_str("none").unwrap();
                //Check if the data is a image via Content-Type Header
                let content_type = res.headers().get(CONTENT_TYPE).unwrap_or_else(||{&default_content_type}).clone();
                //Check if the data is compressed via Content-Encoding Header
                let content_encoding = res.headers().get(CONTENT_ENCODING).unwrap_or_else(||{&default_content_encoding}).clone();
                //Get site domain's settings 
                let decoded_reaction = site_reactions.get(&yaml::Yaml::from_str(uri_domain));
                let reaction = if decoded_reaction.is_none() {
                    config.default_reaction
                } else {
                    encode_reaction(decoded_reaction.unwrap().as_str().unwrap())
                };
                //Grab original response HTTP version for Spoofing
                let http_v = res.version().clone();
                //Convert Body Stream into bytes
                let (mut parts, mut data) = res.into_parts();
                let mut body = BytesMut::new();
                while let Some(Ok(chunk)) = data.frame().await {
                    body.extend(chunk.into_data().unwrap());
                }
                if (content_type == "image/jpeg" || content_type == "image/png" || content_type == "image/webp") && (reaction != 3 && reaction != 4) {
                    let mut  end_stats = String::new();
                    println_buffer(&mut end_stats, "Image Detected");
                    let now = Instant::now();

                    //Process the img
                    let mut input_img: Mat = if content_encoding == "gzip" {
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
                            IMREAD_COLOR,
                        ).unwrap()
                    } else {
                        opencv::imgcodecs::imdecode(
                            &opencv::core::Mat::from_slice(&body.to_vec()).unwrap(),
                            IMREAD_COLOR,
                        ).unwrap()
                    };
                    let input_img_w: i32 = input_img.cols();
                    let input_img_h: i32 = input_img.rows();
                    if input_img_h > 60 || input_img_w > 60 {
                        if reaction == 2 {
                            input_img = cleaner.clean(&input_img, openlb::CleanLevel::OriginalLB);
                        } else if reaction == 1 {
                            input_img = cleaner.clean(&input_img, openlb::CleanLevel::Human);
                        } else if reaction == 0 {
                            input_img = cleaner.clean(&input_img, openlb::CleanLevel::Overall);
                        }
                        println_buffer(&mut end_stats, &format!("  Time: {:?}", now.elapsed()));
                    } else {
                        println_buffer(&mut end_stats, "  Image is Too Small: Allowed");
                        parts.headers.insert("LustBlock-Tagged", 0.into());
                    }
                    let mut bytes = Vector::new();
                    let _ = opencv::imgcodecs::imencode(if content_type == "image/png" {".png"} else if content_type == "image/webp" {".webp"} else {".jpg"}, &input_img, &mut bytes, &opencv::core::Vector::new());
                    body = BytesMut::from(bytes.as_slice());
                    parts.headers.insert(CONTENT_LENGTH, body.len().into());
                    //Finish the recognition by printing the buffered stats
                    print!("{}", &end_stats);
                }
                //Reconstruct and return response 
                if reaction == 4 {
                    let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::new(Bytes::from_static("YOU SHALL NOT SEE!".as_bytes())));
                    *after.version_mut() = http_v;
                    *after.status_mut() = StatusCode::FORBIDDEN;
                    Ok::<_, http_mitm_proxy::default_client::Error>(after)
                } else {
                    let mut after = Response::<Full<Bytes>>::from_parts(parts, Full::new(body.into()));
                    *after.version_mut() = http_v;
                    Ok::<_, http_mitm_proxy::default_client::Error>(after)
                }
            }
        })
        .await
        .unwrap();

    println!("HTTPS Proxy is listening on http://{}:{}", &config.ip, config.port);

    println!();

    if config.mode {
        #[cfg(target_family = "windows")]
        {
            use windows_registry::CURRENT_USER;
            let proxy_set = CURRENT_USER.create("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Internet Settings").unwrap();
            let _ = proxy_set.set_string("ProxyServer", &format!("{}:{}", &config.ip, config.port));
            let _ = proxy_set.set_u32("ProxyEnable", 1);
            println!("Proxy added to Computer Network Stack");
            println!("Enjoy a (sexual) temptation-less internet!");
        }
    } else {
        if !cfg!(target_os = "windows") {
            println!("Trust the pub.crt certificate if you want to use HTTPS on the device");
        }
        println!("Use the URL above in your proxy settings to apply ");
        println!("Enjoy a (sexual) temptation-less internet!");
    }

    /*
        Debug Test if you wish to not change your proxy settings in Windows
        curl -i https://news.clark.edu/wp-content/uploads/2024/10/20241003-8P5A5738-1-1024x683.jpg --proxy 127.0.0.1:3003 --cacert C:\Users\{Username}\AppData\Roaming\LustBlock\pub.crt --ssl-revoke-best-effort
    */
    #[cfg(target_os = "windows")]
    if config.mode {
        unsafe{ register_close_handler(); }
    }
    server.await;

}

//App-based Closing Handler to stop Proxy
#[cfg(target_os = "windows")]
#[link(name = "close", kind = "static")]
extern "C" {
    fn register_close_handler();
}

//Create Certificate for MITM
fn make_root_cert(configdir: &String, config: &Config) -> rcgen::CertifiedKey {
    let mut param = rcgen::CertificateParams::default();

    param.distinguished_name = rcgen::DistinguishedName::new();
    param.distinguished_name.push(
        rcgen::DnType::CommonName,
        rcgen::DnValue::Utf8String("LustBlock MITM cert".to_string()),
    );
    param.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    param.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);

    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = param.self_signed(&key_pair).unwrap();
    let _ = std::fs::write(Path::new(format!("{}pub.crt", configdir).as_str()), cert.pem());
    let _ = std::fs::write(Path::new(format!("{}priv.crt", configdir).as_str()), key_pair.serialize_pem());
    if config.mode {
        if cfg!(target_os = "windows") { 
            println!("Adding to Root Certificate Store");
            println!("{:?}", Command::new("cmd")
            .args(["/C", format!("certutil -user -addstore Root {}pub.crt", configdir).as_str()])
            .output()
            .expect("failed to execute process"));
        }
    }
    rcgen::CertifiedKey { cert, key_pair }
}

//Decodes reaction settings from String to u8 
fn encode_reaction (input: &str) -> u8 {
    let trimmed_input = input.trim();
    if trimmed_input == "hc" {
        2
    } else if trimmed_input == "h" {
        1
    } else if trimmed_input == "c" {
        0
    } else if trimmed_input == "w" {
        3
    } else if trimmed_input == "b" {
        4
    } else {
        panic!("One of the reaction values is impossible to fullfil.");
    }
}

//Adds on to console buffer to be released all at once for comprehensible output 
fn println_buffer (buffer: &mut String, print: &str) {
    buffer.push_str(print);
    buffer.push_str("\n");
}