# Panduan Instalasi electrs untuk Nusacoin Testnet di VPS (Versi Binary)

Panduan ini menjelaskan cara menjalankan electrs untuk jaringan **testnet**
Nusacoin (`--network nusacoin-testnet`) di VPS, berdampingan dengan instance
mainnet yang sudah ada. Port testnet berbeda dari mainnet sehingga tidak
bentrok.

Port yang dipakai testnet:

| Layanan      | Mainnet | Testnet |
|--------------|---------|---------|
| Daemon RPC   | 28832   | 18332   |
| Electrum RPC | 50009   | 60009   |
| REST API     | 3010    | 3011    |
| Monitoring   | 4225    | 14225   |

## 1. Siapkan `nusacoind` testnet dan tunggu sync

Daemon testnet harus berjalan dengan flag `-testnet` dan selesai sync
sebelum electrs dijalankan. Verifikasi:

```bash
nusacoin-cli -testnet getblockchaininfo | grep -E '"chain"|"blocks"|"headers"'
```

Pastikan `"chain": "test"` dan `blocks` == `headers`. Nama folder
(`testnet3`) tidak menentukan jaringan — yang menentukan adalah flag
`-testnet`.

## 2. Download binary electrs

Ambil binary resmi dari artifact CI (build `checkbuild` di branch
`new-index`):

```bash
curl -L -o /tmp/electrs <URL_ARTIFACT_CI>
```

Verifikasi checksum (wajib):

```bash
echo "<SHA256_YANG_DIPUBLIKASIKAN>  /tmp/electrs" | sha256sum -c
# harus keluar: /tmp/electrs: OK
```

Install:

```bash
sudo install -m 755 /tmp/electrs /usr/local/bin/electrs
/usr/local/bin/electrs --version
# harus menampilkan: Electrum Rust Server 0.4.1
```

## 3. Buat user dan direktori database

```bash
sudo useradd -r -s /usr/sbin/nologin electrs
sudo mkdir -p /var/lib/electrs-nusacoin-testnet
sudo chown electrs:electrs /var/lib/electrs-nusacoin-testnet
```

## 4. Beri user `electrs` akses baca ke block daemon

electrs membaca file `blk*.dat` langsung dari disk (jauh lebih cepat
daripada via RPC). Daemon menyimpan block testnet di
`<datadir>/testnet3/blocks`.

```bash
sudo apt install -y acl
# hak traverse di setiap folder parent (tanpa ini, electrs tidak bisa
# mencapai folder blocks walau folder blocks-nya sendiri sudah dibuka):
sudo setfacl -m u:electrs:--x /root /root/.nusacoin /root/.nusacoin/testnet3
# hak baca di folder blocks dan isinya:
sudo setfacl -R -m u:electrs:rX /root/.nusacoin/testnet3/blocks
```

Verifikasi:

```bash
sudo -u electrs ls /root/.nusacoin/testnet3/blocks/ | head -3
# harus menampilkan daftar file blk*.dat, bukan "Permission denied"
```

## 5. Pasang sebagai service systemd

`/etc/systemd/system/electrs-nusacoin-testnet.service`:

```ini
[Unit]
Description=electrs Electrum server untuk Nusacoin testnet
After=network.target

[Service]
User=electrs
# PENTING: --daemon-dir diisi dengan BASE datadir (tanpa "testnet3")!
# electrs otomatis menambahkan subfolder "testnet3" untuk network
# nusacoin-testnet. Jika diisi "/root/.nusacoin/testnet3", electrs akan
# mencari block di "/root/.nusacoin/testnet3/testnet3/blocks" yang tidak ada.
ExecStart=/usr/local/bin/electrs \
  --network nusacoin-testnet \
  --daemon-dir /root/.nusacoin \
  --daemon-rpc-addr 127.0.0.1:18332 \
  --cookie "ISI_RPCUSER:ISI_RPCPASSWORD" \
  --db-dir /var/lib/electrs-nusacoin-testnet \
  --electrum-rpc-addr 127.0.0.1:60009 \
  --http-addr 127.0.0.1:3011
Restart=on-failure
RestartSec=30
LimitNOFILE=100000

[Install]
WantedBy=multi-user.target
```

Isi `--cookie` dengan `rpcuser:rpcpassword` dari `nusacoin.conf`
(sebelum flag `-testnet` di config, kredensial RPC dibaca dari bagian
umum config).

Aktifkan:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now electrs-nusacoin-testnet.service
```

## 6. Pantau indexing awal

```bash
sudo journalctl -u electrs-nusacoin-testnet.service -f
# keluar dengan tombol 'q'
```

Log yang sehat:

```
INFO initial sync complete
INFO Electrum RPC server running on 127.0.0.1:60009
INFO REST server running on 127.0.0.1:3011
```

Testnet jauh lebih pendek dari mainnet sehingga indexing selesai cepat.

## 7. Verifikasi

```bash
# Electrum RPC:
echo '{"id":1,"jsonrpc":"2.0","method":"server.version","params":[]}' \
  | nc 127.0.0.1 60009
# harus balik JSON: {"id":1,"jsonrpc":"2.0","result":["electrs-esplora 0.4.1","1.4"]}

# REST API:
curl -s http://127.0.0.1:3011/blocks/tip/height
# harus balik tinggi block testnet
```

## 8. Troubleshooting

### `chown: invalid user: 'electrs:electrs'`

User `electrs` belum dibuat. Jalankan dulu:

```bash
sudo useradd -r -s /usr/sbin/nologin electrs
```

### `setfacl: command not found`

```bash
sudo apt install -y acl
```

### `Failed at step EXEC spawning /usr/local/bin/electrs: No such file or directory`

Binary belum terinstall di mesin ini. Ulangi langkah 2 (download +
verifikasi checksum + install).

### `failed to read daemon xor.dat file`

User `electrs` tidak bisa membaca folder blocks. Biasanya bukan folder
blocks-nya yang bermasalah, melainkan folder **parent**-nya (`/root`
umumnya mode 700). Beri hak traverse di tiap parent (lihat langkah 4).

### Indexing gagal total: `failed to index N blocks from blk*.dat files`

Kemungkinan penyebab, urut dari yang paling sering:

1. **`--daemon-dir` salah (paling sering di testnet!)** — electrs otomatis
   menambahkan subfolder network (`testnet3` untuk nusacoin-testnet) ke
   `--daemon-dir`. Jika `--daemon-dir` sudah berisi `testnet3`, electrs
   mencari block di `<dir>/testnet3/testnet3/blocks` yang tidak ada, sehingga
   0 file blk dibaca. Gejala khas: tidak ada baris `block file reading` di
   log. Solusi: isi `--daemon-dir` dengan base datadir saja
   (mis. `/root/.nusacoin`), bukan `/root/.nusacoin/testnet3`.
2. **File blk berisi chain lama** (hasil fork yang sudah ditinggalkan),
   sementara daemon melayani chain yang baru. Gejalanya: jumlah block di
   file masuk akal, magic benar, tapi 0 yang cocok. Solusi bersih: stop
   daemon testnet, hapus data basi, sync ulang (lihat di bawah). **Catatan:**
   setelah wipe, pasang ulang ACL (lihat langkah 4) karena ACL ikut
   terhapus!

```bash
nusacoin-cli -testnet stop
rm -rf /root/.nusacoin/testnet3/blocks /root/.nusacoin/testnet3/chainstate
nusacoind -daemon -testnet
# tunggu sync selesai, baru start electrs testnet
```

### File `xor.dat` nyasar merusak parsing

electrs versi baru mendukung file `xor.dat` (Bitcoin Core v28+ meng-XOR
file blk). Jika file `xor.dat` ada di folder blocks tetapi file blk
**tidak** di-XOR (magic terbaca plaintext via `xxd`), electrs akan
merusak datanya sendiri saat parsing.

Cek:

```bash
xxd -l 4 /root/.nusacoin/testnet3/blocks/blk00000.dat
# kalau keluar "0b11 0907" (plaintext), file TIDAK di-XOR
```

Jika plaintext tetapi `xor.dat` ada, pindahkan file tersebut:

```bash
mv /root/.nusacoin/testnet3/blocks/xor.dat /root/.nusacoin/testnet3/blocks/xor.dat.bak
sudo systemctl restart electrs-nusacoin-testnet.service
```

### Memastikan daemon yang dihubungi benar-benar testnet

Jika di satu mesin ada dua daemon (mainnet + testnet), pastikan electrs
testnet menembak port RPC yang benar (18332) dan daemon di port tersebut
memang testnet:

```bash
nusacoin-cli -rpcport=18332 -rpcuser=XXX -rpcpassword=XXX getblockchaininfo \
  | grep '"chain"'
# harus: "chain": "test"
```

### Catatan khusus testnet Nusacoin

- Testnet Nusacoin memakai **genesis block yang sama** dengan mainnet
  (tidak umum; Bitcoin memakai genesis berbeda). Jadi `getblockhash 0`
  yang sama di kedua jaringan adalah normal.
- Block hash testnet juga memakai **X11**, sama seperti mainnet.
- Testnet wajar memiliki banyak fork (miner independen, koin tidak
  bernilai). Jika electrs gagal index setelah reorg besar, wipe dan
  sync ulang adalah solusi paling bersih.
