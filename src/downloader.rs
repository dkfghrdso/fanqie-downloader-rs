use std::path::PathBuf;
use tokio::sync::mpsc;
use futures::future::join_all;
use indicatif::{ProgressBar, ProgressStyle};

use crate::api::{get_api_client, ChapterContent, ChapterInfo, ChapterInfoRaw};
use crate::config::get_config;
use crate::error::{FanqieError, Result};
use crate::export::{export_txt, export_epub, ensure_output_dir};

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub book_id: String,
    pub save_path: String,
    pub format: String,
    pub start_chapter: Option<usize>,
    pub end_chapter: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum DownloadProgress {
    Started { total: usize },
    Chapter { current: usize, total: usize, title: String },
    Completed { output_path: String },
    Error { message: String },
}

pub struct Downloader {
    options: DownloadOptions,
}

impl Downloader {
    pub fn new(options: DownloadOptions) -> Self {
        Self { options }
    }

    pub async fn get_chapters(&self) -> Result<Vec<ChapterInfo>> {
        let client = get_api_client();
        
        let directory_response = client.get_directory(&self.options.book_id).await?;
        
        if directory_response.code == 200 {
            if let Some(data) = directory_response.data {
                if let Some(lists) = data.lists {
                    return Ok(lists);
                }
            }
        }

        let chapter_response = client.get_chapter_list(&self.options.book_id).await?;
        
        if chapter_response.code != 200 {
            return Err(FanqieError::ChapterFetch(
                format!("获取章节列表失败: API 返回错误码 {}", chapter_response.code)
            ));
        }

        if let Some(wrapper) = chapter_response.data {
            if let Some(data) = wrapper.data {
                if let Some(volumes) = data.chapter_list_with_volume {
                    let chapters: Vec<ChapterInfo> = volumes
                        .iter()
                        .flat_map(|v| v.iter())
                        .map(|c| c.to_chapter_info())
                        .collect();
                    if !chapters.is_empty() {
                        return Ok(chapters);
                    }
                }
                if let Some(chapters) = data.data {
                    return Ok(chapters);
                }
            }
        }

        Err(FanqieError::ChapterFetch("章节列表为空".to_string()))
    }

    pub async fn download_chapter(&self, chapter_id: &str) -> Result<ChapterContent> {
        let client = get_api_client();
        let response = client.get_chapter_content(chapter_id).await?;

        if response.code != 200 {
            return Err(FanqieError::ChapterFetch(
                format!("获取章节内容失败: {}", chapter_id)
            ));
        }

        response.data.ok_or_else(|| {
            FanqieError::ChapterFetch(format!("章节内容为空: {}", chapter_id))
        })
    }

    pub async fn download_all_chapters(
        &self,
        chapters: &[ChapterInfo],
        progress_tx: Option<mpsc::Sender<DownloadProgress>>,
    ) -> Result<Vec<ChapterContent>> {
        let config = get_config().await;
        let config_guard = config.read().await;
        let max_workers = config_guard.params.max_workers;
        drop(config_guard);

        let total = chapters.len();
        
        if let Some(tx) = &progress_tx {
            tx.send(DownloadProgress::Started { total }).await.ok();
        }

        let pb = ProgressBar::new(total as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"));

        let mut results = Vec::with_capacity(total);
        let chunks: Vec<Vec<ChapterInfo>> = chapters
            .chunks(max_workers)
            .map(|c| c.to_vec())
            .collect();

        for (chunk_index, chunk) in chunks.iter().enumerate() {
            let futures: Vec<_> = chunk
                .iter()
                .map(|chapter| self.download_chapter(&chapter.chapter_id))
                .collect();

            let chunk_results = join_all(futures).await;

            for (i, result) in chunk_results.into_iter().enumerate() {
                let current = chunk_index * max_workers + i + 1;
                
                match result {
                    Ok(content) => {
                        if let Some(tx) = &progress_tx {
                            tx.send(DownloadProgress::Chapter {
                                current,
                                total,
                                title: content.title.clone(),
                            }).await.ok();
                        }
                        results.push(content);
                        pb.inc(1);
                    }
                    Err(e) => {
                        if let Some(tx) = &progress_tx {
                            tx.send(DownloadProgress::Error {
                                message: format!("章节下载失败: {}", e),
                            }).await.ok();
                        }
                    }
                }
            }
        }

        pb.finish_with_message("下载完成");
        Ok(results)
    }

    pub async fn download_book(&self) -> Result<PathBuf> {
        let client = get_api_client();
        
        let detail_response = client.get_book_detail(&self.options.book_id).await?;
        
        if detail_response.code != 200 {
            return Err(FanqieError::BookNotFound(self.options.book_id.clone()));
        }

        let book_info = detail_response.data
            .and_then(|d| d.data)
            .ok_or_else(|| {
                FanqieError::BookNotFound(self.options.book_id.clone())
            })?;

        println!("正在下载: {}", book_info.book_name);
        println!("作者: {}", book_info.author);

        let chapters = self.get_chapters().await?;
        let total_chapters = chapters.len();
        println!("共 {} 章", total_chapters);

        let selected_chapters: Vec<ChapterInfo> = {
            let start = self.options.start_chapter.unwrap_or(1).max(1) - 1;
            let end = self.options.end_chapter
                .unwrap_or(total_chapters)
                .min(total_chapters);
            
            if start >= total_chapters {
                return Err(FanqieError::Download("起始章节超出范围".to_string()));
            }

            chapters[start..end].to_vec()
        };

        println!("下载范围: {} - {}", 
            selected_chapters.first().map(|c| c.title.as_str()).unwrap_or(""),
            selected_chapters.last().map(|c| c.title.as_str()).unwrap_or("")
        );

        ensure_output_dir(&self.options.save_path)?;

        let contents = self.download_all_chapters(&selected_chapters, None).await?;

        let output_path = match self.options.format.to_lowercase().as_str() {
            "txt" => export_txt(&book_info, &contents, &self.options.save_path)?,
            "epub" => export_epub(&book_info, &contents, &self.options.save_path)?,
            _ => export_txt(&book_info, &contents, &self.options.save_path)?,
        };

        println!("保存至: {}", output_path.display());

        Ok(output_path)
    }
}

pub async fn download_book(options: DownloadOptions) -> Result<PathBuf> {
    let downloader = Downloader::new(options);
    downloader.download_book().await
}
