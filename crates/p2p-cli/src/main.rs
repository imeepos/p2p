//! p2p-cli 可执行入口：初始化日志，分发子命令，错误统一转非零退出码。

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    match p2p_cli::run().await {
        Ok(code) => std::process::exit(code),
        Err(msg) => {
            eprintln!("p2p-cli: 错误: {msg}");
            std::process::exit(1);
        }
    }
}
