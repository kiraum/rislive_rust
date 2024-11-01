// Required external crate imports for functionality
use clap::{Arg, Command};
use futures_util::{SinkExt, StreamExt};
use ipnetwork::IpNetwork;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use thiserror::Error;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};
use url::Url;

// Constants for WebSocket connection configuration
const RIS_WEBSOCKET_URL: &str = "wss://ris-live.ripe.net/v1/ws/";
const CLIENT_IDENTIFIER: &str = "RipeRisStreamer";
const VERSION: &str = env!("CARGO_PKG_VERSION");

// Custom error type for handling various error cases in the application
#[derive(Debug, Error)]
enum RisError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
}

// Structure for deserializing messages received from the RIS Live service
#[derive(Debug, Deserialize)]
struct RisMessage {
    data: serde_json::Value,
}

// Main parameters structure for RIS Live subscription
#[derive(Debug, Serialize, Deserialize)]
struct RisParams {
    #[serde(rename = "type")]
    msg_type: String,
    data: RisParamsData,
}

// Detailed configuration structure for RIS Live subscription parameters
#[derive(Debug, Serialize, Deserialize)]
struct RisParamsData {
    #[serde(rename = "socketOptions")]
    socket_options: SocketOptions,
    #[serde(rename = "moreSpecific")]
    more_specific: bool,
    #[serde(rename = "lessSpecific")]
    less_specific: bool,
    #[serde(rename = "autoReconnect")]
    auto_reconnect: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    type_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    require: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
}

// Socket options configuration structure
#[derive(Debug, Serialize, Deserialize)]
struct SocketOptions {
    #[serde(rename = "includeRaw")]
    include_raw: bool,
}

// Main streamer structure that manages the WebSocket connection
struct RipeRisStreamer {
    params: RisParams,
}

impl RipeRisStreamer {
    // Creates a new RipeRisStreamer instance from command line arguments
    fn new(matches: &clap::ArgMatches) -> Self {
        let socket_options = SocketOptions {
            include_raw: matches.get_flag("include-raw"),
        };

        let data = RisParamsData {
            socket_options,
            more_specific: matches.get_flag("more-specific"),
            less_specific: matches.get_flag("less-specific"),
            auto_reconnect: !matches.get_flag("disable-auto-reconnect"),
            host: matches.get_one::<String>("host").cloned(),
            type_filter: matches.get_one::<String>("type").cloned(),
            require: matches.get_one::<String>("key").cloned(),
            peer: matches.get_one::<String>("peer").cloned(),
            path: matches.get_one::<String>("aspath").cloned(),
            prefix: matches.get_one::<String>("prefix").cloned(),
        };

        RipeRisStreamer {
            params: RisParams {
                msg_type: "ris_subscribe".to_string(),
                data,
            },
        }
    }

    // Initiates and manages the WebSocket connection and message streaming
    async fn start_streaming(&self) -> Result<(), RisError> {
        let url = format!("{}?client={}", RIS_WEBSOCKET_URL, CLIENT_IDENTIFIER);
        let url = Url::parse(&url)?;

        debug!("Creating websocket connection...");
        let (ws_stream, _) = connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        debug!("Sending RIS parameters...");
        let params = serde_json::to_string(&self.params)?;
        debug!("Parameters sent: {}", params);
        write.send(Message::Text(params)).await?;

        info!("Listening...");
        debug!("Starting the reception loop...");

        while let Some(msg) = read.next().await {
            match msg {
                Ok(msg) => {
                    let msg_str = msg.to_string();
                    if msg_str.is_empty() {
                        continue; // Skip empty messages
                    }
                    match serde_json::from_str::<RisMessage>(&msg_str) {
                        Ok(parsed) => {
                            if let Some(msg_type) = parsed.data.get("type") {
                                if let Some(filter_type) = &self.params.data.type_filter {
                                    if msg_type.as_str() == Some(filter_type) {
                                        println!("{}", msg_str);
                                    }
                                } else {
                                    println!("{}", msg_str);
                                }
                            }
                        }
                        Err(e) => error!("Failed to parse message: {}", e),
                    }
                }
                Err(e) => error!("WebSocket error: {}", e),
            }
        }

        Ok(())
    }
}

// Validates RRC (Route Collector) format
fn validate_rrc(input: &str) -> Result<String, String> {
    if !Regex::new(r"^rrc\d{2}$").unwrap().is_match(input) {
        return Err(format!(
            "Invalid RRC format '{}'. Must be in format 'rrcXX' where X is a digit",
            input
        ));
    }
    Ok(input.to_string())
}

// Validates peer IP address format
fn validate_peer(peer: &str) -> Result<String, String> {
    if peer.contains(',') {
        return Err(
            "Multiple IP addresses are not supported. Please provide a single IP address"
                .to_string(),
        );
    }
    peer.parse::<IpAddr>()
        .map(|_| peer.to_string())
        .map_err(|_| "Invalid IP address format".to_string())
}

// Validates AS path format and values
fn validate_aspath(path: &str) -> Result<String, String> {
    let clean_path = path.trim_matches(|c| c == '^' || c == '$');
    for asn in clean_path.split(',') {
        if !asn.chars().all(char::is_numeric) {
            return Err(
                "AS path must contain only numbers, commas, and optional ^ $ anchors".to_string(),
            );
        }
        // Just parse as u32 since ASN can't be larger than that anyway
        asn.parse::<u32>()
            .map_err(|_| "Invalid AS number in path".to_string())?;
    }
    Ok(path.to_string())
}

// Validates network prefix format
fn validate_prefix(prefix: &str) -> Result<String, String> {
    let prefixes: Vec<&str> = prefix.split(',').collect();
    for p in prefixes {
        if !p.contains('/') {
            return Err("Network prefix must include mask in CIDR notation".to_string());
        }
        p.parse::<IpNetwork>()
            .map_err(|_| "Invalid network prefix format".to_string())?;
    }
    Ok(prefix.to_string())
}

// Main application entry point
#[tokio::main]
async fn main() -> Result<(), RisError> {
    let matches = Command::new("RIPE RIS Live Streamer")
        .version(VERSION)
        .about("Monitor the streams from RIPE RIS Live")
        .arg(
            Arg::new("host")
                .short('H')
                .long("host")
                .value_name("RRC")
                .help("Filter messages by specific RRCs (format: rrcXX)")
                .value_parser(validate_rrc),
        )
        .arg(
            Arg::new("type")
                .short('t')
                .long("type")
                .value_name("TYPE")
                .value_parser([
                    "UPDATE",
                    "OPEN",
                    "NOTIFICATION",
                    "KEEPALIVE",
                    "RIS_PEER_STATE",
                ])
                .help("Filter messages by BGP or RIS type"),
        )
        .arg(
            Arg::new("key")
                .short('k')
                .long("key")
                .value_name("KEY")
                .value_parser(["announcements", "withdrawals"])
                .help("Filter messages containing a specific key"),
        )
        .arg(
            Arg::new("peer")
                .short('p')
                .long("peer")
                .value_name("IP")
                .help("Filter messages by BGP peer IP address (single IP only)")
                .value_parser(validate_peer),
        )
        .arg(
            Arg::new("aspath")
                .short('a')
                .long("aspath")
                .value_name("PATH")
                .help("Filter by AS path")
                .value_parser(validate_aspath),
        )
        .arg(
            Arg::new("prefix")
                .short('f')
                .long("prefix")
                .value_name("PREFIX")
                .help(
                    "Filter UPDATE messages by IPv4/IPv6 prefix (e.g. 192.0.2.0/24,2001:db8::/32)",
                )
                .value_parser(validate_prefix),
        )
        .arg(
            Arg::new("more-specific")
                .short('m')
                .long("more-specific")
                .action(clap::ArgAction::SetTrue)
                .help("Match prefixes that are more specific"),
        )
        .arg(
            Arg::new("less-specific")
                .short('l')
                .long("less-specific")
                .action(clap::ArgAction::SetTrue)
                .help("Match prefixes that are less specific"),
        )
        .arg(
            Arg::new("include-raw")
                .short('r')
                .long("include-raw")
                .action(clap::ArgAction::SetTrue)
                .help("Include Base64-encoded original binary BGP message"),
        )
        .arg(
            Arg::new("disable-auto-reconnect")
                .short('d')
                .long("disable-auto-reconnect")
                .action(clap::ArgAction::SetTrue)
                .help("Disable auto-reconnect on connection drop"),
        )
        .arg(
            Arg::new("debug")
                .short('D')
                .long("debug")
                .action(clap::ArgAction::SetTrue)
                .help("Enable debug logging output"),
        )
        .get_matches();

    // Initialize logging based on debug flag
    if matches.get_flag("debug") {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();
    }

    let streamer = RipeRisStreamer::new(&matches);

    // Handle auto-reconnect logic
    if !matches.get_flag("disable-auto-reconnect") {
        loop {
            if let Err(e) = streamer.start_streaming().await {
                error!("Streamer encountered an error: {}", e);
            }
        }
    } else {
        streamer.start_streaming().await?;
    }

    Ok(())
}
