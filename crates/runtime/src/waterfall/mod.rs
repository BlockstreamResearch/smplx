mod types;

// pub struct WaterfallClient {
//     base_url: String,
//     client: reqwest::Client,
// }

// impl WaterfallClient {
//     pub fn new(base_url: impl Into<String>) -> Self {
//         Self {
//             base_url: base_url.into(),
//             client: reqwest::Client::new(),
//         }
//     }
//
//     fn url(&self, path: &str) -> String {
//         format!(
//             "{}/{}",
//             self.base_url.trim_end_matches('/'),
//             path.trim_start_matches('/')
//         )
//     }
//
//     // Waterfalls v2 endpoints (JSON)
//     pub async fn waterfalls_v2(
//         &self,
//         descriptor: &str,
//         page: Option<u32>,
//         to_index: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(WaterfallResponse, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!("v2/waterfalls?descriptor={}", urlencoding::encode(descriptor)));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if let Some(idx) = to_index {
//             url.push_str(&format!("&to_index={}", idx));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.json().await?;
//         Ok((data, headers))
//     }
//
//     pub async fn waterfalls_v2_addresses(
//         &self,
//         addresses: &[String],
//         page: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(WaterfallResponse, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!("v2/waterfalls?addresses={}", addresses.join(",")));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.json().await?;
//         Ok((data, headers))
//     }
//
//     pub async fn waterfalls_v2_utxo_only(
//         &self,
//         descriptor: &str,
//         to_index: Option<u32>,
//     ) -> Result<(WaterfallResponse, reqwest::header::HeaderMap), reqwest::Error> {
//         self.waterfalls_v2(descriptor, None, to_index, true).await
//     }
//
//     // Waterfalls v4 endpoints (JSON with extended tip metadata)
//     pub async fn waterfalls_v4(
//         &self,
//         descriptor: &str,
//         page: Option<u32>,
//         to_index: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(WaterfallResponseV4, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!("v4/waterfalls?descriptor={}", urlencoding::encode(descriptor)));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if let Some(idx) = to_index {
//             url.push_str(&format!("&to_index={}", idx));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.json().await?;
//         Ok((data, headers))
//     }
//
//     pub async fn waterfalls_v4_addresses(
//         &self,
//         addresses: &[String],
//         page: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(WaterfallResponseV4, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!("v4/waterfalls?addresses={}", addresses.join(",")));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.json().await?;
//         Ok((data, headers))
//     }
//
//     pub async fn waterfalls_v4_utxo_only(
//         &self,
//         descriptor: &str,
//         to_index: Option<u32>,
//     ) -> Result<(WaterfallResponseV4, reqwest::header::HeaderMap), reqwest::Error> {
//         self.waterfalls_v4(descriptor, None, to_index, true).await
//     }
//
//     // Waterfalls v1 endpoint (for compatibility)
//     pub async fn waterfalls_v1(
//         &self,
//         descriptor: &str,
//         page: Option<u32>,
//         to_index: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(WaterfallResponse, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!("v1/waterfalls?descriptor={}", urlencoding::encode(descriptor)));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if let Some(idx) = to_index {
//             url.push_str(&format!("&to_index={}", idx));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.json().await?;
//         Ok((data, headers))
//     }
//
//     // CBOR endpoints
//     pub async fn waterfalls_v2_cbor(
//         &self,
//         descriptor: &str,
//         page: Option<u32>,
//         to_index: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!(
//             "v2/waterfalls.cbor?descriptor={}",
//             urlencoding::encode(descriptor)
//         ));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if let Some(idx) = to_index {
//             url.push_str(&format!("&to_index={}", idx));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.bytes().await?.to_vec();
//         Ok((data, headers))
//     }
//
//     pub async fn waterfalls_v4_cbor(
//         &self,
//         descriptor: &str,
//         page: Option<u32>,
//         to_index: Option<u32>,
//         utxo_only: bool,
//     ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), reqwest::Error> {
//         let mut url = self.url(&format!(
//             "v4/waterfalls.cbor?descriptor={}",
//             urlencoding::encode(descriptor)
//         ));
//
//         if let Some(p) = page {
//             url.push_str(&format!("&page={}", p));
//         }
//         if let Some(idx) = to_index {
//             url.push_str(&format!("&to_index={}", idx));
//         }
//         if utxo_only {
//             url.push_str("&utxo_only=true");
//         }
//
//         let response = self.client.get(&url).send().await?;
//         let headers = response.headers().clone();
//         let data = response.bytes().await?.to_vec();
//         Ok((data, headers))
//     }
//
//     // Last used index endpoint
//     pub async fn last_used_index(&self, descriptor: &str) -> Result<LastUsedIndex, reqwest::Error> {
//         self.client
//             .get(&self.url(&format!(
//                 "v1/last_used_index?descriptor={}",
//                 urlencoding::encode(descriptor)
//             )))
//             .send()
//             .await?
//             .json()
//             .await
//     }
//
//     // Server information endpoints
//     pub async fn server_recipient(&self) -> Result<String, reqwest::Error> {
//         self.client
//             .get(&self.url("v1/server_recipient"))
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     pub async fn server_address(&self) -> Result<String, reqwest::Error> {
//         self.client
//             .get(&self.url("v1/server_address"))
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     pub async fn time_since_last_block(&self) -> Result<String, reqwest::Error> {
//         self.client
//             .get(&self.url("v1/time_since_last_block"))
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     pub async fn build_info(&self) -> Result<BuildInfo, reqwest::Error> {
//         self.client.get(&self.url("v1/build_info")).send().await?.json().await
//     }
//
//     // Blockchain data endpoints
//     pub async fn tip_hash(&self) -> Result<String, reqwest::Error> {
//         self.client.get(&self.url("blocks/tip/hash")).send().await?.text().await
//     }
//
//     pub async fn block_hash_by_height(&self, height: u64) -> Result<String, reqwest::Error> {
//         self.client
//             .get(&self.url(&format!("block-height/{}", height)))
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     pub async fn block_header(&self, hash: &str) -> Result<String, reqwest::Error> {
//         self.client
//             .get(&self.url(&format!("block/{}/header", hash)))
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     pub async fn tx_raw(&self, txid: &str) -> Result<Vec<u8>, reqwest::Error> {
//         self.client
//             .get(&self.url(&format!("tx/{}/raw", txid)))
//             .send()
//             .await?
//             .bytes()
//             .await
//             .map(|b| b.to_vec())
//     }
//
//     pub async fn address_txs(&self, address: &str) -> Result<Vec<AddressTxs>, reqwest::Error> {
//         self.client
//             .get(&self.url(&format!("address/{}/txs", address)))
//             .send()
//             .await?
//             .json()
//             .await
//     }
//
//     // Transaction broadcasting
//     pub async fn broadcast(&self, tx_hex: &str) -> Result<String, reqwest::Error> {
//         self.client
//             .post(&self.url("tx"))
//             .body(tx_hex.to_string())
//             .send()
//             .await?
//             .text()
//             .await
//     }
//
//     // Prometheus metrics
//     pub async fn metrics(&self) -> Result<String, reqwest::Error> {
//         self.client.get(&self.url("metrics")).send().await?.text().await
//     }
// }
