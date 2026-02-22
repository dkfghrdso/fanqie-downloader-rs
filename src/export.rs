use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use html_escape::encode_text;

use crate::api::{BookInfo, ChapterContent};
use crate::error::{FanqieError, Result};

pub fn export_txt(book_info: &BookInfo, chapters: &[ChapterContent], save_path: &str) -> Result<PathBuf> {
    let file_name = sanitize_filename(&book_info.book_name);
    let output_path = PathBuf::from(save_path).join(format!("{}.txt", file_name));

    let mut file = File::create(&output_path)
        .map_err(|e| FanqieError::FileWrite(format!("创建文件失败: {}", e)))?;

    writeln!(file, "书名: {}", book_info.book_name)
        .map_err(|e| FanqieError::FileWrite(format!("写入失败: {}", e)))?;
    writeln!(file, "作者: {}", book_info.author)
        .map_err(|e| FanqieError::FileWrite(format!("写入失败: {}", e)))?;
    writeln!(file, "\n{}\n", "=".repeat(50))
        .map_err(|e| FanqieError::FileWrite(format!("写入失败: {}", e)))?;

    for chapter in chapters {
        writeln!(file, "\n{}\n", chapter.title)
            .map_err(|e| FanqieError::FileWrite(format!("写入失败: {}", e)))?;
        writeln!(file, "{}\n", chapter.content)
            .map_err(|e| FanqieError::FileWrite(format!("写入失败: {}", e)))?;
    }

    Ok(output_path)
}

pub fn export_epub(book_info: &BookInfo, chapters: &[ChapterContent], save_path: &str) -> Result<PathBuf> {
    let file_name = sanitize_filename(&book_info.book_name);
    let output_path = PathBuf::from(save_path).join(format!("{}.epub", file_name));

    let file = File::create(&output_path)
        .map_err(|e| FanqieError::FileWrite(format!("创建文件失败: {}", e)))?;

    let mut builder = epub_builder::EpubBuilder::new(epub_builder::ZipLibrary::new()
        .map_err(|e| FanqieError::EpubGeneration(format!("创建 ZIP 库失败: {}", e)))?)
        .map_err(|e| FanqieError::EpubGeneration(format!("创建 EPUB 构建器失败: {}", e)))?;

    builder.set_title(&book_info.book_name);
    builder.set_authors(vec![book_info.author.clone()]);
    builder.set_lang("zh-CN");

    for (index, chapter) in chapters.iter().enumerate() {
        let chapter_filename = format!("chapter_{:04}.xhtml", index);
        let html_content = chapter_to_html(&chapter.title, &chapter.content);
        
        builder.add_content(
            epub_builder::EpubContent::new(&chapter_filename, html_content.as_bytes())
                .title(&chapter.title)
        ).map_err(|e| FanqieError::EpubGeneration(format!("添加章节失败: {}", e)))?;
    }

    builder.generate(file)
        .map_err(|e| FanqieError::EpubGeneration(format!("生成 EPUB 失败: {}", e)))?;

    Ok(output_path)
}

fn chapter_to_html(title: &str, content: &str) -> String {
    let escaped_title = encode_text(title);
    let escaped_content = encode_text(content);
    
    let paragraphs: Vec<&str> = escaped_content.split('\n').collect();
    let formatted_content = paragraphs
        .iter()
        .map(|p| format!("<p>{}</p>", p.trim()))
        .collect::<Vec<String>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>{}</title>
</head>
<body>
    <h1>{}</h1>
    {}
</body>
</html>"#,
        escaped_title, escaped_title, formatted_content
    )
}

fn sanitize_filename(name: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut result = name.to_string();
    
    for c in invalid_chars {
        result = result.replace(c, "_");
    }
    
    result.trim().to_string()
}

pub fn ensure_output_dir(path: &str) -> Result<PathBuf> {
    let output_path = PathBuf::from(path);
    
    if !output_path.exists() {
        fs::create_dir_all(&output_path)
            .map_err(|e| FanqieError::FileWrite(format!("创建目录失败: {}", e)))?;
    }

    Ok(output_path)
}
