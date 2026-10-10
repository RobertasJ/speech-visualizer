mod request;
mod response;

use reqwest::{Client, StatusCode};

use request::Request;
use response::Response;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("TYPESAFE_API_KEY not set")]
    MissingKey,
    #[error("failed to send request: {0}")]
    Send(reqwest::Error),
    #[error("request failed with status {status}: {body}")]
    Status { status: StatusCode, body: String },
    #[error("failed to parse response: {0}")]
    Parse(reqwest::Error),
}

pub async fn send(request: &Request) -> Result<Response, Error> {
    let auth = std::env::var("TYPESAFE_API_KEY").map_err(|_| Error::MissingKey)?;

    let response = Client::new()
        .post("https://api.typesafe.ai/v1/systemone")
        .bearer_auth(auth)
        .json(request)
        .send()
        .await
        .map_err(Error::Send)?;

    let status = response.status();
    if !status.is_success() {
        // The body usually says what was wrong with the request.
        let body = response.text().await.unwrap_or_default();
        return Err(Error::Status { status, body });
    }

    response.json::<Response>().await.map_err(Error::Parse)
}
