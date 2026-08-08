# mos-projects — Workspace for the mos OS kernel

Cargo workspace hosting the [mos](https://github.com/WgZgYn/mos) operating system kernel and its companion crates.

## Contents

- `mos` — the kernel itself
- `fs/ramfs` — in-memory RAM filesystem (inode / VFS / frame management)
- `fs/mem_fs` — memory-backed FS layer
- `libs/allocators` — kernel allocators (bump allocator, etc.)
- `ktests` — kernel tests runner
- `ramfs_builder`, `user`, and various `mos_*` / `vfs_*` crates

## Layout

```
mos-projects/
├── mos/            # kernel entry point
├── fs/             # filesystems
├── libs/           # allocators & pure libs
├── ktests/         # kernel-space tests
└── Cargo.toml      # workspace definition
```

## Requirements

Same as [mos](https://github.com/WgZgYn/mos): nightly Rust, `llvm-tools-preview`, `x86_64-unknown-none`, QEMU + OVMF, clang.

## License

MIT
