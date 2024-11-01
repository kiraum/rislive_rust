# RIPE RIS Live Streaming Client

A Rust-based CLI tool for monitoring BGP updates from RIPE RIS (Routing Information Service) Live in real-time.

## Features

- Real-time BGP update monitoring;
- Flexible filtering options (RRC, BGP message types, AS paths, prefixes);
- Support for IPv4 and IPv6 prefixes;
- Auto-reconnect capability;
- Debug logging;
- Raw BGP message access.

## Prerequisites (build only)

- Rust
- Cargo

## Build new binary

Build from source:

```bash
cargo build --release
```

The pre-compiled binary (macOS Apple Silicon) will be available at bin/$version/rislive.

## Usage
```bash
./rislive -h
Monitor the streams from RIPE RIS Live

Usage: rislive [OPTIONS]

Options:
  -H, --host <RRC>              Filter messages by specific RRCs (format: rrcXX)
  -t, --type <TYPE>             Filter messages by BGP or RIS type [possible values: UPDATE, OPEN, NOTIFICATION, KEEPALIVE, RIS_PEER_STATE]
  -k, --key <KEY>               Filter messages containing a specific key [possible values: announcements, withdrawals]
  -p, --peer <IP>               Filter messages by BGP peer IP address (single IP only)
  -a, --aspath <PATH>           Filter by AS path
  -f, --prefix <PREFIX>         Filter UPDATE messages by IPv4/IPv6 prefix (e.g. 192.0.2.0/24,2001:db8::/32)
  -m, --more-specific           Match prefixes that are more specific
  -l, --less-specific           Match prefixes that are less specific
  -r, --include-raw             Include Base64-encoded original binary BGP message
  -d, --disable-auto-reconnect  Disable auto-reconnect on connection drop
  -D, --debug                   Enable debug logging output
  -h, --help                    Print help
```

Listen all messages:
```bash
./rislive
```

With filters:
```bash
./rislive --host rrc00 --type UPDATE --prefix 192.0.2.0/24
```

## Examples

Monitor all BGP updates:
```bash
./rislive --type UPDATE
```

Monitor specific prefix with debug output:
```bash
./rislive --prefix 2001:db8::/32 --debug
```

Filter by RRC and include raw messages:
```bash
./rislive --host rrc00 --include-raw
```

## Dependencies

Key dependencies:

* tokio (1.0+)
* tokio-tungstenite (0.20+)
* clap (4.0+)
* serde (1.0+)
* tracing (0.1+)

For a complete list, see `Cargo.toml`.

## Contributing

Contributions are welcome! Please ensure your code follows the existing style and includes appropriate tests.