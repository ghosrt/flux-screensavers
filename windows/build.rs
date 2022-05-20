#[cfg(windows)]
extern crate winres;
#[cfg(windows)]
use std::error::Error;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn Error>> {
    let mut resource = winres::WindowsResource::new();
    resource.set_icon("flux.ico");
    resource.set_manifest_file("flux-windows-screensaver.exe.manifest");
    resource.compile()?;

    Ok(())
}

#[cfg(unix)]
fn main() {}
