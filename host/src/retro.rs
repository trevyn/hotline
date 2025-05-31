use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{env, fs, io::Write};

#[derive(Serialize)]
struct ApiRequest {
    width: u32,
    height: u32,
    prompt: String,
    num_images: u32,
    prompt_style: &'static str,
}

#[derive(Deserialize, Debug)]
struct ApiResponse {
    created_at: Option<u64>,
    credit_cost: Option<u32>,
    base64_images: Option<Vec<String>>,
    #[serde(rename = "type")]
    response_type: Option<String>,
    remaining_credits: Option<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key =
        env::var("RETRO_API_KEY").expect("RETRO_API_KEY environment variable must be set");

    let prompt = env::args()
        .nth(1)
        .unwrap_or_else(|| "A really cool corgi".to_string());

    let client = Client::new();
    let payload = ApiRequest {
        width: 48,
        height: 48,
        prompt: prompt.clone(),
        prompt_style: "rd_plus__topdown_asset",
        num_images: 1,
    };

    println!("requesting image for prompt: {}", prompt);

    let response = client
        .post("https://api.retrodiffusion.ai/v1/inferences")
        .header("X-RD-Token", api_key)
        .json(&payload)
        .send()?;

    if !response.status().is_success() {
        return Err(format!("api error: {}", response.text()?).into());
    }

    let raw_response = response.text()?;
    println!("raw response: {}", raw_response);

    let api_response: ApiResponse = serde_json::from_str(&raw_response)?;

    println!("api response: {:#?}", api_response);

    if let Some(credits) = api_response.remaining_credits {
        println!("credits remaining: {}", credits);
    }

    let images = api_response.base64_images.unwrap_or_default();
    for (i, base64_image) in images.iter().enumerate() {
        let image_data = STANDARD.decode(base64_image)?;

        let timestamp = api_response.created_at.unwrap_or(0);
        let filename = format!("retro_{}_{}.png", timestamp, i);
        let mut file = fs::File::create(&filename)?;
        file.write_all(&image_data)?;

        println!("saved image: {}", filename);
    }

    Ok(())
}
