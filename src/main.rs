use prometheus::{Encoder, TextEncoder, register_counter_vec, register_histogram_vec, register_gauge, Gauge};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::fs;
use warp::Filter;
use log::{info, Level};
use sys_info::{cpu_num, loadavg, mem_info};

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
        let cpu_count = cpu_num().unwrap();
        let load = loadavg().unwrap();
        let mem = mem_info().unwrap();

        cpu_gauge.set(cpu_count as f64);
        load_avg_gauge.set(load.one);
        mem_total_gauge.set(mem.total as f64);

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn ping_endpoint(endpoint: Endpoint, success_counter: prometheus::CounterVec, fail_counter: prometheus::CounterVec, latency_histogram: prometheus::HistogramVec) {
    let success_metric = success_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
    let fail_metric = fail_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
    let latency_metric = latency_histogram.with_label_values(&[&endpoint.name, &endpoint.address]);

    loop {
        let start = std::time::Instant::now();
        let output = if cfg!(target_family = "unix") {
            Command::new("ping")
                .arg("-c")
                .arg("1")
                .arg(&endpoint.address)
                .output()
                .expect("Failed to execute command")
        } else if cfg!(target_family = "windows") {
            Command::new("ping")
                .arg("-n")
                .arg("1")
                .arg(&endpoint.address)
                .output()
                .expect("Failed to execute command")
        } else {
            panic!("Unsupported platform");
        };
        let duration = start.elapsed();

        if output.status.success() {
            success_metric.inc();
            latency_metric.observe(duration.as_secs_f64());
        } else {
            fail_metric.inc();
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(Level::Info).unwrap();
    info!("Prometheus metrics are being exposed at http://localhost:9898/metrics");

    let cpu_gauge = register_gauge!("system_cpu_cores", "Number of CPU cores").unwrap();
    let load_avg_gauge = register_gauge!("system_load_average", "System load average").unwrap();
    let mem_total_gauge = register_gauge!("system_memory_total", "Total system memory").unwrap();

    let sys_metrics_handle = tokio::spawn(update_system_metrics(cpu_gauge.clone(), load_avg_gauge.clone(), mem_total_gauge.clone()));


    let ping_success_counter = register_counter_vec!("ping_success", "Count of successful pings", &["name", "address"]).unwrap();
    let ping_fail_counter = register_counter_vec!("ping_fail", "Count of failed pings", &["name", "address"]).unwrap();
    let ping_latency_histogram = register_histogram_vec!("ping_latency", "Ping latency in seconds", &["name", "address"]).unwrap();

    let config: Config = serde_yaml::from_str(&fs::read_to_string("config.yaml")?)?;

    let mut handles = Vec::new();

    for endpoint in config.endpoints {
        let handle = tokio::spawn(ping_endpoint(
            endpoint, 
            ping_success_counter.clone(), // Cloning the Arc
            ping_fail_counter.clone(),    // Cloning the Arc
            ping_latency_histogram.clone() // Cloning the Arc
        ));
        handles.push(handle);
    }

    handles.push(sys_metrics_handle);

    let metrics_route = warp::path!("metrics").map(move || {
        let encoder = TextEncoder::new();
        let mut buffer = vec![];
        let metric_families = prometheus::gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    });

    let metrics_server = warp::serve(metrics_route).run(([127, 0, 0, 1], 9898));
    tokio::spawn(metrics_server);

    for handle in handles {
        handle.await?;
    }

    Ok(())
}
