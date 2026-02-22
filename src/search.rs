use crate::api::{get_api_client, BookInfo};
use crate::error::{FanqieError, Result};

pub struct SearchResult {
    pub books: Vec<BookInfo>,
    pub total: usize,
}

pub async fn search(keyword: &str, offset: Option<i32>) -> Result<SearchResult> {
    let client = get_api_client();
    let response = client.search_books(keyword, offset.unwrap_or(0)).await?;

    if response.code != 200 {
        return Err(FanqieError::ApiRequest(
            format!("搜索失败: API 返回错误码 {}", response.code)
        ));
    }

    let mut books = Vec::new();

    if let Some(data) = response.data {
        if let Some(tabs) = data.search_tabs {
            for tab in tabs {
                if tab.tab_type == 3 {
                    if let Some(items) = tab.data {
                        for item in items {
                            if let Some(book_data) = item.book_data {
                                books.extend(book_data);
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    let total = books.len();

    Ok(SearchResult { books, total })
}

fn truncate_string(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        format!("{}...", s.chars().take(max_chars - 3).collect::<String>())
    } else {
        s.to_string()
    }
}

pub fn format_search_results(books: &[BookInfo]) -> String {
    if books.is_empty() {
        return "未找到相关书籍".to_string();
    }

    let mut result = format!("\n找到 {} 本书籍:\n\n", books.len());
    result.push_str(&format!("{:<15} {:<25} {:<15} {:<8}\n", "书籍ID", "书名", "作者", "状态"));
    result.push_str(&format!("{}\n", "-".repeat(65)));

    for book in books {
        let name = truncate_string(&book.book_name, 22);
        let author = truncate_string(&book.author, 12);

        result.push_str(&format!(
            "{:<15} {:<25} {:<15} {:<8}\n",
            book.book_id,
            name,
            author,
            book.get_status()
        ));
    }

    result
}

pub async fn get_book_info(book_id: &str) -> Result<BookInfo> {
    let client = get_api_client();
    
    let detail_response = client.get_book_detail(book_id).await?;
    
    if detail_response.code != 200 {
        return Err(FanqieError::BookNotFound(book_id.to_string()));
    }

    if let Some(data) = detail_response.data {
        if let Some(book_info) = data.data {
            return Ok(book_info);
        }
    }

    Err(FanqieError::BookNotFound(book_id.to_string()))
}

pub fn format_book_info(book: &BookInfo, chapter_count: Option<usize>) -> String {
    let mut result = String::new();
    result.push_str(&"=".repeat(50));
    result.push_str("\n");
    result.push_str(&format!("书名: {}\n", book.book_name));
    result.push_str(&format!("作者: {}\n", book.author));
    
    if let Some(count) = chapter_count {
        result.push_str(&format!("章节数: {}\n", count));
    }
    
    result.push_str(&"-".repeat(50));
    result.push_str("\n");
    
    let abstract_text = book.get_abstract();
    if !abstract_text.is_empty() {
        result.push_str("简介:\n");
        let abstract_display = truncate_string(abstract_text, 200);
        result.push_str(&abstract_display);
        result.push_str("\n");
    }
    
    result.push_str(&"=".repeat(50));
    result.push_str("\n");

    result
}
