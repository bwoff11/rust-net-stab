use std::fs;
use std::process::Command;
use std::time::Instant;

use log::{info, Level};
use prometheus::{Encoder, Gauge, HistogramVec, IntCounterVec, TextEncoder};
use serde::{Deserialize, Serialize};
use sys_info::{cpu_num, loadavg, mem_info};
use tokio::time::Duration;
use warp::Filter;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    endpoints: Vec<Endpoint>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Endpoint {
    name: String,
    address: String,
    location: Option<String>,
}

async fn update_system_metrics(cpu_gauge: Gauge, load_avg_gauge: Gauge, mem_total_gauge: Gauge) {
    loop {
        if let Ok(cpu_count) = cpu_num() {
            cpu_gauge.set(cpu_count as f64);
        }

        if let Ok(load) = loadavg() {
            load_avg_gauge.set(load.one);
        }

        if let Ok(mem) = mem_info() {
            mem_total_gauge.set(mem.total as f64);
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn ping_endpoint(
    endpoint: Endpoint,
    success_counter: IntCounterVec,
    fail_counter: IntCounterVec,
    latency_histogram: HistogramVec,
) {
    let success_metric = success_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
    let fail_metric = fail_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
    let latency_metric = latency_histogram.with_label_values(&[&endpoint.name, &endpoint.address]);

    loop {
        let start = Instant::now();
        let output = ping(&endpoint.address);
        let duration = start.elapsed();

        match output {
            Ok(_) => {
                success_metric.inc();
                latency_metric.observe(duration.as_secs_f64());
            }
            Err(_) => {
                fail_metric.inc();
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn ping(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = if cfg!(target_family = "unix") {
        Command::new("ping")
            .arg("-c")
            .arg("1")
            .arg(address)
            .output()?
    } else if cfg!(target_family = "windows") {
        Command::new("ping")
            .arg("-n")
            .arg("1")
            .arg(address)
            .output()?
    } else {
        return Err("Unsupported platform".into());
    };

    if output.status.success() {
        Ok(())
    } else {
        Err("Ping failed".into())
    }
}

async fn serve_metrics() {
    let metrics_route = warp::path!("metrics").map(|| {
        let encoder = TextEncoder::new();
        let mut buffer = Vec::new();
        let metric_families = prometheus::gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    });

    let metrics_server = warp::serve(metrics_route).run(([127, 0, 0, 1], 9898));
    metrics_server.await;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(Level::Info).unwrap();
    info!("Prometheus metrics are being exposed at http://localhost:9898/metrics");

    let cpu_gauge = prometheus::register_gauge!("system_cpu_cores", "Number of CPU cores").unwrap();
    let load_avg_gauge = prometheus::register_gauge!("system_load_average", "System load average").unwrap();
    let mem_total_gauge = prometheus::register_gauge!("system_memory_total", "Total system memory").unwrap();

    let ping_success_counter =
        prometheus::register_int_counter_vec!("ping_success", "Count of successful pings", &["name", "address"]).unwrap();
    let ping_fail_counter =
        prometheus::register_int_counter_vec!("ping_fail", "Count of failed pings", &["name", "address"]).unwrap();
    let ping_latency_histogram =
        prometheus::register_histogram_vec!("ping_latency", "Ping latency in seconds", &["name", "address"]).unwrap();

    let config: Config = serde_yaml::from_str(&fs::read_to_string("config.yaml")?)?;

    let mut handles = Vec::new();

    handles.push(tokio::spawn(update_system_metrics(
        cpu_gauge,
        load_avg_gauge,
        mem_total_gauge,
    )));

    for endpoint in config.endpoints {
        let handle = tokio::spawn(ping_endpoint(
            endpoint,
            ping_success_counter.clone(),
            ping_fail_counter.clone(),
            ping_latency_histogram.clone(),
        ));
        handles.push(handle);
    }

    handles.push(tokio::spawn(serve_metrics()));

    for handle in handles {
        handle.await?;
    }

    Ok(())
}
