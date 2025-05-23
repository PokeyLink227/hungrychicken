use self_update::cargo_crate_version;

pub fn update() -> Result<(), Box<dyn std::error::Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("pokeylink227")
        .repo_name("hungrychicken")
        .bin_name("hungrychicken")
        .show_download_progress(false)
        .no_confirm(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    if status.updated() {
        std::process::exit(0);
    }
    Ok(())
}
