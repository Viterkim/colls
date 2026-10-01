# Platforms

The default uses `Arc` and a blocking mutex, on native Rust or wasm. Use whatever executor you already have.

For `no_std + alloc`, or an app that stays on one thread:

```toml
[dependencies]
colls = { version = "0.0.1", default-features = false }
```

That uses `Rc<RefCell<_>>` behind the same `.with()` calls. Spawn its handles on a local executor, like Tokio's `LocalSet`. If another dependency enables `colls/std`, Cargo picks the default backend for everyone using that version.

With `no_std` your app supplies the allocator. Embassy tasks on one executor can use it. Sharing across interrupts needs an embedded backend we haven't added yet.
