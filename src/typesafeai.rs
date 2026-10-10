mod request;
mod response;

use reqwest::Client;
use serde_json::json;

use request::Request;

use crate::typesafeai::{
    request::{Choice, Noul, Score},
    response::Response,
};

pub async fn request() {
    let auth = std::env::var("TYPESAFE_API_KEY").expect("TYPESAFE_API_KEY not set");

    let request = Request::new(json!({
        "user": {
            "name": "John Doe",
            "age": 30,
            "job": "Unemployable"
        }
    }))
    .with_question(
        "english_user",
        Noul::new("is the user english?").with_criteria("english name", "non english name"),
    )
    .with_question(
        "age_range",
        Choice::new("what is the age range of the user?")
            .with_option("0-18", "is within the age range of 0-18")
            .with_option("19-35", "is within the age range of 19-35")
            .with_option("36-50", "is within the age range of 36-50")
            .with_option("51+", "is within the age range of 51+"),
    )
    .with_question(
        "job category",
        Choice::new("what is the job category of the user?")
            .with_option("tech", "is in the tech industry")
            .with_option("finance", "is in the finance industry")
            .with_option("healthcare", "is in the healthcare industry")
            .with_option("education", "is in the education industry")
            .with_option("other", "is in another industry"),
    )
    .with_question(
        "assumed_intelligence",
        Score::new("What is the most likely IQ range of the user?")
            .with_level("very low")
            .with_level("low")
            .with_level("average")
            .with_level("high")
            .with_level("very high"),
    );

    let client = Client::new()
        .post("https://api.typesafe.ai/v1/systemone")
        .bearer_auth(auth)
        .json(&request);

    let response = client.send().await.expect("Failed to send request");

    if !response.status().is_success() {
        panic!("Request failed with status: {}", response.status());
    }

    let response_json = response
        .json::<Response>()
        .await
        .expect("Failed to parse response");

    println!("Response: {:#?}", response_json);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Calls the live API with the key from .env: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_request() {
        dotenv::dotenv().ok();
        request().await;
    }
}
