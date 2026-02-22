use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::init_config;
use crate::api::init_api_client;
use crate::search::{search, format_search_results, get_book_info, format_book_info};
use crate::downloader::{DownloadOptions, download_book};
use crate::batch::{BatchOptions, batch_download};
use crate::export::ensure_output_dir;

#[derive(Parser)]
#[command(name = "fqdl")]
#[command(author = "Fanqie Novel Downloader Rust Team")]
#[command(version)]
#[command(about = "番茄小说下载器 - Rust 版本", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "搜索书籍")]
    Search {
        #[arg(help = "搜索关键词")]
        keyword: String,
        
        #[arg(short, long, default_value = "0")]
        offset: i32,
    },

    #[command(about = "显示书籍信息")]
    Info {
        #[arg(help = "书籍ID")]
        book_id: String,
    },

    #[command(about = "下载书籍")]
    Download {
        #[arg(help = "书籍ID")]
        book_id: String,
        
        #[arg(short, long, default_value = "~/Downloads")]
        path: String,
        
        #[arg(short = 'f', long, default_value = "txt")]
        format: String,
        
        #[arg(short, long, help = "起始章节 (从1开始)")]
        start: Option<usize>,
        
        #[arg(short, long, help = "结束章节")]
        end: Option<usize>,
    },

    #[command(about = "批量下载书籍")]
    Batch {
        #[arg(help = "书籍ID列表 (空格分隔)")]
        book_ids: Vec<String>,
        
        #[arg(short, long, default_value = "~/Downloads/FanqieNovels")]
        path: String,
        
        #[arg(short = 'f', long, default_value = "txt")]
        format: String,
        
        #[arg(short = 'c', long, default_value = "3")]
        concurrent: usize,
        
        #[arg(short, long, help = "从文件读取书籍ID列表")]
        file: Option<String>,
    },

    #[command(about = "显示配置信息")]
    Config {
        #[arg(short, long, help = "配置文件路径")]
        config_file: Option<String>,
    },
}

pub async fn run() -> crate::error::Result<()> {
    let cli = Cli::parse();

    let config_path = find_config_file();
    if let Some(path) = &config_path {
        println!("使用配置文件: {}", path.display());
    }

    init_config().await?;
    init_api_client().await?;

    match cli.command {
        Commands::Search { keyword, offset } => {
            cmd_search(keyword, offset).await?;
        }
        Commands::Info { book_id } => {
            cmd_info(book_id).await?;
        }
        Commands::Download { book_id, path, format, start, end } => {
            cmd_download(book_id, path, format, start, end).await?;
        }
        Commands::Batch { book_ids, path, format, concurrent, file } => {
            cmd_batch(book_ids, path, format, concurrent, file).await?;
        }
        Commands::Config { config_file } => {
            cmd_config(config_file).await?;
        }
    }

    Ok(())
}

fn find_config_file() -> Option<PathBuf> {
    let possible_paths = vec![
        PathBuf::from("config/fanqie.json"),
        PathBuf::from("fanqie.json"),
    ];

    for path in possible_paths {
        if path.exists() {
            return Some(path);
        }
    }

    None
}

async fn cmd_search(keyword: String, offset: i32) -> crate::error::Result<()> {
    println!("正在搜索: {}", keyword);
    
    let result = search(&keyword, Some(offset)).await?;
    println!("{}", format_search_results(&result.books));
    
    Ok(())
}

async fn cmd_info(book_id: String) -> crate::error::Result<()> {
    println!("正在获取书籍信息: {}", book_id);
    
    let book_info = get_book_info(&book_id).await?;
    
    let client = crate::api::get_api_client();
    let chapter_response = client.get_chapter_list(&book_id).await?;
    
    let chapter_count = if let Some(wrapper) = chapter_response.data {
        if let Some(data) = wrapper.data {
            if let Some(ids) = data.all_item_ids {
                Some(ids.len())
            } else if let Some(volumes) = data.chapter_list_with_volume {
                let count: usize = volumes.iter().map(|v| v.len()).sum();
                Some(count)
            } else if let Some(chapters) = data.data {
                Some(chapters.len())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    println!("{}", format_book_info(&book_info, chapter_count));
    
    Ok(())
}

async fn cmd_download(
    book_id: String,
    path: String,
    format: String,
    start: Option<usize>,
    end: Option<usize>,
) -> crate::error::Result<()> {
    let save_path = expand_tilde(&path);
    ensure_output_dir(&save_path)?;

    let options = DownloadOptions {
        book_id,
        save_path,
        format,
        start_chapter: start,
        end_chapter: end,
    };

    download_book(options).await?;

    println!("\n下载完成!");
    
    Ok(())
}

async fn cmd_batch(
    book_ids: Vec<String>,
    path: String,
    format: String,
    concurrent: usize,
    file: Option<String>,
) -> crate::error::Result<()> {
    let mut all_book_ids = book_ids;

    if let Some(file_path) = file {
        let content = std::fs::read_to_string(&file_path)?;
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                all_book_ids.push(line.to_string());
            }
        }
    }

    if all_book_ids.is_empty() {
        println!("错误: 请提供至少一个书籍ID");
        return Ok(());
    }

    let save_path = expand_tilde(&path);
    ensure_output_dir(&save_path)?;

    let options = BatchOptions {
        book_ids: all_book_ids,
        save_path,
        format,
        max_concurrent: concurrent.min(5),
    };

    batch_download(options).await?;

    Ok(())
}

async fn cmd_config(config_file: Option<String>) -> crate::error::Result<()> {
    let config = crate::config::get_config().await;
    let config_guard = config.read().await;

    println!("\n当前配置:");
    println!("{}", "=".repeat(50));
    println!("API 节点数量: {}", config_guard.api_sources.len());
    
    if let Some(node) = config_guard.get_current_node() {
        println!("当前节点: {}", node.base_url);
    }
    
    println!("最大并发数: {}", config_guard.params.max_workers);
    println!("API 速率限制: {}", config_guard.params.api_rate_limit);
    println!("请求超时: {}秒", config_guard.params.request_timeout);
    println!("连接池大小: {}", config_guard.params.connection_pool_size);
    println!("{}", "=".repeat(50));

    if let Some(path) = config_file {
        println!("\n配置文件路径: {}", path);
    } else if let Some(path) = find_config_file() {
        println!("\n配置文件路径: {}", path.display());
    }

    Ok(())
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return path.replacen("~", &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}
