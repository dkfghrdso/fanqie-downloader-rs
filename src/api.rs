use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore, Mutex};
use reqwest::Client;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

use crate::config::{AppConfig, get_config};
use crate::error::{FanqieError, Result};

pub struct ApiClient {
    client: Client,
    semaphore: Arc<Semaphore>,
    current_node: Arc<Mutex<String>>,
}

impl ApiClient {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.params.request_timeout))
            .pool_max_idle_per_host(config.params.connection_pool_size)
            .pool_idle_timeout(Duration::from_secs(60))
            .user_agent(Self::random_user_agent())
            .gzip(true)
            .brotli(true)
            .build()
            .map_err(|e| FanqieError::ApiRequest(format!("创建 HTTP 客户端失败: {}", e)))?;

        let semaphore = Arc::new(Semaphore::new(config.params.max_workers));
        
        let current_node = if let Some(node) = config.get_current_node() {
            node.base_url.clone()
        } else {
            String::new()
        };
        let current_node = Arc::new(Mutex::new(current_node));

        Ok(Self {
            client,
            semaphore,
            current_node,
        })
    }

    fn random_user_agent() -> &'static str {
        let user_agents = [
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
        ];
        user_agents[rand::random::<usize>() % user_agents.len()]
    }

    fn get_headers() -> HashMap<&'static str, &'static str> {
        let mut headers = HashMap::new();
        headers.insert("Accept", "application/json, text/javascript, */*; q=0.01");
        headers.insert("Accept-Language", "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7");
        headers.insert("Referer", "https://fanqienovel.com/");
        headers.insert("X-Requested-With", "XMLHttpRequest");
        headers.insert("Content-Type", "application/json");
        headers
    }

    pub async fn get_current_node(&self) -> String {
        self.current_node.lock().await.clone()
    }

    pub async fn set_current_node(&self, node: String) {
        *self.current_node.lock().await = node;
    }

    pub async fn request<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &HashMap<&str, &str>,
    ) -> Result<T> {
        let _permit = self.semaphore.acquire().await
            .map_err(|e| FanqieError::ApiRequest(format!("获取信号量失败: {}", e)))?;

        let config = get_config().await;
        let config_guard = config.read().await;

        let nodes: Vec<String> = config_guard.api_sources
            .iter()
            .map(|s| s.base_url.clone())
            .collect();

        let mut last_error = None;

        for (index, base_url) in nodes.iter().enumerate() {
            let url = format!("{}{}", base_url, endpoint);

            let mut request = self.client.get(&url)
                .query(params);

            for (key, value) in Self::get_headers() {
                request = request.header(key, value);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match response.json::<T>().await {
                            Ok(data) => {
                                if index != config_guard.current_node_index {
                                    drop(config_guard);
                                    let config = get_config().await;
                                    let mut config_guard = config.write().await;
                                    config_guard.set_node(index);
                                    self.set_current_node(base_url.clone()).await;
                                }
                                return Ok(data);
                            }
                            Err(e) => {
                                last_error = Some(FanqieError::JsonParse(format!("{}: {}", url, e)));
                            }
                        }
                    } else if status.as_u16() >= 500 {
                        last_error = Some(FanqieError::ApiNodeUnavailable(base_url.clone()));
                        continue;
                    } else {
                        let status_code = status.as_u16();
                        match response.json::<T>().await {
                            Ok(data) => return Ok(data),
                            Err(e) => {
                                last_error = Some(FanqieError::ApiRequest(
                                    format!("HTTP {}: {}", status_code, e)
                                ));
                            }
                        }
                    }
                }
                Err(e) => {
                    if e.is_timeout() {
                        last_error = Some(FanqieError::Timeout);
                    } else if e.is_connect() {
                        last_error = Some(FanqieError::Network(format!("{}: {}", base_url, e)));
                    } else {
                        last_error = Some(FanqieError::ApiRequest(format!("{}: {}", base_url, e)));
                    }
                }
            }
        }

        Err(last_error.unwrap_or(FanqieError::AllNodesUnavailable))
    }

    pub async fn search_books(&self, keyword: &str, offset: i32) -> Result<SearchResponse> {
        let offset_str = offset.to_string();
        let mut params = HashMap::new();
        params.insert("key", keyword);
        params.insert("tab_type", "3");
        params.insert("offset", &offset_str);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.search.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }

    pub async fn get_book_detail(&self, book_id: &str) -> Result<BookDetailResponse> {
        let mut params = HashMap::new();
        params.insert("book_id", book_id);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.detail.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }

    pub async fn get_chapter_list(&self, book_id: &str) -> Result<ChapterListResponse> {
        let mut params = HashMap::new();
        params.insert("book_id", book_id);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.book.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }

    pub async fn get_directory(&self, book_id: &str) -> Result<DirectoryResponse> {
        let mut params = HashMap::new();
        params.insert("fq_id", book_id);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.directory.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }

    pub async fn get_chapter_content(&self, chapter_id: &str) -> Result<ChapterContentResponse> {
        let mut params = HashMap::new();
        params.insert("tab", "小说");
        params.insert("item_id", chapter_id);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.content.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }

    pub async fn get_raw_full(&self, book_id: &str) -> Result<RawFullResponse> {
        let mut params = HashMap::new();
        params.insert("book_id", book_id);

        let config = get_config().await;
        let config_guard = config.read().await;
        let endpoint = config_guard.endpoints.raw_full.clone();
        drop(config_guard);

        self.request(&endpoint, &params).await
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchResponse {
    pub code: i32,
    pub data: Option<SearchData>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchData {
    pub search_tabs: Option<Vec<SearchTab>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchTab {
    pub tab_type: i32,
    pub data: Option<Vec<SearchItem>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchItem {
    pub book_data: Option<Vec<BookInfo>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BookInfo {
    #[serde(default)]
    pub book_id: String,
    #[serde(default)]
    pub book_name: String,
    #[serde(default)]
    pub author: String,
    pub abstract_field: Option<String>,
    #[serde(rename = "abstract")]
    pub abstract_opt: Option<String>,
    pub cover: Option<String>,
    pub creation_status: Option<String>,
    pub word_count: Option<i64>,
    pub chapter_count: Option<i32>,
}

impl BookInfo {
    pub fn get_abstract(&self) -> &str {
        self.abstract_field
            .as_deref()
            .or(self.abstract_opt.as_deref())
            .unwrap_or("")
    }

    pub fn get_status(&self) -> &str {
        match self.creation_status.as_deref() {
            Some("0") => "已完结",
            Some("1") => "连载中",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BookDetailResponse {
    pub code: i32,
    pub data: Option<BookDetailData>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BookDetailData {
    pub code: Option<i32>,
    pub data: Option<BookInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterListResponse {
    pub code: i32,
    pub data: Option<ChapterListDataWrapper>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterListDataWrapper {
    pub code: Option<i32>,
    pub data: Option<ChapterListData>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterListData {
    #[serde(rename = "allItemIds")]
    pub all_item_ids: Option<Vec<String>>,
    #[serde(rename = "chapterListWithVolume")]
    pub chapter_list_with_volume: Option<Vec<Vec<ChapterInfoRaw>>>,
    pub data: Option<Vec<ChapterInfo>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterInfoRaw {
    #[serde(rename = "itemId")]
    pub item_id: String,
    pub title: String,
    #[serde(rename = "volume_name")]
    pub volume_name: Option<String>,
}

impl ChapterInfoRaw {
    pub fn to_chapter_info(&self) -> ChapterInfo {
        ChapterInfo {
            chapter_id: self.item_id.clone(),
            title: self.title.clone(),
            word_count: None,
            is_vip: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterInfo {
    #[serde(default)]
    pub chapter_id: String,
    #[serde(default)]
    pub title: String,
    pub word_count: Option<i32>,
    pub is_vip: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DirectoryResponse {
    pub code: i32,
    pub data: Option<DirectoryData>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DirectoryData {
    pub lists: Option<Vec<ChapterInfo>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterContentResponse {
    pub code: i32,
    pub data: Option<ChapterContent>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChapterContent {
    #[serde(default)]
    pub chapter_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RawFullResponse {
    pub code: i32,
    pub data: Option<RawFullData>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RawFullData {
    pub chapters: Option<Vec<ChapterContent>>,
}

pub static API_CLIENT: once_cell::sync::OnceCell<Arc<ApiClient>> = once_cell::sync::OnceCell::new();

pub async fn init_api_client() -> Result<Arc<ApiClient>> {
    let config = get_config().await;
    let config_guard = config.read().await;
    let client = ApiClient::new(&config_guard)?;
    let arc_client = Arc::new(client);
    
    API_CLIENT.set(arc_client.clone())
        .map_err(|_| FanqieError::ApiRequest("API 客户端已初始化".to_string()))?;

    Ok(arc_client)
}

pub fn get_api_client() -> Arc<ApiClient> {
    API_CLIENT.get()
        .expect("API 客户端未初始化，请先调用 init_api_client()")
        .clone()
}
