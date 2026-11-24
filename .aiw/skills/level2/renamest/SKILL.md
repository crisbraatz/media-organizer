---
name: renamest
description: Use when changing the exclusive filesystem rename adapter.
level: 2
kind: technology
version: 0.1.3
sources:
  - https://docs.rs/crate/renamest/0.1.3 — Renamest maintainers
  - https://learn.microsoft.com/openspecs/windows_protocols/ms-fsa/87f86c9b-6c2a-4803-84b7-131a74a434fa — Microsoft
fetched: 2026-11-24
---

# Exclusive renames

Use `rename_exclusive` in infrastructure for both forward and recovery moves. It calls the native no-replace operation on supported platforms. Treat unsupported operations and occupied destinations as failures. Apply the same canonical path and source checks to staging, finalization and recovery. Keep the complete source observation for confirmation revalidation. On Windows, a rename can attach a previous creation timestamp to a reused name, so per-move checks compare size and modification time. Test occupied destinations on real temporary files and inject failures for recovery tests.
