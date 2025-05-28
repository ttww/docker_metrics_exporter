
# docker_metrics_exporter

A lightweight [Prometheus](https://prometheus.io/) exporter for live Docker container statistics, with optional InfluxDB export.

This Rust-based exporter streams container resource metrics from `docker stats` and either serves them via a `/metrics` HTTP endpoint for Prometheus **or** writes them to InfluxDB, depending on the `--target` argument.

---

## Features

- **Live container stats:** CPU, memory, network, and block I/O per container
- **Prometheus-compatible:** `/metrics` endpoint for easy scraping
- **InfluxDB-compatible:** Direct write to InfluxDB (2.x or 1.x)
- **Labels:** Each metric is labeled with the Docker container name
- **Configurable target:** `--target prometheus` (default) or `--target influxdb`
- **Configurable HTTP/Influx port and host**

---

## Exported Metrics

- `docker_cpu_percent{name}` – CPU usage (%)
- `docker_mem_usage_bytes{name}` – Memory usage (bytes)
- `docker_mem_limit_bytes{name}` – Memory limit (bytes)
- `docker_net_input_bytes{name}` – Network input (bytes)
- `docker_net_output_bytes{name}` – Network output (bytes)
- `docker_block_read_bytes{name}` – Block I/O read (bytes)
- `docker_block_write_bytes{name}` – Block I/O write (bytes)

---

## Quick Start

### Prerequisites

- **Rust toolchain** ([Install via rustup](https://rustup.rs/))
- **Docker** must be installed and in the `PATH`
- Prometheus and/or InfluxDB as desired

---

### Build and Install

1. **Clone this repository**:
    ```sh
    git clone <your-repo-url>
    cd docker_metrics_exporter
    ```

2. **Add dependencies** (if needed, see below for Cargo.toml additions).

3. **Build a release version:**
    ```sh
    cargo build --release
    ```

4. **Copy the binary to `/usr/local/bin` (may require sudo):**
    ```sh
    sudo cp target/release/docker_metrics_exporter /usr/local/bin/
    ```

---

### Usage Examples

| Command                                             | Description                        |
|-----------------------------------------------------|------------------------------------|
| `docker_metrics_exporter`                           | Prometheus mode, port 9187         |
| `docker_metrics_exporter --target prometheus -p 9000` | Prometheus, custom port 9000     |
| `docker_metrics_exporter --target influxdb --host 127.0.0.1 --port 8086 --db metrics` | InfluxDB mode |
| `docker_metrics_exporter -h`                        | Show help/usage                    |

---

### Prometheus scrape config

```yaml
scrape_configs:
  - job_name: 'docker-metrics'
    static_configs:
      - targets: ['localhost:9187']
```

---

### InfluxDB usage

- **InfluxDB must be accessible from this exporter.**
- Write-compatibility is for Influx 1.x and 2.x HTTP APIs.
- The default measurement is `docker_stats`.
- Adjust the database/organization name as required.

---

## Run as a systemd service

1. **Create a dedicated user (optional, but recommended):**
    ```sh
    sudo useradd -r -s /bin/false docker-metrics
    sudo usermod -aG docker docker-metrics
    ```

2. **Create `/etc/systemd/system/docker_metrics_exporter.service`:**
    ```ini
    [Unit]
    Description=Docker Metrics Exporter for Prometheus or InfluxDB
    After=network.target docker.service
    Requires=docker.service

    [Service]
    Type=simple
    ExecStart=/usr/local/bin/docker_metrics_exporter --target prometheus -p 9187
    Restart=on-failure
    User=docker-metrics
    Group=docker-metrics

    [Install]
    WantedBy=multi-user.target
    ```
   - Change `ExecStart` as needed for your config.

3. **Reload, enable, and start:**
    ```sh
    sudo systemctl daemon-reload
    sudo systemctl enable docker_metrics_exporter
    sudo systemctl start docker_metrics_exporter
    sudo systemctl status docker_metrics_exporter
    ```

---

## Cargo.toml additions

```toml
prometheus = "0.13"
warp = "0.3"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
influxdb = { version = "0.7", features = ["derive", "reqwest-client"] }
chrono = { version = "0.4", features = ["serde"] }
```

---

## License

MIT License

```
MIT License

Copyright (c) 2024

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```


