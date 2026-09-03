use clap::Parser;
use p2p::{Node, PeerId, ProtocolId};
use p2p_log::{init, LogConfig};
use repair_bridge::{pump, PROTOCOL_ID};
use std::path::PathBuf;
use tokio::io::{self, AsyncWriteExt};

#[derive(Debug, Parser)]
#[command(name = "repair-bridge", about = "Connect MCP stdio to a repair helper")]
struct Args {
    #[arg(long)]
    ticket: String,
    #[arg(long)]
    peer: String,
    #[arg(long, required = true, action = clap::ArgAction::Append)]
    bootstrap: Vec<String>,
}

#[tokio::main]
async fn main() {
    let code = match run(Args::parse()).await {
        Ok(()) => 1,
        Err(error) => {
            tracing::error!(%error, "repair bridge stopped");
            eprintln!("repair-bridge: {error}");
            1
        }
    };
    std::process::exit(code);
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = init(LogConfig::default());
    let peer = parse_peer(&args.peer)?;
    let ticket = load_ticket(&args.ticket).await?;
    let node = build_node(&args.bootstrap).await?;
    wait_for_peer(&node, peer).await?;
    let protocol = ProtocolId::new(PROTOCOL_ID)?;
    let mut stream = node.new_stream(peer, protocol).await?;
    p2p_protocol::write_frame(&mut stream, ticket.as_bytes()).await?;
    stream.flush().await?;
    let (read, write) = tokio::io::split(stream);
    let stdin = io::stdin();
    let stdout = io::stdout();
    pump(stdin, write, read, stdout).await.map_err(Into::into)
}

async fn build_node(
    bootstrap: &[String],
) -> Result<Node, Box<dyn std::error::Error + Send + Sync>> {
    let data = std::env::temp_dir().join(format!("repair-bridge-{}", std::process::id()));
    Ok(Node::builder()
        .data_dir(data)
        .bootstrap(bootstrap.to_vec())
        .mdns(false)
        .build()
        .await?)
}

async fn wait_for_peer(
    node: &Node,
    peer: PeerId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut events = node.events();
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(30));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(p2p::NodeEvent::PeerDiscovered { peer: found, .. }) if found == peer => { node.connect(peer).await?; return Ok(()); },
                Ok(_) => {},
                Err(error) => return Err(format!("discovery event stream failed: {error}").into()),
            },
            _ = &mut deadline => return Err("peer discovery timed out".into()),
        }
    }
}

async fn load_ticket(value: &str) -> Result<String, std::io::Error> {
    match tokio::fs::read_to_string(value).await {
        Ok(ticket) => Ok(ticket),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(value.to_owned()),
        Err(error) => Err(error),
    }
}

fn parse_peer(value: &str) -> Result<PeerId, String> {
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|error| format!("invalid peer id: {error}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "peer id must be 32 bytes".to_owned())?;
    Ok(PeerId::from_bytes(bytes))
}

#[allow(dead_code)]
fn _path(_: PathBuf) {}
