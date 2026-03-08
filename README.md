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

## Contributing

Contributions are welcome! Check issue list.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
