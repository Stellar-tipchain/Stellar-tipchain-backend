#[derive(Debug)]
pub enum StellarVerifyError {
    NotFound,
    Unsuccessful,
    NetworkError(String),
}

#[derive(serde::Deserialize)]
struct HorizonTransactionResponse {
    successful: bool,
}

#[derive(Clone)]
pub struct StellarService {
    base_url: String,
    client: reqwest::Client,
}

impl StellarService {
    pub fn new(network: &str) -> Self {
        let base_url = if network == "mainnet" {
            "https://horizon.stellar.org"
        } else {
            "https://horizon-testnet.stellar.org"
        };
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn verify_transaction(&self, tx_hash: &str) -> Result<(), StellarVerifyError> {
        let url = format!("{}/transactions/{}", self.base_url, tx_hash);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| StellarVerifyError::NetworkError(e.to_string()))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Err(StellarVerifyError::NotFound);
        }
        if !status.is_success() {
            return Err(StellarVerifyError::NetworkError(status.to_string()));
        }

        let body: HorizonTransactionResponse = resp
            .json()
            .await
            .map_err(|e| StellarVerifyError::NetworkError(e.to_string()))?;

        if body.successful {
            Ok(())
        } else {
            Err(StellarVerifyError::Unsuccessful)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_successful_transaction() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/transactions/abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"successful": true})))
            .mount(&server)
            .await;

        let svc = StellarService::with_base_url(&server.uri());
        assert!(matches!(svc.verify_transaction("abc123").await, Ok(())));
    }

    #[tokio::test]
    async fn test_unsuccessful_transaction() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/transactions/abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"successful": false})))
            .mount(&server)
            .await;

        let svc = StellarService::with_base_url(&server.uri());
        assert!(matches!(
            svc.verify_transaction("abc123").await,
            Err(StellarVerifyError::Unsuccessful)
        ));
    }

    #[tokio::test]
    async fn test_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/transactions/abc123"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let svc = StellarService::with_base_url(&server.uri());
        assert!(matches!(
            svc.verify_transaction("abc123").await,
            Err(StellarVerifyError::NotFound)
        ));
    }

    #[tokio::test]
    async fn test_network_error() {
        // Use a URL that won't connect
        let svc = StellarService::with_base_url("http://127.0.0.1:1");
        assert!(matches!(
            svc.verify_transaction("abc123").await,
            Err(StellarVerifyError::NetworkError(_))
        ));
    }
}
