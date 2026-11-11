# Stellar-tipchain-backend

A REST API backend for a Stellar-based tip chain. Creators register with a username and Stellar wallet address, and supporters send tips by submitting verified on-chain Stellar transactions.

---

## Table of Contents

- [Overview](#overview)
- [How It Works](#how-it-works)
- [Features](#features)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Configuration](#configuration)
- [Running the Server](#running-the-server)
- [Running with Docker Compose](#running-with-docker-compose)
- [API Reference](#api-reference)
- [Database Migrations](#database-migrations)
- [Project Structure](#project-structure)
- [Current Status](#current-status)
- [License](#license)

---

## Overview

Stellar-tipchain-backend is a Rust web service built with [Axum](https://github.com/tokio-rs/axum) and [SQLx](https://github.com/launchbadge/sqlx) that enables a creator tipping system on top of the [Stellar](https://stellar.org) blockchain. It acts as a trusted intermediary that:

1. Stores creator profiles (username + Stellar wallet address) in PostgreSQL.
2. Accepts tip submissions from supporters.
3. Verifies each tip transaction directly on the Stellar network via the [Horizon API](https://developers.stellar.org/api) before persisting it.

No funds are held by this backend — all money movement happens on-chain. The backend only records and verifies.

---

## How It Works

### Creator Registration

A creator calls `POST /creators` with their username and Stellar wallet address. The backend stores this in the `creators` table. The wallet address is the destination where supporters will send XLM (or other Stellar assets) on-chain.

### Sending a Tip (Supporter Flow)

1. The supporter opens their Stellar wallet and sends a payment to the creator's wallet address on the Stellar network.
2. After the transaction is confirmed on-chain, the supporter submits the transaction hash to `POST /tips` along with the creator's username and the tip amount.
3. The backend calls the Stellar Horizon API to look up the transaction by hash and confirms:
   - The transaction exists on the network.
   - The transaction has `"successful": true`.
4. If verification passes, the tip is saved to the `tips` table and a `201 Created` response is returned.
5. If the transaction cannot be found or was not successful, a `422 Unprocessable Entity` is returned.
6. If the Horizon API is unreachable, a `502 Bad Gateway` is returned.

### Querying Tips

Anyone can call `GET /creators/:username/tips` to retrieve the full tip history for a creator.

### On-Chain Verification Flow

```
Supporter wallet ──sends XLM──► Creator wallet (Stellar network)
                                        │
                                  tx_hash returned
                                        │
Supporter ──POST /tips──► Backend ──GET /transactions/:hash──► Horizon API
                                        │
                                  verified ✓
                                        │
                              saved to PostgreSQL
```

---

## Features

- Register creators with a username and Stellar wallet address
- Input validation for usernames, Stellar wallet addresses, tip amounts, and transaction hashes
- Verify Stellar transactions on-chain via Horizon API before recording tips
- Retrieve creator profiles by username
- List all tips for a creator
- `GET /health` liveness endpoint
- Automatic database migrations on startup via SQLx
- Reversible migrations (`sqlx migrate revert` supported)
- CORS enabled for all origins
- Structured logging via `tracing`
- Unit and integration test coverage (validation, services, controllers)

---

## Prerequisites

- [Rust](https://rustup.rs) (edition 2021)
- PostgreSQL 12 or later
- A Stellar account and access to the Stellar Horizon API (testnet or mainnet)

---

## Getting Started

```bash
# Clone the repository
git clone https://github.com/your-org/stellar-tipchain-backend.git
cd stellar-tipchain-backend

# Copy the example environment file and fill in your values
cp .env.example .env

# Build the project
cargo build
```

---

## Configuration

All configuration is provided through environment variables. Copy `.env.example` to `.env` and set each value:

| Variable          | Required | Default                              | Description                                      |
|-------------------|----------|--------------------------------------|--------------------------------------------------|
| `DATABASE_URL`    | Yes      | —                                    | PostgreSQL connection string, e.g. `postgres://user:password@localhost/tipchain` |
| `STELLAR_NETWORK` | No       | `testnet`                            | Stellar network to use: `testnet` or `mainnet`   |
| `STELLAR_RPC_URL` | No       | `https://soroban-testnet.stellar.org`| Soroban RPC endpoint                             |
| `PORT`            | No       | `8000`                               | Port the HTTP server listens on                  |

Logging verbosity is controlled by the `RUST_LOG` environment variable (defaults to `stellar_tipchain_backend=debug,tower_http=debug`).

---

## Running the Server

```bash
# Development
cargo run

# Production (optimised binary)
cargo build --release
./target/release/stellar-tipchain-backend
```

The server applies any pending database migrations automatically on startup, then begins listening on `0.0.0.0:<PORT>`.

---

## Running with Docker Compose

To run the complete stack (Rust API + PostgreSQL) locally with a single command:

```bash
# Build and start both API and database services
docker compose up --build

# Run in background (detached mode)
docker compose up -d

# Stop services (preserves database data via named volume)
docker compose down

# Stop services and remove persistent volume
docker compose down -v
```

- **API Endpoint:** `http://localhost:8000`
- **PostgreSQL Database:** `localhost:5432` (database: `tipchain`, user: `user`, password: `password`)
- **Healthcheck & Startup Order:** The API container automatically waits for PostgreSQL's `pg_isready` health check before booting and running migrations.

---

## API Reference

### Health

```
GET /health
```
Response `200 OK`:
```json
{ "status": "ok" }
```

### Creators

#### Register a creator
```
POST /creators
```
Request body:
```json
{
  "username": "alice",
  "wallet_address": "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN"
}
```
Validation:
- `username`: 3-32 characters, must start with a letter, letters/numbers/underscores only
- `wallet_address`: 56-character Stellar public key starting with `G`

Response `201 Created`:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "alice",
  "wallet_address": "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
  "created_at": "2024-03-14T10:30:00Z"
}
```

#### Get a creator
```
GET /creators/:username
```
Response `200 OK` or `404 Not Found`:
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "username": "alice",
  "wallet_address": "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN",
  "created_at": "2024-03-14T10:30:00Z"
}
```

#### List tips for a creator
```
GET /creators/:username/tips
```
Response `200 OK`:
```json
[
  {
    "id": "660e8400-e29b-41d4-a716-446655440001",
    "creator_username": "alice",
    "amount": "10.5",
    "transaction_hash": "abc123...",
    "created_at": "2024-03-14T11:00:00Z"
  }
]
```

### Tips

#### Record a tip
The transaction is verified on the Stellar network before the tip is saved.
```
POST /tips
```
Request body:
```json
{
  "username": "alice",
  "amount": "10.5",
  "transaction_hash": "abc123def456..."
}
```
Validation:
- `amount`: a positive, finite number
- `transaction_hash`: non-empty hexadecimal string

Response `201 Created`:
```json
{
  "id": "660e8400-e29b-41d4-a716-446655440001",
  "creator_username": "alice",
  "amount": "10.5",
  "transaction_hash": "abc123def456...",
  "created_at": "2024-03-14T11:00:00Z"
}
```

#### Error responses

| Status | Meaning |
|--------|---------|
| `400 Bad Request` | Invalid `amount`/`transaction_hash` format, or invalid creator input |
| `404 Not Found` | Creator does not exist |
| `409 Conflict` | Transaction hash already recorded, or username already taken |
| `422 Unprocessable Entity` | Transaction not found or unsuccessful on the Stellar network |
| `502 Bad Gateway` | Could not reach the Stellar network to verify the transaction |
| `500 Internal Server Error` | Unexpected server-side error |

---

## Database Migrations

Migrations live in the `migrations/` directory and run automatically at startup via SQLx. Each migration ships as a reversible `.up.sql`/`.down.sql` pair.

To manage migrations manually, install the SQLx CLI:

```bash
cargo install sqlx-cli

# Create a new migration
sqlx migrate add -r <migration_name>

# Run pending migrations
sqlx migrate run

# Revert the last migration
sqlx migrate revert
```

### Schema

**creators**
| Column           | Type        | Notes              |
|------------------|-------------|--------------------|
| `id`             | UUID        | Primary key        |
| `username`       | TEXT        | Unique             |
| `wallet_address` | TEXT        |                    |
| `created_at`     | TIMESTAMPTZ | Defaults to now()  |

**tips**
| Column             | Type        | Notes                          |
|--------------------|-------------|--------------------------------|
| `id`               | UUID        | Primary key                    |
| `creator_username` | TEXT        | FK → creators(username), indexed |
| `amount`           | TEXT        |                                |
| `transaction_hash` | TEXT        | Unique — prevents double-spend |
| `created_at`       | TIMESTAMPTZ | Defaults to now()              |

---

## Project Structure

```
src/
├── main.rs                        # Server bootstrap (env, DB pool, router, CORS)
├── validation.rs                  # Input validation (username, wallet, amount, tx hash)
├── controllers/
│   ├── creator_controller.rs      # Creator CRUD handlers
│   └── tip_controller.rs          # Tip handlers
├── db/
│   └── connection.rs              # AppState (DB pool + StellarService)
├── models/
│   ├── creator.rs                 # Creator entity + request DTO
│   └── tip.rs                     # Tip entity + request DTO
├── routes/
│   ├── health.rs                  # /health endpoint
│   ├── creators.rs                # /creators endpoints
│   └── tips.rs                    # /tips endpoints
└── services/
    ├── stellar_service.rs         # Horizon API transaction verification
    └── tip_service.rs             # Tip business logic
migrations/
├── 0001_create_creators.up.sql
├── 0001_create_creators.down.sql
├── 0002_create_tips.up.sql
└── 0002_create_tips.down.sql
```

---

## Current Status

This project is **100% complete** against its original scope. The following is in place:

- [x] Cargo.toml with all dependencies
- [x] Database migrations (creators + tips tables), reversible via `.up.sql`/`.down.sql`
- [x] Data models (`Creator`, `Tip`, request DTOs)
- [x] `AppState` with PostgreSQL pool
- [x] `main.rs` connects to DB and runs migrations
- [x] `StellarService` — Horizon API transaction verification
- [x] `TipService` — tip business logic
- [x] Controllers — request handlers
- [x] Routes — Axum router wiring, organized under `routes/`
- [x] Full server bootstrap (CORS, `axum::serve`)
- [x] Input validation for all write endpoints
- [x] `GET /health` liveness endpoint
- [x] Unit tests (validation) and integration tests (services, controllers) against a real Postgres instance

---

## License

This project is licensed under the MIT License.
