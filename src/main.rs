use rs_fanqie_downloader::cli;
use rs_fanqie_downloader::error::FanqieError;

#[tokio::main]
async fn main() {
    println!("{}", "=".repeat(50));
    println!("番茄小说下载器 (Rust 版本)");
    println!("{}", "=".repeat(50));

    if let Err(e) = cli::run().await {
        match e {
            FanqieError::ConfigNotFound(msg) => {
                eprintln!("\n配置错误: {}", msg);
                eprintln!("请确保 fanqie.json 配置文件存在");
            }
            FanqieError::AllNodesUnavailable => {
                eprintln!("\n错误: 所有 API 节点都不可用");
                eprintln!("请检查网络连接或稍后重试");
            }
            FanqieError::BookNotFound(id) => {
                eprintln!("\n错误: 书籍不存在或已下架: {}", id);
            }
            _ => {
                eprintln!("\n错误: {}", e);
            }
        }
        std::process::exit(1);
    }
}
