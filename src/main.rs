// If using the `binstart` feature of `esp-idf-sys`, always keep this module
// imported
use esp_idf_sys as _;

fn main() -> anyhow::Result<()> {
    // It is necessary to call this function once. Otherwise, some patches to the
    // runtime implemented by esp-idf-sys might not link properly. See
    // https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_sys::link_patches();

    println!("Hello, world!");

    Ok(())
}
