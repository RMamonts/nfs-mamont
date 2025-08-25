### Simple NFS Export Example

This is a default behavior of the example. It creates a single NFS export using the simple in-memory filesystem.

You can run it with:
```shell
cargo run --example demofs
```

And mount with:
```shell
sudo mount -o proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1,vers=3 127.0.0.1:/ /mnt/nfs
```


### Multiple NFS Exports Example

This example demonstrates how to set up multiple NFS exports.

It creates two exports, `/one` and `/two`, both using DemoFS from the previous example.

You can run it with:
```shell
cargo run --example demofs -- --multi-export
```

And mount with:
```shell
sudo mount -o proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1,vers=3 127.0.0.1:/one /mnt/nfs_one
sudo mount -o proto=tcp,port=11111,mountport=11111,nolock,addr=127.0.0.1,vers=3 127.0.0.1:/two /mnt/nfs_two
```

