
# docker_metrics_exporter

A lightweight [Prometheus](https://prometheus.io/) exporter for live Docker container statistics.

This Rust-based exporter continuously streams container resource metrics from `docker stats`, parses them, and serves them via a `/metrics` HTTP endpoint for Prometheus to scrape.  
Each container is labeled by its name.

---

## Features

- **Live container stats:** CPU, memory, network, and block I/O per container
- **Prometheus-compatible:** `/metrics` endpoint for easy scraping
- **Labels:** Each metric is labeled with the Docker container name
- **Configurable port:** Set the HTTP port with `-p PORT` (default: 9187)
- **Single-binary, no dependencies:** Runs anywhere Docker is available

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

- **For Rust**: apt install build-essential
- **Rust toolchain** ([Install via rustup](https://rustup.rs/))
- **Relogin** after Rust install or source the bash/sh profile...
- **Docker** must be installed and in the `PATH`
- Prometheus (for scraping, optional)

---

### Build and Install

1. **Clone this repository**:
    ```sh
    git clone https://github.com/ttww/docker_metrics_exporter.git
    cd docker_metrics_exporter
    ```

2. **Build a release version (optimized binary):**
    ```sh
    cargo build --release
    ```

3. **Copy the binary to `/usr/local/bin` for global use (may require sudo):**
    ```sh
    sudo cp target/release/docker_metrics_exporter /usr/local/bin/
    ```

---

### Usage Examples

| Command                                             | Description                        |
|-----------------------------------------------------|------------------------------------|
| `docker_metrics_exporter`                           | Listen on port 9187 (default)      |
| `docker_metrics_exporter -p 9000`                   | Listen on port 9000                |
| `docker_metrics_exporter -p 12313`                  | Listen on port 12313               |
| `docker_metrics_exporter -h`                        | Show help/usage                    |

Metrics endpoint will be available at e.g.:  
`http://localhost:9187/metrics`  
or  
`http://localhost:9000/metrics` (if you set a custom port)

---

### Prometheus scrape config

Add this to your `prometheus.yml` on your prometheus server:

```yaml
scrape_configs:
  - job_name: 'docker-metrics'
    static_configs:
      - targets: ['<your_host_or_ip>:9187']
```

Change the port if you are running the exporter on a different port.

---

## Run as a systemd service

Running the exporter as a systemd service ensures automatic startup and robust operation.

### 1. Create a systemd user/group

Create a dedicated user with limited permissions to run the exporter safely:

```sh
sudo useradd -r -s /bin/false docker-metrics
sudo usermod -aG docker docker-metrics
```

This user must belong to the `docker` group to run Docker commands.

### 2. Create the systemd unit file

Create `/etc/systemd/system/docker_metrics_exporter.service` with the following content:

```ini
[Unit]
Description=Docker Metrics Exporter for Prometheus
After=network.target docker.service
Requires=docker.service

[Service]
Type=simple
ExecStart=/usr/local/bin/docker_metrics_exporter -p 9187
Restart=on-failure
User=docker-metrics
Group=docker-metrics

[Install]
WantedBy=multi-user.target
```

### 3. Enable and start the service

Reload systemd, enable the service at boot, and start it now:

```sh
sudo systemctl daemon-reload
sudo systemctl enable docker_metrics_exporter
sudo systemctl start docker_metrics_exporter
sudo systemctl status docker_metrics_exporter
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



