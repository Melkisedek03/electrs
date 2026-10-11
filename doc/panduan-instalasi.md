# Panduan Instalasi electrs untuk Nusacoin di VPS

Panduan langkah demi langkah memasang **electrs** (Electrum server) untuk
blockchain **Nusacoin (NUX)** di VPS Ubuntu. Electrum server ini adalah
prasyarat agar NUX bisa listing di DEX atomic-swap seperti Komodo Wallet.

> Estimasi waktu: ±30 menit (di luar sync `nusacoind` dan compile awal).

## 1. Siapkan VPS

Spesifikasi minimal (chain Nusacoin masih kecil, ±73 ribu block):

| Komponen | Minimal |
|----------|---------|
| OS       | Ubuntu 22.04 / 24.04 LTS |
| CPU      | 2 vCPU |
| RAM      | 2 GB |
| Disk     | 30 GB SSD |

> Catatan: `nusacoind` + `electrs` untuk chain sekecil Nusacoin muat di VPS
> $5–10/bulan. Yang penting bukan speknya, tapi VPS-nya **tetap online**.

## 2. Install `nusacoind` dan tunggu sync

electrs membutuhkan `nusacoind` yang sudah sync penuh (semua block tersimpan,
*pruning* mati). `txindex` **tidak** wajib — electrs membangun index sendiri.

```bash
# contoh: jalankan nusacoind sebagai user 'nusacoin'
sudo useradd -m -s /bin/bash nusacoin
```

Isi `~nusacoin/.nusacoin/nusacoin.conf`:

```ini
server=1
daemon=1
disablewallet=1
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcuser=nusacoinrpc
rpcpassword=GANTI_DENGAN_PASSWORD_ACAK_YANG_KUAT
```

Jalankan dan tunggu sampai sync selesai:

```bash
nusacoind -datadir=/home/nusacoin/.nusacoin -daemon
# cek progres:
nusacoin-cli -datadir=/home/nusacoin/.nusacoin getblockchaininfo
# "verificationprogress" harus 1.0 sebelum lanjut
```

> **Penting:** port RPC (28832) jangan dibuka ke internet. Biarkan hanya
> `127.0.0.1`.

## 3. Install peralatan build

```bash
sudo apt-get update
sudo apt-get install -y git curl build-essential libclang-dev clang cmake
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

## 4. Download dan compile electrs

```bash
git clone https://github.com/Melkisedek03/electrs.git
cd electrs
cargo build --release --bin electrs
# binary hasil: ./target/release/electrs
# (compile pertama ±15–30 menit, sesudahnya cepat karena cache)
sudo cp ./target/release/electrs /usr/local/bin/
```

## 5. Jalankan electrs

Buat direktori database dan user khusus:

```bash
sudo useradd -r -s /usr/sbin/nologin electrs
sudo mkdir -p /var/lib/electrs-nusacoin
sudo chown electrs:electrs /var/lib/electrs-nusacoin
```

Tes jalan manual dulu (ganti `rpcuser`/`rpcpassword` sesuai `nusacoin.conf`):

```bash
sudo -u electrs /usr/local/bin/electrs \
  --network nusacoin \
  --daemon-dir /home/nusacoin/.nusacoin \
  --daemon-rpc-addr 127.0.0.1:28832 \
  --db-dir /var/lib/electrs-nusacoin \
  --electrum-rpc-addr 127.0.0.1:50009
```

Saat pertama jalan, electrs mengambil genesis hash dari daemon lalu
meng-index seluruh block (chain sekecil Nusacoin: hitungan menit). Tes:

```bash
echo '{"id":1,"method":"server.version","params":[]}' | nc 127.0.0.1 50009
```

Kalau ada jawaban JSON, index berjalan. Hentikan dengan `Ctrl+C`, lalu
pasang sebagai service agar otomatis jalan.

### Service systemd

`/etc/systemd/system/electrs-nusacoin.service`:

```ini
[Unit]
Description=electrs Electrum server untuk Nusacoin
After=network.target
Wants=nusacoind.service

[Service]
User=electrs
ExecStart=/usr/local/bin/electrs \
  --network nusacoin \
  --daemon-dir /home/nusacoin/.nusacoin \
  --daemon-rpc-addr 127.0.0.1:28832 \
  --db-dir /var/lib/electrs-nusacoin \
  --electrum-rpc-addr 127.0.0.1:50009
Restart=on-failure
RestartSec=30
LimitNOFILE=100000

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now electrs-nusacoin
sudo systemctl status electrs-nusacoin
sudo journalctl -u electrs-nusacoin -f   # lihat log
```

## 6. Buka akses publik (untuk listing Komodo)

Wallet/client DEX butuh server Electrum yang bisa diakses publik, biasanya
port `50001` (TCP) dan `50002` (SSL). electrs sendiri hanya bicara TCP polos,
jadi pasang `stunnel` untuk TLS-nya:

```bash
sudo apt-get install -y stunnel4 certbot
# dapatkan sertifikat (ganti domainnya):
sudo certbot certonly --standalone -d electrum1.contoh.com
```

`/etc/stunnel/electrs-nusacoin.conf`:

```ini
[electrs-nux]
accept = 50002
connect = 127.0.0.1:50009
cert = /etc/letsencrypt/live/electrum1.contoh.com/fullchain.pem
key = /etc/letsencrypt/live/electrum1.contoh.com/privkey.pem
```

Buka port di firewall, **tetap tutup port RPC daemon**:

```bash
sudo ufw allow 50001/tcp
sudo ufw allow 50002/tcp
# JANGAN buka 28832 ke publik!
```

Untuk pengajuan listing ke Komodo (`KomodoPlatform/coins`), sediakan minimal
**dua** server Electrum independen (redundansi), lalu siapkan:

- `electrums/NUX` — daftar server + port
- `explorers/NUX` — URL block explorer
- `coins` — entri coin + `icons/nux.png` (82x82)
- `swaps/NUX` — link explorer ke 4 transaksi atomic-swap hasil uji coba
  (`makerpayment`, `takerpayment`, `takerpaymentspent`, `makerpaymentspent`)
  via Komodo DeFi Framework API

## 7. Catatan operasional

- **Chain Nusacoin jarang ada block** (±98 menit/block di 2026). Client akan
  melihat konfirmasi lambat — itu perilaku chain, bukan masalah indexer.
  Atur timelock swap secara konservatif.
- **Index ulang dari nol:** hentikan service, hapus isi `--db-dir`, jalankan
  lagi — electrs akan index ulang dari genesis daemon.
- **Monitoring:** endpoint Prometheus di `--monitoring-addr`
  (default `127.0.0.1:4225` untuk `--network nusacoin`).
- **Update:** `cd electrs && git pull && cargo build --release --bin electrs`,
  lalu `sudo systemctl restart electrs-nusacoin`.

## Bantuan

Butuh bantuan? Buka *issue* di
[Melkisedek03/electrs](https://github.com/Melkisedek03/electrs) atau tanya di
komunitas Nusacoin.
