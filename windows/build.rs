use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(windows)]
    {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("flux.ico");
        resource.set_manifest_file("flux-windows-screensaver.exe.manifest");
        resource.compile().unwrap();
    }

    Ok(())
}
