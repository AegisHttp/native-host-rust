# Native Host Rust | TR / [EN](README.md)

![Logo](assets/logo.png)

Bu depo, Aegis Http tarayıcı eklentisi için Rust tabanlı yerel mesajlaşma sunucusunu (native messaging host) içerir. Tarayıcı eklentisi ile yerel GnuPG (`gpg`) komut satırı aracı arasında güvenli bir köprü görevi görerek, özel anahtarları (GnuPG private keys) tarayıcı ortamına hiçbir zaman ifşa etmeden uçtan uca şifreleme, şifre çözme ve mesaj imzalama yeteneklerini sağlar.

## Özellikler

- **Native Messaging Protokolü**: Standart girdi/çıktı (stdin/stdout) kullanarak Google Chrome, Chromium tabanlı ve Firefox tabanlı tarayıcılarla, uzunluk öneki (length-prefixed) JSON veri yükleri üzerinden iletişim kurar.
- **GPG Entegrasyonu**: Kriptografik işlemleri gerçekleştirmek için yerel makinedeki `gpg` yürütülebilir dosyasını çalıştırır.
- **Parçalı (Chunked) Veri İletimi**: Eklentiden gönderilen büyük veri yüklerini (tarayıcı mesajlaşma sınırlarını aşmak için) küçük parçalar (chunks) halinde birleştirir ve büyük cevapları da tarayıcıya yorulmadan parçalar halinde geri iletir.
- **Eşzamanlılık (Concurrency) Kontrolü**: Aynı anda birden fazla GPG işleminin eş zamanlı çalışması sırasında çökme veya sistem yığılmalarını ve yarış durumlarını (race conditions) önlemek için dosya tabanlı bir mutex kilitlemesi uygular (`/tmp/aegis_http_gpg.lock`).

## Desteklenen Eylemler (Actions)

- `list-keys`: İmzalama ve şifre çözme için mevcut GPG gizli (secret) anahtarlarını listeler (alt anahtar şifreleme yeteneklerini denetler).
- `add-subkey`: Güvenli tünelleme/şifreleme desteği için hedef GPG anahtarına yeni bir şifreleme alt anahtarı (encryption subkey) tanımlar.
- `sign`: Yerel kullanıcının GPG anahtarını kullanarak verilen doğrulama metnini imzalar (clear-sign).
- `encrypt`: Alıcının açık anahtarını kullanarak metni şifreler (anahtarı GPG anahtarlığına kalıcı olarak eklemeden doğrudan `public_key` bloğu üzerinden `--recipient-file` ile işlem yapar).
- `decrypt`: GPG ile şifrelenmiş gelen bir şifreli metnin deşifre işlemini yürütür.

## Ön Koşullar

- **Rust / Cargo**: Bu projeyi derlemek için sisteminizde Rust toolchain kurulu olmalıdır.
- **GnuPG (`gpg`)**: Dağıtımınıza uygun şekilde `gpg` aracı komut satırı `PATH` ortam değişkeninde yüklü olmalıdır.

## Derleme

Projeyi optimize edilmiş (`release`) modunda derlemek ve hazırlamak için şu komutu çalıştırın:

```bash
cargo build --release
```

## Kurulum

Host programını Google Chrome ve diğer Chromium tabanlı tarayıcılara tanıtmak ve kaydetmek için:

```bash
chmod +x install.sh
./install.sh
```

Bu script, Native Messaging JSON manifest dosyasını ve programın çalıştırılabilir Rust çıktısını doğru Chrome yapılandırma dosyalarının olduğu konuma kopyalayacaktır.

## Mimari Nasıl Çalışır?

1. Tarayıcı eklentisi, komutu (action) ve hedef veriyi (veya büyük veriler için bir index'li veri parçasını) kapsayan bir JSON bloğu gönderir.
2. Rust daemon'ı, Chrome protokolüne uygun olarak başlangıçtaki 4 baytlık header bilgisini okuyarak ne kadarlık bir string alacağını önceden anlar.
3. Arka planda gelen parçalar (chunks) bütünleşik bir string içerisinde toplanır.
4. Doğru parametrelerle (örneğin `--encrypt`, `--decrypt`) Linux `Command` interface'i ile `gpg` programı ayağa kaldırılır ve standart okuma noktalarından (stdin) veriler pipelar vasıtasıyla iletilir.
5. Sonuç GPG stdout/stderr hatlarından geri okunur ve oluşturulan yeni formatlı bir JSON objesinde güvenli bir şekilde tarayıcı eklentisine iade edilir.
