// Written by pixeluted (uwu)

use rand::seq::SliceRandom;
use rand::thread_rng;
use reqwest::{self, Proxy};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{error::Error, fmt, io};
use std::fs::File;
use std::io::{Write, BufRead};
use futures::future::join_all;

#[derive(Deserialize, Debug)]
struct TokenResponse {
    token: String,
}

#[derive(Debug)]
enum TokenError {
    Http(reqwest::Error),
    Parsing(reqwest::Error),
    StatusCode(reqwest::StatusCode),
}

impl Error for TokenError {}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenError::Http(e) => write!(f, "HTTP request failed: {}", e),
            TokenError::Parsing(e) => write!(f, "Response parsing failed: {}", e),
            TokenError::StatusCode(code) => write!(f, "Unexpected status code: {}", code),
        }
    }
}

impl From<reqwest::Error> for TokenError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_status() {
            TokenError::StatusCode(err.status().unwrap())
        } else if err.is_body() {
            TokenError::Parsing(err)
        } else {
            TokenError::Http(err)
        }
    }
}

fn load_proxies<P>(filename: P) -> Result<Vec<String>, Box<dyn Error>>
where 
    P: AsRef<Path>,
{
    let path = filename.as_ref();
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut proxies = Vec::new();

    for line_result in reader.lines() {
        let line = line_result?;
        proxies.push(line);
    }

    Ok(proxies)
}

async fn filter_proxies(proxies: Vec<String>, timeout_duration: Duration) -> Result<Vec<String>, Box<dyn Error>> {
    let mut tasks = vec![];

    for proxy_url in proxies {
        let task = tokio::spawn(async move {
            match Proxy::all(&proxy_url) {
                Ok(proxy) => {
                    let client = reqwest::Client::builder()
                        .proxy(proxy)
                        .build()
                        .unwrap();

                    match tokio::time::timeout(timeout_duration, client.get("https://example.com").send()).await {
                        Ok(Ok(_)) => Some(proxy_url),
                        _ => None,
                    }
                },
                Err(_) => None, 
            }
        });

        tasks.push(task);
    }

    let results = join_all(tasks).await;
    let mut fast_proxies = Vec::new();

    for result in results {
        if let Ok(Some(proxy_url)) = result {
            fast_proxies.push(proxy_url);
            if fast_proxies.len() >= 200 {
                break;
            }
        }
    }

    Ok(fast_proxies)
}

async fn generate_token(proxies: Vec<String>) -> Result<String, TokenError> {
    let mut client_builder = reqwest::Client::builder();

    if !proxies.is_empty() {
        let mut rng = thread_rng();
        let proxy = proxies.choose(&mut rng).ok_or("No proxies available").unwrap();

        client_builder = client_builder.proxy(Proxy::all(proxy)?);
    }

    let client = client_builder.build()?;

    let json_body = json!({
        "partnerUserId": "cd8dc419c91d8884804707eae4c5302cf3b6f101fa30a1f530a52b1f0af472a4"
    });

    let request_builder = client
        .post("https://api.discord.gx.games/v1/direct-fulfillment")
        .json(&json_body)
        .header("Content-Type", "application/json");


    let response = request_builder
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(TokenError::StatusCode(response.status()));
    }

    let parsed_response: TokenResponse = response.json().await?;
    Ok(parsed_response.token)
}

#[tokio::main]
async fn main() {
    let proxies_path = Path::new("proxies.txt");
    let mut proxies = vec![];
    if !proxies_path.exists() {
        println!("Proxies were not found! Progress might get slower over time!");
    } else {
        proxies = match load_proxies("proxies.txt") {
            Ok(proxies) => proxies,
            Err(e) => {
                eprintln!("Failed to load proxies: {}", e);
                return;
            }
        };
        println!("Loaded proxies!");
        println!("Filtering out proxies please wait...");
        proxies = filter_proxies(proxies, Duration::from_millis(2000)).await.unwrap();

        if proxies.is_empty() {
            println!("No proxies are fast enough to continue execution of this generator!");
            panic!();
        }

        println!("We now have {} fast proxies!", proxies.len());
    }

    let count: usize;

    println!("Please enter how many promotion links you want to generate:");
    let mut user_input = String::new();

    io::stdin().read_line(&mut user_input)
        .expect("Failed to read line");

    count = user_input.trim().parse()
        .expect("Please type a number!");

    let started_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    let generated_links = Arc::new(Mutex::new(vec![]));
    let mut tasks = vec![];

    for _ in 0..count {
        let proxies_clone = proxies.clone();
        let generated_links_clone = generated_links.clone();

        let task = tokio::spawn(async move {
            loop {
                match generate_token(proxies_clone.clone()).await {
                    Ok(promotion_token) => {
                        let promotion_link = format!("https://discord.com/billing/partner-promotions/1180231712274387115/{}", promotion_token);
                        
                        println!("Promotion link generated: {}", promotion_link);
    
                        let mut links = generated_links_clone.lock().await;
                        links.push(promotion_link);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Failed to generate token: {}", e);
                    }
                }    
            }
        });
        
        tasks.push(task);
    }

    let _ = join_all(tasks).await;
    let ended_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let generated_links = generated_links.lock().await;

    let mut exit_input = String::new();

    let mut text_file = File::create("links.txt").unwrap();

    for line in generated_links.iter() {
        let _ = writeln!(text_file, "{}", line);
    }

    println!("Sucessfully generated {} promotion link(s) in {} second(s)! Failed {} generation(s)", generated_links.len(), ended_at - started_at , count - generated_links.len());
    println!("All links have been saved into links.txt!");
    println!("Press enter to exit.");

    io::stdin().read_line(&mut exit_input).expect("Wtf");

}
