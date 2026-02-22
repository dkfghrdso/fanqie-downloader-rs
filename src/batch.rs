use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use futures::future::join_all;

use crate::downloader::{DownloadOptions, download_book};
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct BatchOptions {
    pub book_ids: Vec<String>,
    pub save_path: String,
    pub format: String,
    pub max_concurrent: usize,
}

#[derive(Debug, Clone)]
pub struct BatchResult {
    pub book_id: String,
    pub success: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

pub struct BatchDownloader {
    options: BatchOptions,
}

impl BatchDownloader {
    pub fn new(options: BatchOptions) -> Self {
        Self { options }
    }

    pub async fn run(&self) -> Result<Vec<BatchResult>> {
        let total = self.options.book_ids.len();
        println!("开始批量下载 {} 本书籍", total);
        println!("保存路径: {}", self.options.save_path);
        println!("文件格式: {}", self.options.format);
        println!("并发数量: {}", self.options.max_concurrent);
        println!("{}", "-".repeat(50));

        let semaphore = Arc::new(Semaphore::new(self.options.max_concurrent));

        let futures: Vec<_> = self.options.book_ids
            .iter()
            .enumerate()
            .map(|(index, book_id)| {
                let semaphore = semaphore.clone();
                let save_path = self.options.save_path.clone();
                let format = self.options.format.clone();
                let book_id = book_id.clone();

                async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    let start = Instant::now();

                    println!("[{}/{}] 开始下载: {}", index + 1, total, book_id);

                    let options = DownloadOptions {
                        book_id: book_id.clone(),
                        save_path,
                        format,
                        start_chapter: None,
                        end_chapter: None,
                    };

                    let result = match download_book(options).await {
                        Ok(path) => {
                            let duration = start.elapsed().as_millis() as u64;
                            println!("[{}/{}] ✓ 下载完成: {} ({}ms)", 
                                index + 1, total, book_id, duration);
                            BatchResult {
                                book_id,
                                success: true,
                                output_path: Some(path.to_string_lossy().to_string()),
                                error: None,
                                duration_ms: duration,
                            }
                        }
                        Err(e) => {
                            let duration = start.elapsed().as_millis() as u64;
                            println!("[{}/{}] ✗ 下载失败: {} - {}", 
                                index + 1, total, book_id, e);
                            BatchResult {
                                book_id,
                                success: false,
                                output_path: None,
                                error: Some(e.to_string()),
                                duration_ms: duration,
                            }
                        }
                    };

                    result
                }
            })
            .collect();

        let results = join_all(futures).await;

        let success_count = results.iter().filter(|r| r.success).count();
        let failed_count = results.iter().filter(|r| !r.success).count();
        let total_duration: u64 = results.iter().map(|r| r.duration_ms).sum();

        println!("\n{}", "=".repeat(60));
        println!("批量下载完成!");
        println!("总计: {} 本书籍", total);
        println!("成功: {} 本", success_count);
        println!("失败: {} 本", failed_count);
        println!("用时: {:.1} 秒", total_duration as f64 / 1000.0);
        println!("{}", "=".repeat(60));

        Ok(results)
    }
}

pub async fn batch_download(options: BatchOptions) -> Result<Vec<BatchResult>> {
    let downloader = BatchDownloader::new(options);
    downloader.run().await
}
