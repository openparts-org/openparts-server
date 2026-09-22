# openparts-server

OpenParts distribution API server (Architecture Specification section
20). Serves search, part/device/package lookup, provenance, and
on-demand CAD artifact generation (KiCad symbol/footprint, STEP) over
HTTP. Canonical Data editing is out of scope here -- this crate is
read-only; all writes happen through `openparts-data` Pull Requests.

## Status

First slice: loads `openparts-data` into an in-memory index at startup
(`src/store.rs`) instead of a real PostgreSQL-backed Importer
(Architecture Specification section 22) -- the HTTP layer above it is
written so that swap doesn't require route changes later. No
authentication, rate limiting, caching, or object storage yet.

## Running

```sh
# from this directory, with a sibling ../openparts-data checkout
cargo run
# or point at a different data directory / port
OPENPARTS_DATA_DIR=/path/to/openparts-data PORT=8080 cargo run
```

## API

| Route | Description |
| --- | --- |
| `GET /health` | liveness check |
| `GET /v1/search?q=...` | substring search over manufacturer/MPN |
| `GET /v1/parts/{manufacturer}/{mpn}` | Part |
| `GET /v1/parts/{manufacturer}/{mpn}/provenance` | Part+Device+Package provenance |
| `GET /v1/parts/{manufacturer}/{mpn}/revisions` | Device's declared silicon revisions |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts` | links to available artifact kinds |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-symbol?silicon_revision=...` | generated `.kicad_sym` text |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/kicad-footprint?silicon_revision=...` | generated `.kicad_mod` text |
| `GET /v1/parts/{manufacturer}/{mpn}/artifacts/step?silicon_revision=...` | generated STEP text |
| `GET /v1/devices/{id}` | Device (`id` may contain `/`, e.g. `raspberrypi/RP2040`) |
| `GET /v1/packages/{id}` | Package |

Artifact responses carry an `X-Content-Hash: sha256:<hex>` header
(Architecture Specification section 24, Content Addressing). Missing
required dimensions or an unknown `silicon_revision` return `422` with
a diagnostic body rather than guessing (Testing and Quality
Specification section 7).

## Deployment

`container/Containerfile` (Podman-first, OCI-compatible) and
`deploy/quadlet/openparts-server.container` (systemd-managed Podman
Quadlet unit) describe the intended production shape. Neither has been
exercised in this development environment (no container runtime
available here) -- treat a first real build/deploy as unverified until
run once.
