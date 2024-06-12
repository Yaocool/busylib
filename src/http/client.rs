use crate::prelude::EnhancedUnwrap;

pub type ReqwestError = reqwest::Error;
pub type ReqwestClient = reqwest::Client;
pub use reqwest::ClientBuilder;
pub use reqwest::Proxy;

pub fn default_reqwest_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwp()
}

#[cfg(test)]
mod test {
    use crate::http::client::default_reqwest_client;

    #[tokio::test]
    async fn query() {
        let ip_info = default_reqwest_client()
            .get("http://cip.cc")
            .header("User-Agent", "curl")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
            // 20221222: remove special characters in response of cip.cc (IP_PROVIDER)
            .replace(['\n', '\t'], "");
        dbg!(ip_info);
    }
}
