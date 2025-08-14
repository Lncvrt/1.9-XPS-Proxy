use bytes::Bytes;
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::{env, fs, sync::Arc, process::Command};
use tokio::sync::Mutex;
use warp::{http::StatusCode, Filter, Rejection, Reply};

const GDPS: &str = "https://xps.lncvrt.xyz";

fn encode_gjp(password: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("{password}mI29fmAnxgTs"));
    format!("{:x}", hasher.finalize())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let debug = env::var("debug").unwrap_or_default() == "true";

    let gjp = if let Ok(val) = env::var("gjp") {
        val
    } else if let Ok(password) = env::var("password") {
        let encoded = encode_gjp(&password);
        let _ = fs::write(".env", format!("gjp={}", &encoded));
        encoded
    } else {
        println!("You need to (refresh) login in order to use XPS 1.9!\nLogin: Gear Icon => Account\nRefresh: Gear Icon => Account => More => Refresh Login");
        "null".to_string()
    };

    if fs::metadata("new").is_ok() {
        let _ = fs::remove_file("new");
        let url = "http://localhost:4815";
        let opener = if cfg!(target_os = "windows") {
            "start"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let _ = Command::new(opener).arg(url).spawn();
    }

    let state = AppState {
        client: Client::new(),
        gjp: Arc::new(Mutex::new(gjp)),
        debug,
    };

    let state_filter = warp::any().map(move || state.clone());

    let post_route = warp::post()
        .and(warp::path::full())
        .and(warp::body::bytes())
        .and(state_filter.clone())
        .and_then(handle_post);

    let get_route = warp::get()
        .and(warp::path::full())
        .map(|_path| warp::redirect::temporary(warp::http::Uri::from_static(GDPS)));

    let routes = post_route.or(get_route);

    println!("XPS compatibility server started!");
    warp::serve(routes).run(([127, 0, 0, 1], 4815)).await;
}

#[derive(Clone)]
struct AppState {
    client: Client,
    gjp: Arc<Mutex<String>>,
    debug: bool,
}

async fn handle_post(
    full_path: warp::filters::path::FullPath,
    body: Bytes,
    state: AppState,
) -> Result<impl Reply, Rejection> {
    let path = full_path.as_str().to_string();
    let body_str = String::from_utf8_lossy(&body);

    let mut parsed: Vec<(String, String)> = body_str
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            Some((
                urlencoding::decode(parts.next().unwrap_or_default()).ok()?.to_string(),
                urlencoding::decode(parts.next().unwrap_or_default()).ok()?.to_string(),
            ))
        })
        .collect();

    let mut user_name = None;
    let mut password_opt = None;

    for (k, v) in &parsed {
        if k == "userName" {
            user_name = Some(v.clone());
        }
        if k == "password" {
            password_opt = Some(v.clone());
        }
    }

    if path.ends_with("loginGJAccount.php") {
        if let Some(pass) = &password_opt {
            parsed.push(("gjp2".to_string(), encode_gjp(pass)));
        }
    } else if parsed.iter().any(|(k, _)| k == "accountID") {
        let gjp_val = state.gjp.lock().await.clone();
        parsed.push(("gjp2".to_string(), gjp_val));
    }

    if state.debug {
        println!("[{}]\n{}", path, body_str);
    }

    let post_body = serde_urlencoded::to_string(&parsed).unwrap_or_default();

    let res = state
        .client
        .post(format!("{GDPS}{path}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("User-Agent", "")
        .body(post_body)
        .send()
        .await;

    match res {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_else(|_| "-1".to_string());

            if path.ends_with("loginGJAccount.php") && !text.starts_with('-') {
                if let Some(pass) = password_opt {
                    let encoded = encode_gjp(&pass);
                    let mut lock = state.gjp.lock().await;
                    *lock = encoded.clone();
                    let _ = fs::write(".env", format!("gjp={}", &encoded));
                    if let Some(name) = user_name {
                        println!("Logged in as {}!", name);
                    }
                }
            }

            Ok(warp::reply::with_status(text, status))
        }
        Err(e) => {
            eprintln!("Request error: {:?}", e);
            Ok(warp::reply::with_status("-1".to_string(), StatusCode::OK))
        }
    }
}
