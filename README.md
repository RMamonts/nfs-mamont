## NFS Mamont

Project aiming to implement NFS3 and NFS4 user space servers.

## Manual NFS server verification

After starting the `mirrorfs` example and mounting the export on Linux, you can run
a terminal-based end-to-end check script:

- Full mounted-export check: [scripts/nfs_manual_full_check.sh](scripts/nfs_manual_full_check.sh)

Example flow:

1. Start the server:
	- `cargo run --example mirrorfs -- /tmp/nfs-export 127.0.0.1:2049`
2. Mount the export:
	- `sudo mount -v -t nfs -o vers=3,tcp,proto=tcp,port=2049,mountport=2049,nolock 127.0.0.1:/tmp/nfs-export /tmp/nfs-mount`
3. Run the manual functional check:
	- `bash scripts/nfs_manual_full_check.sh /tmp/nfs-mount`

The script verifies:

- regular file create/read/overwrite/append/truncate/remove
- file rename and chmod
- hard links for files
- symbolic links for files
- directory create/rename/remove
- nested file operations inside directories
- symbolic links to directories
- writes through a directory symlink
- rejection of removing a non-empty directory
- larger file read/write/compare flow
- rename over an existing file

To keep the generated workspace for inspection, run:

- `bash scripts/nfs_manual_full_check.sh /tmp/nfs-mount --keep`

## Prometheus metrics

The `mirrorfs` example can expose Prometheus-compatible metrics on a separate
HTTP listener.

Example flow:

1. Start the NFS server and metrics endpoint:
	- `cargo run --example mirrorfs -- /tmp/nfs-export 127.0.0.1:2049 127.0.0.1:9100`
2. Check the metrics endpoint manually:
	- `curl http://127.0.0.1:9100/metrics`
3. Add the endpoint to Prometheus:

```yaml
scrape_configs:
	- job_name: nfs_mamont
		static_configs:
			- targets: ["127.0.0.1:9100"]
```

Common metric names:

- `nfs_mamont_connections_total`
- `nfs_mamont_requests_received_total`
- `nfs_mamont_replies_sent_total`
- `nfs_mamont_read_queue_depth`
- `nfs_mamont_write_queue_depth`

The metrics endpoint only serves `GET /metrics` and returns Prometheus text
format.

For a ready-to-use local observability stack, the repository now includes:

- [prometheus.yml](prometheus.yml)
- [docker-compose.yml](docker-compose.yml)
- [grafana/dashboards/nfs-mamont-overview.json](grafana/dashboards/nfs-mamont-overview.json)
- [grafana/dashboards/nfs-mamont-procedures.json](grafana/dashboards/nfs-mamont-procedures.json)

Example flow with Prometheus and Grafana:

1. Start `mirrorfs` with the metrics listener enabled:
	- `cargo run --example mirrorfs -- /tmp/nfs-export 127.0.0.1:2049 127.0.0.1:9100`
2. Start Prometheus and Grafana:
	- `docker compose up -d`
3. Open the UIs:
	- Prometheus: `http://127.0.0.1:9090`
	- Grafana: `http://127.0.0.1:3000`
4. Sign in to Grafana with:
	- user: `admin`
	- password: `admin`

The Grafana dashboards are provisioned automatically under the `NFS Mamont`
folder:

- `NFS Mamont Overview`
- `NFS Mamont Procedures`

The per-procedure metrics use Prometheus labels:

- `program` - `nfs` or `mount`
- `version` - RPC program version
- `procedure` - procedure name such as `read`, `write`, `lookup`, or `mnt`

Examples:

- `nfs_mamont_procedure_requests_received_total{program="nfs",procedure="read",version="3"}`
- `nfs_mamont_procedure_total_latency_average_micros{program="nfs",procedure="write",version="3"}`

## Contributing

Contributions are welcome! Check issue list.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
