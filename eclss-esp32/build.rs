use anyhow::Context;
fn main() -> anyhow::Result<()> {
    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")?;

    let assets_dir = {
        let mut dir = std::env::current_dir()?;
        dir.push("..");
        dir.push("eclss-web");
        dir.push("dist");
        dir
    };

    edge_frame::assets::prepare::run("ECLSS_WEB", assets_dir)
        .context("failed to prepare edge-frame assets")?;

    Ok(())
}
