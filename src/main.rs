use std::env;
use std::process::Stdio;
use std::sync::Arc;

use prometheus::{Encoder, GaugeVec, TextEncoder, Registry};
use serde::Deserialize;
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::Mutex};
use warp::Filter;

/// Print usage information to stderr.
fn usage() {
    eprintln!("Usage: docker_metrics_exporter [-p PORT]");
    eprintln!("  -p PORT    Optional. Port for the metrics endpoint (default: 9187)");
    eprintln!("  -h, --help Show this help message");
}

/// Struct for parsing JSON output from `docker stats`.
#[derive(Debug, Deserialize)]
struct DockerStat {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "CPUPerc")]
    cpu_perc: String,
    #[serde(rename = "MemUsage")]
    mem_usage: String,
    #[serde(rename = "NetIO")]
    net_io: String,
    #[serde(rename = "BlockIO")]
    block_io: String,
}

/// Convert strings like "4.2MiB", "123kB", "45B" to bytes (u64).
fn parse_bytes(s: &str) -> u64 {
    let units = [("GiB", 1024_u64.pow(3)), ("MiB", 1024_u64.pow(2)), ("kB", 1024), ("B", 1)];
    for (unit, factor) in units {
        if s.ends_with(unit) {
            let num = s[..s.len() - unit.len()].trim().replace(',', ".").parse::<f64>().unwrap_or(0.0);
            return (num * factor as f64) as u64;
        }
    }
    0
}

/// Parse IO or memory strings like "1.2kB / 2.3kB" or "-- / --" into two byte values.
fn parse_io(s: &str) -> (u64, u64) {
    let parts: Vec<&str> = s.split('/').map(|x| x.trim()).collect();
    let a = parts.get(0).map(|x| parse_bytes(x)).unwrap_or(0);
    let b = parts.get(1).map(|x| parse_bytes(x)).unwrap_or(0);
    (a, b)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Parse arguments: -p PORT, -h, --help
    let args: Vec<String> = env::args().collect();
    let mut port: u16 = 9187;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                usage();
                std::process::exit(0);
            }
            "-p" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: -p requires a port number");
                    usage();
                    std::process::exit(1);
                }
                match args[i + 1].parse() {
                    Ok(p) if p > 0 => port = p,
                    _ => {
                        eprintln!("Error: Invalid port number '{}'", args[i + 1]);
                        usage();
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
            arg => {
                eprintln!("Error: Unknown argument '{}'", arg);
                usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Initialize Prometheus metrics and prepare a Mutex for concurrent updates
    let registry = Registry::new();
    let metrics = Arc::new(Mutex::new(Metrics::new(&registry)));
    let metrics_clone = Arc::clone(&metrics);

    // Start `docker stats` as a subprocess and read its output line by line
    tokio::spawn(async move {
        let mut child = Command::new("docker")
            .arg("stats")
            .arg("--all")
            .arg("--format")
            .arg("{{json .}}")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn docker stats");

        let stdout = child.stdout.take().expect("No stdout");
        let mut reader = BufReader::new(stdout).lines();

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    if let Ok(stat) = serde_json::from_str::<DockerStat>(&line) {
                        let mut m = metrics_clone.lock().await;
                        m.update(&stat);
                    }
                }
                Ok(None) => break, // EOF
                Err(e) => {
                    eprintln!("Error reading line: {e}");
                    break;
                }
            }
        }
    });

    // HTTP /metrics endpoint for Prometheus scraping
    let metrics_route = warp::path!("metrics").map(move || {
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        TextEncoder::new().encode(&metric_families, &mut buffer).unwrap();
        warp::http::Response::builder()
            .header("Content-Type", "text/plain")
            .body(String::from_utf8(buffer).unwrap())
    });

    println!("Listening on http://0.0.0.0:{}/metrics", port);
    warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;

    Ok(())
}

/// Holds all Prometheus metrics for the Docker containers.
struct Metrics {
    cpu: GaugeVec,
    mem_usage: GaugeVec,
    mem_limit: GaugeVec,
    net_in: GaugeVec,
    net_out: GaugeVec,
    block_read: GaugeVec,
    block_write: GaugeVec,
}

impl Metrics {
    fn new(registry: &Registry) -> Self {
        let labels = &["name"];
        let cpu = GaugeVec::new(prometheus::Opts::new("docker_cpu_percent", "CPU usage %"), labels).unwrap();
        let mem_usage = GaugeVec::new(prometheus::Opts::new("docker_mem_usage_bytes", "Memory used"), labels).unwrap();
        let mem_limit = GaugeVec::new(prometheus::Opts::new("docker_mem_limit_bytes", "Memory limit"), labels).unwrap();
        let net_in = GaugeVec::new(prometheus::Opts::new("docker_net_input_bytes", "Network In"), labels).unwrap();
        let net_out = GaugeVec::new(prometheus::Opts::new("docker_net_output_bytes", "Network Out"), labels).unwrap();
        let block_read = GaugeVec::new(prometheus::Opts::new("docker_block_read_bytes", "Block I/O Read"), labels).unwrap();
        let block_write = GaugeVec::new(prometheus::Opts::new("docker_block_write_bytes", "Block I/O Write"), labels).unwrap();

        for m in [&cpu, &mem_usage, &mem_limit, &net_in, &net_out, &block_read, &block_write] {
            registry.register(Box::new(m.clone())).unwrap();
        }

        Metrics {
            cpu,
            mem_usage,
            mem_limit,
            net_in,
            net_out,
            block_read,
            block_write,
        }
    }

    /// Updates all Prometheus gauges for a given Docker container stat.
    fn update(&mut self, stat: &DockerStat) {
        let name = stat.name.as_str();

        let cpu_val = stat.cpu_perc.trim_end_matches('%').replace(",", ".").parse::<f64>().unwrap_or(0.0);
        self.cpu.with_label_values(&[name]).set(cpu_val);

        let mem_parts: Vec<&str> = stat.mem_usage.split('/').map(|x| x.trim()).collect();
        let mem_usage = parse_bytes(mem_parts.get(0).unwrap_or(&"0"));
        let mem_limit = parse_bytes(mem_parts.get(1).unwrap_or(&"0"));
        self.mem_usage.with_label_values(&[name]).set(mem_usage as f64);
        self.mem_limit.with_label_values(&[name]).set(mem_limit as f64);

        let (net_in, net_out) = parse_io(&stat.net_io);
        self.net_in.with_label_values(&[name]).set(net_in as f64);
        self.net_out.with_label_values(&[name]).set(net_out as f64);

        let (blk_read, blk_write) = parse_io(&stat.block_io);
        self.block_read.with_label_values(&[name]).set(blk_read as f64);
        self.block_write.with_label_values(&[name]).set(blk_write as f64);
    }
}


