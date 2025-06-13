hotline::object!({
    #[derive(Default)]
    pub struct AnthropicClient {
        api_key: Option<String>,
        #[setter]
        response_target: Option<ChatInterface>,
    }

    impl AnthropicClient {
        pub fn initialize(&mut self) {
            // Get API key from environment
            self.api_key = std::env::var("ANTHROPIC_API_KEY").ok();
            if self.api_key.is_none() {
                eprintln!("Warning: ANTHROPIC_API_KEY environment variable not set");
            }
        }

        pub fn send_message(&mut self, user_message: String) {
            use serde::{Deserialize, Serialize};

            #[derive(Serialize)]
            struct AnthropicMessage {
                role: String,
                content: String,
            }

            #[derive(Serialize)]
            struct AnthropicRequest {
                model: String,
                max_tokens: u32,
                messages: Vec<AnthropicMessage>,
            }

            #[derive(Deserialize, Debug)]
            struct AnthropicResponse {
                id: String,
                #[serde(rename = "type")]
                response_type: String,
                role: String,
                content: Vec<AnthropicContent>,
                model: String,
                stop_reason: Option<String>,
                stop_sequence: Option<String>,
                usage: Usage,
            }

            #[derive(Deserialize, Debug)]
            struct AnthropicContent {
                #[serde(rename = "type")]
                content_type: String,
                text: String,
            }

            #[derive(Deserialize, Debug)]
            struct Usage {
                input_tokens: u32,
                output_tokens: u32,
            }
            let api_key = match &self.api_key {
                Some(key) => key,
                None => {
                    self.send_response("Error: ANTHROPIC_API_KEY not set".to_string());
                    return;
                }
            };

            // Create the request
            let messages = vec![AnthropicMessage { role: "user".to_string(), content: user_message }];

            let request =
                AnthropicRequest { model: "claude-3-5-sonnet-20241022".to_string(), max_tokens: 1024, messages };

            // Clone what we need for the async task
            let api_key_clone = api_key.clone();
            let response_target = self.response_target.clone();

            // Spawn async task on hotline runtime
            ::hotline::hotline_runtime().spawn(async move {
                // Make async API call
                let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap();

                let response = client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", api_key_clone)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&request)
                    .send()
                    .await;

                let response_text = match response {
                    Ok(resp) => {
                        // Check status first
                        let status = resp.status();
                        if !status.is_success() {
                            let error_text = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                            format!("API error ({}): {}", status, error_text)
                        } else {
                            // Try to get response text for debugging
                            let response_text =
                                resp.text().await.unwrap_or_else(|e| format!("Failed to get text: {}", e));

                            // Try to parse the response
                            match serde_json::from_str::<AnthropicResponse>(&response_text) {
                                Ok(api_resp) => {
                                    if let Some(content) = api_resp.content.first() {
                                        content.text.clone()
                                    } else {
                                        "Error: Empty response from API".to_string()
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to parse response: {}", e);
                                    eprintln!("Response was: {}", response_text);
                                    format!("Error parsing response: {}", e)
                                }
                            }
                        }
                    }
                    Err(e) => {
                        format!("Error making request: {}", e)
                    }
                };

                // Send response back to target
                if let Some(mut target) = response_target {
                    target.receive_llm_response(response_text);
                }
            });
        }

        fn send_response(&mut self, response: String) {
            if let Some(ref mut target) = self.response_target {
                target.receive_llm_response(response);
            }
        }
    }
});
