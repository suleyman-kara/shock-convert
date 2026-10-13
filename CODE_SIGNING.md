# Code signing policy / Kod imzalama politikası

> **Status / Durum:** Shock Convert has applied to the SignPath Foundation open source program.
> Until approval, releases are **not signed**. This page is published as part of the application.
>
> Shock Convert, SignPath Foundation açık kaynak programına başvurmuştur. Onaya kadar sürümler
> **imzasızdır**. Bu sayfa başvurunun bir parçası olarak yayınlanmıştır.

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).

## What is signed / Ne imzalanır

- `shock-convert.exe` and the installer `shock-convert-setup-<version>.exe`
- Only binaries built by this repository's GitHub Actions workflow
  ([`.github/workflows/build.yml`](.github/workflows/build.yml)) from the public source code
  in this repository are submitted for signing. Binaries built elsewhere are never signed.
- Bu repodaki herkese açık kaynak koddan, bu reponun GitHub Actions iş akışıyla üretilen
  dosyalar dışında hiçbir şey imzalanmaz.

## Team roles / Ekip rolleri

| Role | Members |
|---|---|
| Committers and reviewers | [suleyman-kara](https://github.com/suleyman-kara) |
| Approvers | [suleyman-kara](https://github.com/suleyman-kara) |

- Committers and reviewers may change the source code; every change reaches `main`
  through a pull request.
- Approvers approve each signing request before a release is signed.
- All team members use multi-factor authentication on GitHub and SignPath.

## Privacy policy / Gizlilik

This program will not transfer any information to other networked systems unless
specifically requested by the user or the person installing or operating it.

Bu program, kullanıcı ya da onu kuran/çalıştıran kişi özellikle istemedikçe başka ağ
sistemlerine hiçbir bilgi aktarmaz. Dosyalar bilgisayarınızda dönüştürülür; telemetri yoktur.

Optional external tools that the user installs themselves (ffmpeg, LibreOffice, Microsoft Edge
or Chrome for PDF printing) are run locally on the user's files only.
