# meting-api-rs

A Rust reimplementation of the [Metowolf/Meting](https://github.com/metowolf/Meting) API — **dual-deployable to Vercel (Fluid) and Cloudflare Workers (WASM)**.

Modern resource-oriented `/v1` API on top of NetEase `weapi`/`eapi` crypto, with a legacy `?server=&type=` compatibility layer for drop-in replacement of `meting-api` / `api.injahow.cn`.

## Features

- **Modern `/v1`** — `GET /v1/search`, `/v1/songs/:id`, `/v1/playlists/:id`, `/v1/songs/:id/url|pic|lyric`, `POST /v1/songs/batch`, `GET /v1/health` with unified envelope `{code, message, data, meta}` and RFC 9457 `application/problem+json` errors
- **Legacy compat** — `GET /api?server=netease&type=playlist&id=...` (also `/meting`, `/`) returns bare Meting arrays with `Deprecation` + `Sunset: 2027-01-01` + `Link: <.../v1>`
- **Crypto** — Pure-Rust `weapi` (AES-128-CBC double + RSA powmod) and `eapi` (AES-128-ECB + MD5), `eapi` for search, `weapi` for playlist/song/url/lyric
- **Covers** — `?param=500y500` (covers 6rem @ 2x, ~57KB) and `T002R500x500` for Tencent
- **Auth** — Optional `METING_TOKEN`; when set, `/url|/pic|/lyric|/batch` require `Authorization: Bearer <token>` (or `?token=`)
- **Deploy targets** — Vercel Rust Runtime (Fluid) and Cloudflare Workers WASM (`workers-rs`, `worker-build`), same `crates/core`

Platforms: **Netease** wired (search/playlist/song/url/lyric/pic). Tencent/Kugou/Kuwo stubs return `400 UNSUPPORTED_PLATFORM`.

## Quick Start

```bash
cargo run -p meting-server              # http://localhost:3000
PORT=3000 METING_TOKEN=secret cargo run -p meting-server

curl "http://localhost:3000/v1/health"
curl "http://localhost:3000/v1/search?platform=netease&q=hello&limit=2"
curl "http://localhost:3000/v1/songs/35847388?platform=netease"
curl "http://localhost:3000/v1/songs/35847388/url?platform=netease&br=320000"
curl "http://localhost:3000/v1/songs/35847388/pic?size=500&redirect=1"  # 307 -> CDN
curl "http://localhost:3000/api?server=netease&type=playlist&id=19723756" # legacy array
```

### nyx-player-solid

```ts
// Modern (recommended)
import { createModernMetingProvider } from "nyx-player-solid";
createModernMetingProvider({ baseURL: "https://meting-api-rs.<you>.workers.dev" });

// Legacy drop-in
import { createMetingProvider } from "nyx-player-solid";
createMetingProvider({ baseURL: "https://meting-api-rs.<you>.workers.dev/api" });
```

## API Reference

### Modern `/v1`

| Method | Path | Description | Cache |
|---|---|---|---|
| GET | `/v1/search?platform=netease&q=hello&limit=20&cursor=20` | Search (eapi) | `s-maxage=3600` |
| GET | `/v1/songs/:id?platform=netease` | Song detail | `s-maxage=3600` |
| GET | `/v1/playlists/:id?platform=netease` | Playlist tracks | `s-maxage=3600` |
| GET | `/v1/albums/:id` | Album (501 until wired) | — |
| GET | `/v1/artists/:id` | Artist (501 until wired) | — |
| GET | `/v1/songs/:id/url?platform=netease&br=320000` | Stream URL `{url, br, size}` | `private max-age=600` |
| GET | `/v1/songs/:id/url?redirect=1` | `307` to CDN | — |
| GET | `/v1/songs/:id/pic?size=500` | Cover `{url, size}` | `s-maxage=3600` |
| GET | `/v1/songs/:id/pic?redirect=1` | `307` to `?param=500y500` | — |
| GET | `/v1/songs/:id/lyric?platform=netease` | `{lrc, tlyric, yrc}` | `s-maxage=3600` |
| POST | `/v1/songs/batch` `{"platform":"netease","ids":["35847388"]}` | Batch detail | `s-maxage=3600` |
| GET | `/v1/health`, `/v1/openapi.json` | Health / OpenAPI 3.1 stub | `s-maxage=60` |

Success envelope:

```json
{"code":0,"message":"ok","data":[...],"meta":{"total":363,"cursor":"20","has_more":true}}
```

Error envelope (RFC 9457):

```json
{"type":"https://api.meting.rs/errors/not_found","title":"Not found","status":404,"code":"NOT_FOUND","detail":"..."}
```

Headers: `X-Meting-Api-Version: v1`, `Cache-Control`, `X-RateLimit-*` (when enabled).

### Legacy `/api`

```
GET /api?server=netease&type=search&id=hello&page=1&limit=20
GET /api?server=netease&type=song&id=35847388
GET /api?server=netease&type=playlist&id=19723756  # -> [{name, artist:"A / B", url:"/api?...&type=url...", pic:"/api?...&type=pic...", lrc:"/api?...&type=lrc..."}]
GET /api?server=netease&type=url&id=35847388       # 307 -> http://m8.music.126.net/...mp3
GET /api?server=netease&type=pic&id=35847388       # 307 -> https://p3.music.126.net/...?param=500y500
GET /api?server=netease&type=lrc&id=35847388       # text/plain LRC
```

Aliases: `server`/`platform`, `/meting`, `/`. Always returns `Deprecation: true`, `Sunset`, `Link: <.../v1>`, `X-Meting-Legacy: true`.

## Deployment

### Vercel (Fluid, Rust Runtime beta)

`vercel.json` already configures `runtime = "rust@2"` and `api/meting.rs` entry (Fluid compute).

```bash
vercel deploy
vercel env add METING_TOKEN  # optional
```

Set `CLOUDFLARE_PAGES_URL` / `VERCEL_URL` not needed. Functions run on Fluid with streaming and `64KB` env limit.

### Cloudflare Workers (WASM)

```bash
rustup target add wasm32-unknown-unknown
cargo install worker-build wrangler
wrangler secret put METING_TOKEN
wrangler deploy          # uses wrangler.toml + worker-build --release
wrangler dev             # http://localhost:8787
```

Switch to native `tokio` via Containers (paid plan) by uncommenting `[containers]` in `wrangler.toml` — same `meting-server` binary runs in a Linux VM.

## Development

```bash
cargo check --workspace
cargo test -p meting-core
cargo run -p meting-server -- --help
curl "http://localhost:3000/v1/playlists/19723756?platform=netease" | jq
```

Crypto lives in `crates/core/src/crypto/{weapi,eapi}.rs` (no `openssl` — `aes`+`cbc`/`ecb`+`num-bigint` only, compiles to `wasm32-unknown-unknown`).

## Environment

| Variable | Description |
|---|---|
| `PORT` | Server port (native only, default `3000`) |
| `METING_TOKEN` | Bearer token for sensitive routes; unset = public |
| `RUST_LOG` | `tracing` filter (`info`, `debug`) |

## Project Structure

```
crates/core    # DTOs, Platform, Error, weapi/eapi, mapping, NeteaseProvider
crates/server  # Axum Router, handlers, auth, provider (reqwest)
crates/worker  # workers-rs WASM entry, same core via Fetch
api/meting.rs  # Vercel Functions entry (vercel_runtime)
wrangler.toml  # Workers WASM
vercel.json    # Vercel Rust runtime
openapi.json   # OpenAPI 3.1 stub
```

## License

MIT — original Meting is MIT. Contributions welcome.
