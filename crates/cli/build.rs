//! Windows hedefinde exe'ye ikon ve sürüm bilgisi gömer (sağ tık menüsündeki simge `exe,0`'dır).
//! Gömme başarısız olursa derleme durmaz, uyarı verir; CI ikonun gerçekten gömüldüğünü ayrıca denetler.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icon.ico")
        .set("ProductName", "Shock Convert")
        .set("CompanyName", "Suleyman Kara")
        .set("LegalCopyright", "Copyright (c) Suleyman Kara. Apache-2.0")
        .set("FileDescription", "Shock Convert - right-click converter")
        .set("InternalName", "shock-convert")
        .set("OriginalFilename", "shock-convert.exe");
    if let Err(e) = res.compile() {
        println!("cargo:warning=ikon/surum bilgisi exe'ye gomulemedi: {e}");
    }
}
