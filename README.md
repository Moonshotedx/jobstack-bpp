# BPP Onest Lite

A Beckn Provider Platform (BPP) backend service .

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.77+-dea056.svg?logo=rust)](https://www.rust-lang.org/)

## Features

- Beckn Protocol v2.0.0 compliant BPP implementation
- REST API for ONDC operations (search, select, init, confirm, status)
- Profile management with background sync
- PostgreSQL for persistent storage
- Redis for caching and session management
- Async Rust with Axum web framework
- Docker and Docker Compose support

## Prerequisites

- Rust 1.77+
- PostgreSQL 14+
- Redis 7+
- Docker & Docker Compose (optional)

## Quick Start

### 1. Clone and Build

```bash
cargo build --release
```

### 2. Configure Environment

Copy the example environment file and adjust values:

```bash
cp .env.example .env
```

Copy the example configuration and customize:

```bash
cp config/example.yaml config/custom.yaml
```

### 3. Database Setup

Run migrations:

```bash
./migrate.sh
```

Or manually with SQLx:

```bash
DATABASE_URL="postgres://user:pass@localhost:5432/db" cargo sqlx migrate run
```

### 4. Run

```bash
cargo run -- config/custom.yaml
```

## Docker

### Using Docker Compose

```bash
docker compose up -d
```

This starts:
- BPP service (port 3009)
- PostgreSQL (port 5432)
- Redis (port 6379)

### Manual Docker Build

```bash
docker build -t bpp-onest-lite .
docker run -p 3009:3009 bpp-onest-lite ./bpp-onest-lite config/local.yaml
```

## Configuration

### YAML Config (`config/*.yaml`)

| Section | Description |
|---------|-------------|
| `debug` | Enable debug logging |
| `use_mock_bpp_response` | Use mock responses for testing |
| `bpp` | BPP identification and network settings |
| `bap` | Default BAP (Beckn Acquisition Platform) settings |
| `http` | Server address and port |
| `provider_db` | External provider database URI |
| `redis` | Redis connection URL |
| `db` | PostgreSQL connection string |
| `cron` | Background job schedules |
| `auth` | API authentication settings |

### Environment Variables

See `.env.example` for all available variables.

## Project Structure

```
bpp-onest-lite/
├── src/
│   ├── config.rs         # Configuration loading
│   ├── main.rs           # Entry point
│   ├── http/             # HTTP server & routes
│   ├── services/         # Business logic
│   ├── models/           # Data models
│   ├── db/               # Database operations
│   ├── workers/          # Background workers
│   └── utils/            # Utilities
├── config/               # Configuration files
├── migrations/           # Database migrations
└── docker-compose.yml    # Docker services
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

Copyright (c) 2025 Dhiway Networks
