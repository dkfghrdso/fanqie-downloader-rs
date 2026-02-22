use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use once_cell::sync::OnceCell;

use crate::error::{FanqieError, Result};

pub static CONFIG: OnceCell<Arc<RwLock<AppConfig>>> = OnceCell::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSource {
    pub base_url: String,
    #[serde(default = "default_supports_full_download")]
    pub supports_full_download: bool,
}

fn default_supports_full_download() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoints {
    #[serde(default = "default_search")]
    pub search: String,
    #[serde(default = "default_detail")]
    pub detail: String,
    #[serde(default = "default_book")]
    pub book: String,
    #[serde(default = "default_directory")]
    pub directory: String,
    #[serde(default = "default_content")]
    pub content: String,
    #[serde(default = "default_chapter")]
    pub chapter: String,
    #[serde(default = "default_raw_full")]
    pub raw_full: String,
}

fn default_search() -> String { "/api/search".to_string() }
fn default_detail() -> String { "/api/detail".to_string() }
fn default_book() -> String { "/api/book".to_string() }
fn default_directory() -> String { "/api/directory".to_string() }
fn default_content() -> String { "/api/content".to_string() }
fn default_chapter() -> String { "/api/chapter".to_string() }
fn default_raw_full() -> String { "/api/raw_full".to_string() }

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            search: default_search(),
            detail: default_detail(),
            book: default_book(),
            directory: default_directory(),
            content: default_content(),
            chapter: default_chapter(),
            raw_full: default_raw_full(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigParams {
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,
    #[serde(default = "default_request_rate_limit")]
    pub request_rate_limit: f64,
    #[serde(default = "default_connection_pool_size")]
    pub connection_pool_size: usize,
    #[serde(default = "default_api_rate_limit")]
    pub api_rate_limit: u32,
    #[serde(default = "default_rate_limit_window")]
    pub rate_limit_window: f64,
    #[serde(default = "default_async_batch_size")]
    pub async_batch_size: usize,
    #[serde(default = "default_download_enabled")]
    pub download_enabled: bool,
}

fn default_max_workers() -> usize { 30 }
fn default_max_retries() -> u32 { 3 }
fn default_request_timeout() -> u64 { 30 }
fn default_request_rate_limit() -> f64 { 0.02 }
fn default_connection_pool_size() -> usize { 200 }
fn default_api_rate_limit() -> u32 { 50 }
fn default_rate_limit_window() -> f64 { 1.0 }
fn default_async_batch_size() -> usize { 50 }
fn default_download_enabled() -> bool { true }

impl Default for ConfigParams {
    fn default() -> Self {
        Self {
            max_workers: default_max_workers(),
            max_retries: default_max_retries(),
            request_timeout: default_request_timeout(),
            request_rate_limit: default_request_rate_limit(),
            connection_pool_size: default_connection_pool_size(),
            api_rate_limit: default_api_rate_limit(),
            rate_limit_window: default_rate_limit_window(),
            async_batch_size: default_async_batch_size(),
            download_enabled: default_download_enabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanqieJson {
    pub version: String,
    pub updated_at: String,
    pub api_sources: Vec<ApiSource>,
    pub endpoints: Endpoints,
    pub config: ConfigParams,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub api_sources: Vec<ApiSource>,
    pub endpoints: Endpoints,
    pub params: ConfigParams,
    pub current_node_index: usize,
    pub config_path: PathBuf,
}

impl AppConfig {
    pub fn new(json: FanqieJson, config_path: PathBuf) -> Self {
        Self {
            api_sources: json.api_sources,
            endpoints: json.endpoints,
            params: json.config,
            current_node_index: 0,
            config_path,
        }
    }

    pub fn get_current_node(&self) -> Option<&ApiSource> {
        self.api_sources.get(self.current_node_index)
    }

    pub fn get_full_download_nodes(&self) -> Vec<&ApiSource> {
        self.api_sources
            .iter()
            .filter(|s| s.supports_full_download)
            .collect()
    }

    pub fn switch_to_next_node(&mut self) -> bool {
        if self.current_node_index + 1 < self.api_sources.len() {
            self.current_node_index += 1;
            true
        } else {
            false
        }
    }

    pub fn reset_node_index(&mut self) {
        self.current_node_index = 0;
    }

    pub fn set_node(&mut self, index: usize) -> bool {
        if index < self.api_sources.len() {
            self.current_node_index = index;
            true
        } else {
            false
        }
    }
}

pub fn find_config_file() -> Option<PathBuf> {
    let possible_paths: Vec<PathBuf> = vec![
        PathBuf::from("config/fanqie.json"),
        PathBuf::from("fanqie.json"),
    ];

    for path in possible_paths {
        if path.exists() {
            return Some(path);
        }
    }

    if let Some(config_dir) = dirs::config_dir() {
        let config_path = config_dir.join("fanqie-downloader/fanqie.json");
        if config_path.exists() {
            return Some(config_path);
        }
    }

    None
}

pub fn load_config_from_file<P: AsRef<Path>>(path: P) -> Result<FanqieJson> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|e| FanqieError::ConfigLoad(format!("无法读取配置文件: {}", e)))?;

    let json: FanqieJson = serde_json::from_str(&content)
        .map_err(|e| FanqieError::ConfigLoad(format!("配置文件格式错误: {}", e)))?;

    if json.api_sources.is_empty() {
        return Err(FanqieError::ConfigLoad("配置文件缺少 api_sources".to_string()));
    }

    Ok(json)
}

pub async fn init_config() -> Result<Arc<RwLock<AppConfig>>> {
    let config_path = find_config_file()
        .ok_or_else(|| FanqieError::ConfigNotFound("未找到 fanqie.json 配置文件".to_string()))?;

    let json = load_config_from_file(&config_path)?;
    let config = AppConfig::new(json, config_path);
    
    let arc_config = Arc::new(RwLock::new(config));
    
    CONFIG.set(arc_config.clone())
        .map_err(|_| FanqieError::ConfigLoad("配置已初始化".to_string()))?;

    Ok(arc_config)
}

pub async fn get_config() -> Arc<RwLock<AppConfig>> {
    CONFIG.get()
        .expect("配置未初始化，请先调用 init_config()")
        .clone()
}
