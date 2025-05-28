use std::env;
use std::process::Stdio;
use std::sync::Arc;

use influxdb::{Client, InfluxDbWriteable, Timestamp};
use prometheus::{Encoder, GaugeVec, TextEncoder, Registry};
use serde::Deserialize;
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command, sync::Mutex};
use warp::Filter;
use chrono::Utc;

/// Print usage information
fn usage() {
    eprintln!("Usage: docker_metrics_exporter [--target prometheus|influxdb] [-p PORT] [--host HOST] [--db DB]");
    eprintln!("  --target   prometheus (default) or influxdb");
    eprintln!("  -p, --port   Port for HTTP (Prometheus) or InfluxDB server");
    eprintln!("  --host       InfluxDB host (default: localhost)");
    eprintln!("  --db         InfluxDB database (default: metrics)");
    eprintln!("  -h, --help   Show this help");
}

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

#[derive(InfluxDbWriteable)]
struct DockerMetrics {
    time: Timestamp,
    #[influxdb(tag)] name: String,
    cpu_percent: f64,
    mem_usage: u64,
    mem_limit: u64,
    net_input: u64,
    net_output: u64,
    block_read: u64,
    block_write: u64,
}

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
fn parse_io(s: &str) -> (u64, u64) {
    let parts: Vec<&str> = s.split('/').map(|x| x.trim()).collect();
    let a = parts.get(0).map(|x| parse_bytes(x)).unwrap_or(0);
    let b = parts.get(1).map(|x| parse_bytes(x)).unwrap_or(0);
    (a, b)
}

/// Parse DockerStat into all metric values
fn parse_stat(stat: &DockerStat) -> (f64, u64, u64, u64, u64, u64, u64) {
    let cpu = stat.cpu_perc.trim_end_matches('%').replace(",", ".").parse::<f64>().unwrap_or(0.0);
    let mem_parts: Vec<&str> = stat.mem_usage.split('/').map(|x| x.trim()).collect();
    let mem_usage = parse_bytes(mem_parts.get(0).unwrap_or(&"0"));
    let mem_limit = parse_bytes(mem_parts.get(1).unwrap_or(&"0"));
    let (net_in, net_out) = parse_io(&stat.net_io);
    let (blk_read, blk_write) = parse_io(&stat.block_io);
    (cpu, mem_usage, mem_limit, net_in, net_out, blk_read, blk_write)
}

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
        Metrics { cpu, mem_usage, mem_limit, net_in, net_out, block_read, block_write }
    }
    fn update(&mut self, stat: &DockerStat) {
        let name = stat.name.as_str();
        let (cpu, mem_usage, mem_limit, net_in, net_out, blk_read, blk_write) = parse_stat(stat);
        self.cpu.with_label_values(&[name]).set(cpu);
        self.mem_usage.with_label_values(&[name]).set(mem_usage as f64);
        self.mem_limit.with_label_values(&[name]).set(mem_limit as f64);
        self.net_in.with_label_values(&[name]).set(net_in as f64);
        self.net_out.with_label_values(&[name]).set(net_out as f64);
        self.block_read.with_label_values(&[name]).set(blk_read as f64);
        self.block_write.with_label_values(&[name]).set(blk_write as f64);
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Argument parsing
    let args: Vec<String> = env::args().collect();
    let mut target = "prometheus".to_string();
    let mut port = 9187;
    let mut host = "localhost".to_string();
    let mut db = "metrics".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => { if i+1 < args.len() { target = args[i+1].clone(); i += 1; } }
            "--port" | "-p" => { if i+1 < args.len() { port = args[i+1].parse().unwrap_or(9187); i += 1; } }
            "--host" => { if i+1 < args.len() { host = args[i+1].clone(); i += 1; } }
            "--db" => { if i+1 < args.len() { db = args[i+1].clone(); i += 1; } }
            "-h" | "--help" => { usage(); return Ok(()); }
            _ => {}
        }
        i += 1;
    }

    // Shared Docker stats reader
    let mut child = Command::new("docker")
        .arg("stats")
        .arg("--format")
        .arg("{{json .}}")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn docker stats");
    let stdout = child.stdout.take().expect("No stdout");
    let mut reader = BufReader::new(stdout).lines();

    if target == "prometheus" {
        // Setup Prometheus exporter
        let registry = Registry::new();
        let metrics = Arc::new(Mutex::new(Metrics::new(&registry)));
        let metrics_clone = Arc::clone(&metrics);

        // Spawn update task
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(stat) = serde_json::from_str::<DockerStat>(&line) {
                    let mut m = metrics_clone.lock().await;
                    m.update(&stat);
                }
            }
        });

        // HTTP endpoint
        let metrics_route = warp::path!("metrics").map(move || {
            let metric_families = registry.gather();
            let mut buffer = Vec::new();
            TextEncoder::new().encode(&metric_families, &mut buffer).unwrap();
            warp::http::Response::builder()
                .header("Content-Type", "text/plain")
                .body(String::from_utf8(buffer).unwrap())
        });

        println!("Prometheus endpoint on http://0.0.0.0:{}/metrics", port);
        warp::serve(metrics_route).run(([0,0,0,0], port)).await;
    } else if target == "influxdb" {
        // Setup InfluxDB client
        let client = Client::new(format!("http://{}:{}", host, port), db);

        // Main loop: read docker stats and write to InfluxDB
        while let Ok(Some(line)) = reader.next_line().await {
            if let Ok(stat) = serde_json::from_str::<DockerStat>(&line) {
                let (cpu, mem_usage, mem_limit, net_in, net_out, blk_read, blk_write) = parse_stat(&stat);
                let metrics = DockerMetrics {
                    time: Timestamp::from(Utc::now()),
                    name: stat.name.clone(),
                    cpu_percent: cpu,
                    mem_usage,
                    mem_limit,
                    net_input: net_in,
                    net_output: net_out,
                    block_read: blk_read,
                    block_write: blk_write,
                };
                if let Err(e) = client.query(metrics.into_query("docker_stats")).await {
                    eprintln!("InfluxDB write error: {}", e);
                }
            }
        }
    } else {
        usage();
        return Ok(());
    }
    Ok(())
}


