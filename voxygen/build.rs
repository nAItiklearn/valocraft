#[cfg(windows)]
fn main() {
    //Set executable logo with winres here.  The original logo.ico embeds its
    // 256x256 image as PNG; the Windows resource compiler rejects that variant
    // with RC2175, so use a copy that only contains DIB entries.
    let mut res = winres::WindowsResource::new();
    res.set_icon("../assets/voxygen/logo-windows.ico");
    res.compile().expect("failed to build executable logo.");
}

#[cfg(not(windows))]
fn main() {}
