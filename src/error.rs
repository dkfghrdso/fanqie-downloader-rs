use thiserror::Error;

#[derive(Error, Debug)]
pub enum FanqieError {
    #[error("配置加载失败: {0}")]
    ConfigLoad(String),

    #[error("配置文件不存在: {0}")]
    ConfigNotFound(String),

    #[error("API 请求失败: {0}")]
    ApiRequest(String),

    #[error("API 节点不可用: {0}")]
    ApiNodeUnavailable(String),

    #[error("所有 API 节点都不可用")]
    AllNodesUnavailable,

    #[error("书籍不存在或已下架: {0}")]
    BookNotFound(String),

    #[error("章节获取失败: {0}")]
    ChapterFetch(String),

    #[error("下载失败: {0}")]
    Download(String),

    #[error("文件写入失败: {0}")]
    FileWrite(String),

    #[error("EPUB 生成失败: {0}")]
    EpubGeneration(String),

    #[error("无效的书籍 ID: {0}")]
    InvalidBookId(String),

    #[error("搜索无结果: {0}")]
    SearchNoResult(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("JSON 解析错误: {0}")]
    JsonParse(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("请求超时")]
    Timeout,

    #[error("速率限制")]
    RateLimited,
}

pub type Result<T> = std::result::Result<T, FanqieError>;
