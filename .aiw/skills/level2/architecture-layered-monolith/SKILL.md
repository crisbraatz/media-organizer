---
name: architecture-layered-monolith
description: Load when working with architecture-layered-monolith in media-organizer.
level: 2
kind: architecture
version: Layered modular monolith
sources:
  - https://learn.microsoft.com/en-us/dotnet/architecture/modern-web-apps-azure/common-web-application-architectures — Microsoft
fetched: 2026-11-24
---

# architecture-layered-monolith

Build one executable with internal modules. CLI calls application, application uses domain, domain depends on neither terminal nor filesystem. Infrastructure implements external operations as needed. For example, the application collects capture times through an adapter and calls the domain to calculate names. Test pure rules in memory and filesystem behavior against temporary directories.
