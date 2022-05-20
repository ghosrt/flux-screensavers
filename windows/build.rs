#[cfg(windows)]
extern crate winres;
#[cfg(windows)]
use std::error::Error;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn Error>> {
    let mut resource = winres::WindowsResource::new();
    // This doesn’t work right now because NixOS doesn’t have windres. Or I
    // can’t find it...
    // resource.set_icon("flux.ico"); // TODO: add an icon
    resource.set_manifest_file("flux-windows-screensaver.exe.manifest");
    resource.compile()?;

    Ok(())
}

#[cfg(unix)]
fn main() {}
