# Panduan Instalasi electrs untuk Nusacoin di VPS (Versi Binary)

Panduan langkah demi langkah memasang **electrs** (Electrum server) untuk
blockchain **Nusacoin (NUX)** di VPS Ubuntu — **tanpa compile**, langsung
pakai binary resmi. electrs adalah backend untuk wallet Electrum-protocol
(seperti GreenWallet) dan prasyarat listing DEX atomic-swap seperti Komodo
Wallet.

> Estimasi waktu: ±20 menit (di luar sync `nusacoind`).
> Panduan ini sudah dipraktikkan langsung (proven) di VPS produksi.

## 1. Spesifikasi VPS

| Komponen | Minimal |
|----------|---------|
| OS       | Ubuntu 22.04 / 24.04 LTS |
| CPU      | 2 vCPU |
| RAM      | 2 GB |
| Disk     | 30 GB SSD |

Chain Nusacoin masih kecil (±73 ribu block, 2026): electrs hanya memakai
±60 MB RAM saat idle dan index ±137 MB di disk. Nyaman di VPS $5–10/bulan.
Yang penting VPS-nya **tetap online**.

## 2. Siapkan `nusacoind` dan tunggu sync

electrs membutuhkan `nusacoind` yang sudah sync penuh (`verificationprogress:
1.0`, *pruning* mati). `txindex` **tidak** wajib — electrs membangun index
sendiri.

```bash
sudo useradd -m -s /bin/bash nusacoin
```

Isi `/home/nusacoin/.nusacoin/nusacoin.conf`:

```ini
server=1
daemon=1
disablewallet=1
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
rpcuser=nusacoinrpc
rpcpassword=GANTI_DENGAN_PASSWORD_ACAK_YANG_KUAT
```

Jalankan dan tunggu sync selesai:

```bash
sudo -u nusacoin nusacoind -datadir=/home/nusacoin/.nusacoin -daemon
sudo -u nusacoin nusacoin-cli -datadir=/home/nusacoin/.nusacoin getblockchaininfo
# "verificationprogress" harus 1.0 sebelum lanjut
```

> **Penting:** port RPC jangan dibuka ke internet. Biarkan hanya `127.0.0.1`.

## 3. Download binary electrs

Download artifact `electrs-nusacoin-linux-x86_64` dari
[halaman GitHub Actions](https://github.com/Melkisedek03/electrs/actions)
(pilih run terbaru yang hijau). Lalu verifikasi checksum-nya — **jangan
lewatkan langkah ini**:

```bash
cd /tmp
# (sesuaikan dengan file yang di-download)
sha256sum electrs-nusacoin-linux-x86_64
# cocokkan dengan checksum yang dipublikasikan di halaman rilis/CI
```

Pasang binary-nya:

```bash
sudo cp electrs-nusacoin-linux-x86_64 /usr/local/bin/electrs
sudo chmod +x /usr/local/bin/electrs
/usr/local/bin/electrs --version
# harus menampilkan: Electrum Rust Server 0.4.1
```

## 4. Buat user dan direktori database

```bash
sudo useradd -r -s /usr/sbin/nologin electrs
sudo mkdir -p /var/lib/electrs-nusacoin
sudo chown electrs:electrs /var/lib/electrs-nusacoin
```

## 5. Beri user `electrs` akses baca ke block daemon

electrs membaca file block mentah (`blk*.dat`) langsung dari datadir daemon.
Karena service berjalan sebagai user `electrs` (bukan root), beri izin baca
via ACL:

```bash
sudo apt-get update && sudo apt-get install -y acl
sudo setfacl -m u:electrs:--x /home/nusacoin /home/nusacoin/.nusacoin
sudo setfacl -Rm u:electrs:rX /home/nusacoin/.nusacoin/blocks
sudo setfacl -Rdm u:electrs:rX /home/nusacoin/.nusacoin/blocks
```

Verifikasi:

```bash
sudo -u electrs ls /home/nusacoin/.nusacoin/blocks/blk*.dat | head -3
# harus menampilkan daftar file blk*.dat, bukan "Permission denied"
```

> Baris `setfacl -Rdm` (default ACL) memastikan file `blk*.dat` **baru**
> yang dibuat daemon nanti otomatis bisa dibaca.

## 6. Pasang sebagai service systemd

`/etc/systemd/system/electrs-nusacoin.service`:

```ini
[Unit]
Description=electrs Electrum server untuk Nusacoin
After=network.target

[Service]
User=electrs
ExecStart=/usr/local/bin/electrs \
  --network nusacoin \
  --daemon-dir /home/nusacoin/.nusacoin \
  --daemon-rpc-addr 127.0.0.1:28832 \
  --cookie "nusacoinrpc:GANTI_DENGAN_PASSWORD_ACAK_YANG_KUAT" \
  --db-dir /var/lib/electrs-nusacoin \
  --electrum-rpc-addr 127.0.0.1:50009 \
  --http-addr 127.0.0.1:3010
Restart=on-failure
RestartSec=30
LimitNOFILE=100000

[Install]
WantedBy=multi-user.target
```

Catatan parameter penting:

- `--daemon-dir` **wajib** — tanpa ini electrs mencari block di direktori
  yang salah dan index gagal total.
- `--cookie` diisi `rpcuser:rpcpassword` sesuai `nusacoin.conf`.
- `--daemon-rpc-addr` sesuaikan dengan port RPC daemon (default Nusacoin:
  `28832`; kalau `nusacoin.conf` memakai port lain, samakan).
- `--http-addr` adalah REST API (kompatibel Esplora). Biarkan `127.0.0.1`
  kecuali tahu yang dilakukan (lihat bagian 9).

Aktifkan:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now electrs-nusacoin.service
```

## 7. Pantau indexing awal

```bash
sudo journalctl -u electrs-nusacoin.service -f
# keluar dengan tombol 'q'
```

Log yang sehat:

```
INFO downloading all block headers up to ...
INFO hash=... height=73637 ... (73638 left to index)
INFO initial sync complete, tip at ...
INFO Electrum RPC server running on 127.0.0.1:50009
INFO REST server running on 127.0.0.1:3010
```

Indexing 73 ribu block hanya butuh hitungan menit di chain sekecil Nusacoin.
Tunggu sampai muncul `initial sync complete`.

## 8. Verifikasi

**Electrum RPC** (port `50009`):

```bash
python3 -c "
import socket
s = socket.create_connection(('127.0.0.1', 50009), timeout=10)
s.sendall(b'{\"id\":1,\"method\":\"server.version\",\"params\":[\"smoketest\",\"1.4\"]}\n')
print(s.recv(4096).decode())
"
# harus balik JSON: {"id":1,"jsonrpc":"2.0","result":["electrs-esplora 0.4.1","1.4"]}
```

**REST API** (port `3010`, kompatibel Esplora):

```bash
curl -s http://127.0.0.1:3010/blocks/tip/height
# harus balik tinggi block, mis. 73637
```

Endpoint berguna lainnya:

| Endpoint | Kegunaan |
|----------|----------|
| `GET /blocks/tip/hash` | hash block terakhir |
| `GET /block/:hash` | detail block |
| `GET /address/:address/utxo` | daftar UTXO sebuah address |
| `GET /address/:address/txs` | riwayat transaksi address |
| `GET /tx/:txid` | detail transaksi |
| `GET /mempool/txids` | transaksi di mempool |
| `POST /tx` | broadcast raw transaction (body = hex) |

Untuk akses REST dari komputer sendiri (mis. via Bruno/Postman), buat SSH
tunnel — jangan buka port ke publik tanpa firewall:

```bash
ssh -f -N -L 3010:127.0.0.1:3010 <user>@<ip-vps>
# lalu akses http://localhost:3010 dari komputer sendiri
```

## 9. Troubleshooting

| Gejala di log | Penyebab & solusi |
|---------------|-------------------|
| `bitcoind returned non-contiguous headers` | Binary lama (sebelum dukungan X11). Download ulang binary terbaru. |
| `failed to index N blocks from blk*.dat files` | `--daemon-dir` salah atau tidak diset — electrs tidak menemukan file block. Perbaiki di service file. |
| `failed to read daemon xor.dat file: Permission denied` | User `electrs` tidak bisa baca datadir daemon. Ulangi langkah 5 (ACL). |
| Service restart terus-menerus | Lihat `journalctl -u electrs-nusacoin.service -n 50 --no-pager` dan cocokkan dengan tabel ini. |

**Index ulang dari nol:** hentikan service, hapus isi `--db-dir`, jalankan
lagi — electrs akan index ulang dari genesis.

## 10. Buka akses publik (untuk listing Komodo)

Wallet/client DEX butuh server Electrum yang bisa diakses publik. electrs
hanya bicara TCP polos — untuk TLS pasang `stunnel`:

```bash
sudo apt-get install -y stunnel4 certbot
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
sudo ufw allow 50002/tcp
# JANGAN buka port RPC daemon ke publik!
```

Untuk pengajuan listing ke Komodo (`KomodoPlatform/coins`), sediakan minimal
**dua** server Electrum independen (redundansi), lalu siapkan:

- `electrums/NUX` — daftar server + port
- `explorers/NUX` — URL block explorer
- `coins` — entri coin + `icons/nux.png` (82x82)
- bukti atomic-swap via Komodo DeFi Framework API

## 11. Catatan operasional

- **Chain Nusacoin jarang ada block** (±98 menit/block di 2026). Client akan
  melihat konfirmasi lambat — itu perilaku chain, bukan masalah indexer.
  Atur timelock swap secara konservatif.
- **Update binary:** download binary baru, verifikasi checksum, timpa
  `/usr/local/bin/electrs`, lalu `sudo systemctl restart electrs-nusacoin`.
- **Monitoring:** endpoint Prometheus di `--monitoring-addr`
  (default `127.0.0.1:4225` untuk `--network nusacoin`).
- **Untuk developer** yang ingin compile dari source:
  `cargo build --locked --release --bin electrs` (butuh Rust toolchain).

## Bantuan

Butuh bantuan? Buka *issue* di
[Melkisedek03/electrs](https://github.com/Melkisedek03/electrs) atau tanya di
komunitas Nusacoin.
