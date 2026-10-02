//! Embeds the icon in `encrust.exe`, where Explorer, the taskbar and the Start menu read it.

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=../../assets/icon/encrust.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("../../assets/icon/encrust.ico")
            .compile()?;
    }
    Ok(())
}
