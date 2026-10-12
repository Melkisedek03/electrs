# Running electrs for Nusacoin (NUX)

This guide covers deploying this electrs fork as a public Electrum server for
the Nusacoin mainnet — the prerequisite for a [Komodo Wallet (AtomicDEX)]
listing, which requires reachable Electrum servers plus a proven test swap.

[Komodo Wallet (AtomicDEX)]: https://github.com/KomodoPlatform/coins

## 1. Requirements

- A synced `nusacoind` with the **full block data** (pruning disabled;
  `txindex` is *not* required — electrs builds its own index).
- This electrs fork, built with a Rust toolchain (`cargo build --release`).
- A Linux server. For the Nusacoin chain (~73k blocks as of late 2026) a small
  VPS is plenty: 2 GB RAM, ~10 GB disk for the electrs database.

`nusacoind` must serve RPC locally. Minimal `nusacoin.conf`:

```ini
server=1
daemon=1
disablewallet=1
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcauth=electrs:<salt>$<hash>   # generate with share/rpcauth/rpcauth.py
# optional, faster mempool/block notifications (otherwise electrs polls):
# zmqpubrawblock=tcp://127.0.0.1:28334
# zmqpubrawtx=tcp://127.0.0.1:28335
```

Keep the RPC port off the public internet. electrs connects to
`127.0.0.1:28832` by default with `--network nusacoin`.

## 2. Build

```bash
git clone https://github.com/Melkisedek03/electrs.git
cd electrs
cargo build --release --bin electrs
# binary: ./target/release/electrs
```

## 3. First run (initial sync)

```bash
./target/release/electrs \
  --network nusacoin \
  --daemon-dir ~/.nusacoin \
  --db-dir /var/lib/electrs-nusacoin \
  --electrum-rpc-addr 127.0.0.1:50009
```

On first start electrs fetches the chain's genesis hash from the daemon
(`getblockhash 0`), then indexes all blocks. For a chain this size the initial
sync takes minutes, not hours. Afterwards it follows the tip in real time.

Verify it's working:

```bash
# Electrum protocol banner / features
echo '{"id":1,"method":"server.version","params":[]}' | nc 127.0.0.1 50009
```

## 4. systemd unit

```ini
[Unit]
Description=electrs for Nusacoin
After=network.target nusacoind.service
Wants=nusacoind.service

[Service]
User=electrs
ExecStart=/usr/local/bin/electrs \
  --network nusacoin \
  --daemon-dir /home/nusacoin/.nusacoin \
  --db-dir /var/lib/electrs-nusacoin \
  --electrum-rpc-addr 127.0.0.1:50009
Restart=on-failure
RestartSec=30
LimitNOFILE=100000

[Install]
WantedBy=multi-user.target
```

## 5. Exposing it publicly (for the Komodo listing)

Komodo's listing guide expects Electrum servers reachable as e.g.
`electrum1.example.com:50001` (TCP) and `:50002` (SSL). electrs itself speaks
plain TCP; terminate TLS in front of it with `stunnel`:

```ini
; /etc/stunnel/electrs.conf
[electrs-nux]
accept = 50002
connect = 127.0.0.1:50009
cert = /etc/letsencrypt/live/electrum1.example.com/fullchain.pem
key = /etc/letsencrypt/live/electrum1.example.com/privkey.pem
```

Open TCP `50001`/`50002` in the firewall, keep `28832` (daemon RPC) closed.

For the Komodo `coins` repo PR you then provide:

- `electrums/NUX`: your server host(s) and ports
- `explorers/NUX`: the chain explorer URL
- `coins` entry + `icons/nux.png` (82x82)
- `swaps/NUX`: explorer links to the four atomic-swap transactions
  (`makerpayment`, `takerpayment`, `takerpaymentspent`, `makerpaymentspent`)
  from a successful test swap via the Komodo DeFi Framework API

Run at least **two** independent Electrum servers for redundancy before
submitting the listing PR.

## 6. Testnet

For integration testing (wallets, swaps) without touching real funds, run
against the Nusacoin testnet instead — it's just a flag switch:

```bash
./target/release/electrs \
  --network nusacoin-testnet \
  --daemon-dir ~/.nusacoin \
  --db-dir /var/lib/electrs-nusacoin-testnet \
  --electrum-rpc-addr 127.0.0.1:60009
```

Testnet defaults: daemon RPC `127.0.0.1:18332`, P2P magic `0x0b110907`,
address encoding P2PKH version 111 / P2SH version 196 / bech32 HRP `"tn"`,
DB subdirectory `testnet3`. The genesis hash is fetched from the daemon, so
any testnet chain works. Testnet difficulty is tiny — one
`generatetoaddress` on a testnet node produces blocks instantly, which makes
it ideal for end-to-end wallet testing.

## 7. Operational notes

- The Nusacoin chain is sparsely mined (~98 min/block in 2026). Electrum
  clients will see slow confirmations — that is chain behavior, not an
  indexer problem. Size any swap timelocks accordingly.
- Reindex from scratch by stopping electrs and deleting the `db-dir`
  contents; it will re-walk from the daemon's genesis on next start.
- Monitor via the Prometheus endpoint (`--monitoring-addr`, default
  `127.0.0.1:4225` for `--network nusacoin`).
