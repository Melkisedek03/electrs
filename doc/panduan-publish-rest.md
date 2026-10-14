# Panduan Publikasi REST API electrs untuk Komunitas

Panduan membuka REST API electrs-Nusacoin ke publik (via Cloudflare +
domain sendiri) agar komunitas bisa mengakses data chain tanpa
menjalankan node sendiri.

> electrs (Rust) **sudah menyertakan HTTP server di dalam binary-nya**
> — tidak butuh server terpisah seperti gunicorn/uvicorn pada Python.
> Yang perlu dilakukan hanya mengubah bind address dan memasang
> Cloudflare di depannya.

> **Catatan:** seluruh panduan memakai `SUBDOMAIN` sebagai contoh.
> Ganti dengan subdomain-mu di semua langkah (file sertifikat, konfigurasi
> nginx, dan DNS record).

## 1. Pastikan REST listen di localhost (tidak perlu diubah)

Biarkan `--http-addr 127.0.0.1:3010` — nginx yang akan meneruskannya.
(Tidak perlu ubah ke `0.0.0.0`.)

## 2. Buat sertifikat Origin CA di Cloudflare

Mode Full (strict) mewajibkan origin (nginx) punya sertifikat TLS yang
valid. Paling praktis: **Cloudflare Origin CA** — gratis, berlaku 15
tahun, dan otomatis dipercaya Cloudflare (tidak perlu renewal tiap
90 hari seperti Let's Encrypt):

1. Buka [dash.cloudflare.com](https://dash.cloudflare.com) → pilih domainmu.
2. Menu kiri: **SSL/TLS** → **Origin Server** → **Create Certificate**.
3. Pilih:
   - *Generate private key and CSR with Cloudflare*: **RSA**
   - *Certificate Validity*: **15 years**
   - Hostnames: `SUBDOMAIN` (ganti dengan subdomainmu)
4. Klik **Create** → simpan isi **Origin Certificate** dan **Private Key**
   ke VPS:
   ```bash
   sudo mkdir -p /etc/nginx/cf-origin
   # tempel sertifikat:
   sudo nano /etc/nginx/cf-origin/SUBDOMAIN.pem
   # tempel private key:
   sudo nano /etc/nginx/cf-origin/SUBDOMAIN.key
   sudo chmod 600 /etc/nginx/cf-origin/SUBDOMAIN.key
   ```

   > Nama file bebas — boleh `cert.pem` atau apa pun. Yang penting:
   > (1) isi sertifikatnya yang benar (dibuat untuk subdomainmu),
   > (2) path-nya sama dengan yang ditulis di `ssl_certificate` /
   > `ssl_certificate_key` pada konfigurasi nginx di bawah.

## 3. Pasang nginx sebagai reverse proxy TLS

Cloudflare hanya bisa meneruskan ke port HTTPS standar (443, 2053, 2083,
dst) — **tidak bisa ke port 3010 langsung**. Jadi nginx listen di 443
dengan sertifikat Origin CA, teruskan ke electrs:

Buat file `/etc/nginx/sites-available/electrs-rest` dan isi dengan konfigurasi berikut:

```bash
sudo nano /etc/nginx/sites-available/electrs-rest
```

Tempel konfigurasi berikut ke dalam file tersebut, lalu simpan:

```nginx
server {
    listen 443 ssl;
    server_name SUBDOMAIN;  # ganti dengan domainmu

    ssl_certificate /etc/nginx/cf-origin/SUBDOMAIN.pem;
    ssl_certificate_key /etc/nginx/cf-origin/SUBDOMAIN.key;

    location / {
        proxy_pass http://127.0.0.1:3010;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

Setelah file tersimpan, aktifkan site dan reload nginx (perintah terminal, bukan bagian dari konfigurasi di atas):

```bash
sudo ln -s /etc/nginx/sites-available/electrs-rest /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
curl -sk -H "Host: SUBDOMAIN" https://127.0.0.1/blocks/tip/height  # tes lokal via nginx
# PENTING: header Host wajib diisi nama subdomainmu — tanpa ini nginx
# memakai vhost default (bukan blok electrs) dan mengembalikan error
# "Cannot GET /..." dari aplikasi lain.
```

## 4. Setting Cloudflare (mode Full strict)

1. Menu kiri: **DNS** → **Records** → **Add record**:
   - Type: `A`, Name: label pertama subdomainmu (mis. `api`), IPv4 address: IP VPS,
     Proxy status: **Proxied** (awan oranye 🟠) → **Save**.
2. Menu kiri: **SSL/TLS** → **Overview**.
3. Pada *"Your SSL/TLS encryption mode"*, klik tombol **Configure**,
   lalu pilih **Full (strict)**:
   - Pengunjung ↔ Cloudflare: HTTPS (sertifikat otomatis dari Cloudflare).
   - Cloudflare ↔ VPS: HTTPS dengan sertifikat Origin CA yang divalidasi.
   - Enkripsi ujung-ke-ujung, cocok untuk layanan publik jangka panjang.
4. Tunggu beberapa menit sampai sertifikat edge aktif.

## 5. Firewall: hanya Cloudflare yang boleh akses port 443 (opsional, disarankan)

Langkah ini **opsional** — API-nya sendiri memang publik dan read-only,
jadi tidak ada data rahasia yang bocor kalau seseorang menembak IP
langsung. Tujuannya adalah **melindungi origin**: tanpa ini, penyerang
bisa mem-bypass proteksi DDoS, rate limiting, dan caching Cloudflare
dengan menyerang IP VPS langsung.

Tutup port 443 untuk umum, buka hanya untuk IP Cloudflare
(daftar lengkap: https://www.cloudflare.com/ips/):

```bash
sudo apt install -y ufw

# PENTING: izinkan SSH dulu sebelum mengaktifkan firewall,
# kalau tidak akses SSH ke VPS akan terputus!
sudo ufw allow 22/tcp

# Izinkan IP Cloudflare ke port 443 (tambahkan DULU agar dicek duluan):
for ip in 173.245.48.0/20 103.21.244.0/22 103.22.200.0/22 103.31.4.0/22 \
  141.101.64.0/18 108.162.192.0/18 190.93.240.0/20 188.114.96.0/20 \
  197.234.240.0/22 198.41.128.0/17 162.158.0.0/15 104.16.0.0/13 \
  104.24.0.0/14 172.64.0.0/13 131.0.72.0/22; do
  sudo ufw allow from $ip to any port 443
done
# Baru tolak sisanya (aturan ini dicek setelah allow di atas):
sudo ufw deny 443/tcp

sudo ufw enable
```

> ⚠️ Urutan penting: di ufw, aturan yang ditambahkan duluan dicek duluan.
> Kalau `deny` dipasang sebelum `allow from`, semua koneksi (termasuk
> dari Cloudflare) ikut ditolak.
>
> ⚠️ Kalau VPS ini juga melayani situs lain yang **tidak** lewat
> Cloudflare, jangan jalankan `deny 443/tcp` — situs-situs itu ikut mati.

> Port 3010 tidak perlu dibuka sama sekali — hanya nginx (lokal) yang
> menghubunginya.

## 6. Tes

```bash
curl -s https://SUBDOMAIN/blocks/tip/height
# harus mengembalikan tinggi block, mis. 73637

curl -s https://SUBDOMAIN/blocks/tip/hash
# harus mengembalikan hash tip
```

Buka juga di browser — endpoint mengembalikan JSON.

### Troubleshooting: respons bukan dari electrs

Kalau responsnya HTML `Cannot GET /...` atau JSON `{"error":{"code":404,...}}`
dari aplikasi lain (bukan electrs), berarti nginx meneruskan request ke
backend yang salah. Cek:

```bash
# 1. pastikan electrs sendiri sehat (bypass nginx):
curl -s http://127.0.0.1:3010/blocks/tip/height
# harus mengembalikan angka

# 2. lihat semua server_name yang aktif:
sudo nginx -T 2>/dev/null | grep -B2 -A6 "server_name"
```

Pastikan: symlink `sites-enabled/electrs-rest` ada, `server_name`-nya persis
sama dengan subdomain yang dipakai, dan tidak ada block lain
(`default_server`/wildcard) yang menangkap duluan.

## 7. Catatan keamanan & operasional

- **REST electrs read-only** — tidak ada private key atau operasi sensitif
  di sini, jadi aman dipublikasikan.
- **Jangan buka port RPC daemon** (`28832`) atau Electrum TCP (`50009`)
  dengan cara ini — panduan ini khusus REST API.
- **Rate limiting:** kalau traffic naik, aktifkan *Rate Limiting Rules*
  di Cloudflare untuk melindungi VPS.
- **Caching:** Cloudflare otomatis me-cache respons dengan header
  cache yang sesuai; endpoint seperti `/block/:hash` (data historis)
  aman di-cache lama.
- **Monitoring:** pastikan service tetap jalan —
  `systemctl is-active electrs-nusacoin.service`.

## Endpoint populer untuk komunitas

| Endpoint | Kegunaan |
|----------|----------|
| `GET /blocks/tip/height` | tinggi block terakhir |
| `GET /blocks/tip/hash` | hash block terakhir |
| `GET /block/:hash` | detail block |
| `GET /address/:address/utxo` | UTXO sebuah address |
| `GET /address/:address/txs` | riwayat transaksi address |
| `GET /tx/:txid` | detail transaksi |
| `GET /mempool/txids` | transaksi di mempool |
