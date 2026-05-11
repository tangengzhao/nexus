//! HSB Server 主入口
//!
//! 医院服务总线主服务程序

mod bootstrap;
mod config;
mod server;

use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::ServerConfig;
use crate::server::HsbServer;

/// HSB - Hospital Service Bus
#[derive(Parser)]
#[command(name = "hsb-server")]
#[command(author = "HSB Team")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Hospital Service Bus - 医院服务总线", long_about = None)]
struct Cli {
    /// 配置文件路径
    #[arg(short, long, default_value = "config/hsb.toml", global = true)]
    config: String,

    /// 日志级别
    #[arg(short, long, default_value = "info", global = true)]
    log_level: String,

    /// 以 JSON 格式输出日志
    #[arg(long, global = true)]
    json_log: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动服务器
    Start,
    /// 检查配置
    Check,
    /// 显示版本信息
    Version,
    /// 生成默认配置
    Init {
        /// 输出路径
        #[arg(short, long, default_value = "config/hsb.toml")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // 初始化日志
    init_tracing(&cli.log_level, cli.json_log);

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => {
            info!("Starting HSB Server v{}", env!("CARGO_PKG_VERSION"));
            run_server(&cli.config).await?;
        }
        Commands::Check => {
            info!("Checking configuration: {}", cli.config);
            check_config(&cli.config)?;
            info!("Configuration is valid");
        }
        Commands::Version => {
            println!("HSB Server v{}", env!("CARGO_PKG_VERSION"));
            println!("Rust Edition: 2024");
        }
        Commands::Init { output } => {
            info!("Generating default configuration to: {}", output);
            generate_default_config(&output)?;
            info!("Configuration generated successfully");
        }
    }

    Ok(())
}

fn init_tracing(level: &str, json: bool) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    if json {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
}

async fn run_server(config_path: &str) -> anyhow::Result<()> {
    // 加载配置
    let config = ServerConfig::load(config_path)?;
    info!("Configuration loaded from: {}", config_path);

    // 创建服务器
    let server = HsbServer::new(config).await?;
    info!("HSB Server initialized");

    // 启动服务器
    server.run().await?;

    Ok(())
}

fn check_config(config_path: &str) -> anyhow::Result<()> {
    let _config = ServerConfig::load(config_path)?;
    Ok(())
}

fn generate_default_config(output_path: &str) -> anyhow::Result<()> {
    let config = ServerConfig::default();
    let toml_str = toml::to_string_pretty(&config)?;

    // 确保目录存在
    if let Some(parent) = std::path::Path::new(output_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(output_path, toml_str)?;
    Ok(())
}
