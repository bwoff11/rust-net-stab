# Network Stability Monitor (net-stab)

Rust Network Stability Monitor (or `rust-net-stab` for short) is a small, efficient, and easy-to-use utility that monitors the stability of your network by continuously pinging a set of configured endpoints. It also exposes metrics compatible with Prometheus, allowing you to track and visualize the network stability over time.

## Installing
You can build it from source by cloning this repository and running:

```
cargo build --release
```

The resulting binary will be placed in `./target/release/net-stab`.

## Getting Started

To start monitoring your network, you need to create a `config.yaml` file in the same directory as your `net-stab` executable. This file should specify the endpoints you want to monitor. Here's an example:

```yaml
endpoints:
  - name: "Google DNS"
    address: "8.8.8.8"
    location: "USA"
  - name: "Cloudflare DNS"
    address: "1.1.1.1"
    location: "USA"
```

You can list as many endpoints as you want. Each endpoint should have a `name` and an `address`. The `location` field is optional.

Once your `config.yaml` file is ready, you can start `rust-net-stab` by simply running the executable:

```
./rust-net-stab
```

## Prometheus Metrics

`rust-net-stab` exposes metrics at `http://localhost:9898/metrics`, which can be scraped by a Prometheus server. The exposed metrics include counts of successful and failed pings, as well as histograms of ping latencies, for each configured endpoint.

Here's an example of the metrics output:

```
# HELP ping_success Count of successful pings
# TYPE ping_success counter
ping_success{address="1.1.1.1",name="Cloudflare DNS"} 12
ping_success{address="8.8.8.8",name="Google DNS"} 12

# HELP ping_fail Count of failed pings
# TYPE ping_fail counter
ping_fail{address="1.1.1.1",name="Cloudflare DNS"} 0
ping_fail{address="8.8.8.8",name="Google DNS"} 0

# HELP ping_latency Ping latency in seconds
# TYPE ping_latency histogram
ping_latency_bucket{address="1.1.1.1",name="Cloudflare DNS",le="0.005"} 0

ping_latency_sum{address="1.1.1.1",name="Cloudflare DNS"} 0.1337374
ping_latency_count{address="1.1.1.1",name="Cloudflare DNS"} 12
```


## Contributing

Contributions are welcome! Please fork this repository and create a Pull Request with your changes.

For significant changes, please open an issue first to discuss what you would like to change.

## License

`rust-net-stab` is available under the MIT license.
