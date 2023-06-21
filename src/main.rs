use prometheus::{Encoder, TextEncoder, register_counter_vec, register_histogram_vec};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::fs;
use warp::Filter;
use log::{info, Level};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init_with_level(Level::Info).unwrap(); // Initialize logger
    info!("Prometheus metrics are being exposed at http://localhost:9898/metrics");

    // Create Prometheus counters for successful and failed pings, and histogram for latency
    let ping_success_counter = register_counter_vec!("ping_success", "Count of successful pings", &["name", "address"]).unwrap();
    let ping_fail_counter = register_counter_vec!("ping_fail", "Count of failed pings", &["name", "address"]).unwrap();
    let ping_latency_histogram = register_histogram_vec!("ping_latency", "Ping latency in seconds", &["name", "address"]).unwrap();

    // Deserialize the config.yaml file into a Config struct
    let config: Config = serde_yaml::from_str(&fs::read_to_string("config.yaml")?)?;

    // Create a vector to hold all the ping operations (known as "handles")
    let mut handles = Vec::new();

    // For each endpoint defined in the config file, create a new asynchronous task
    for endpoint in config.endpoints {
        let success_counter = ping_success_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
        let fail_counter = ping_fail_counter.with_label_values(&[&endpoint.name, &endpoint.address]);
        let latency_histogram = ping_latency_histogram.with_label_values(&[&endpoint.name, &endpoint.address]);

        let handle = tokio::spawn(async move {
            // Inside each task, loop indefinitely
            loop {
                // Define the "ping" command differently based on the platform we're on
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

                // If the command was successful, increment the success counter and record the latency. Otherwise, increment the failure counter.
                if output.status.success() {
                    success_counter.inc();
                    latency_histogram.observe(duration.as_secs_f64());
                } else {
                    fail_counter.inc();
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        });

        handles.push(handle);
    }

    // Start a web server that exposes the metrics on /metrics
    let metrics_route = warp::path!("metrics").map(move || {
        let encoder = TextEncoder::new();
        let mut buffer = vec![];
        let metric_families = prometheus::gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8(buffer).unwrap()
    });

    let metrics_server = warp::serve(metrics_route).run(([127, 0, 0, 1], 9898));
    
    // Spawn the metrics server separately, so it doesn't interfere with your main logic
    tokio::spawn(metrics_server);

    // Await all the handles, i.e. perform all the pings concurrently and wait for them all to finish.
    for handle in handles {
        handle.await?;
    }

    Ok(())
}
