//! Windows resources for themisio.exe: the icon, version information and a manifest (per-monitor
//! DPI awareness v2, long paths, UTF-8 code page, Windows 10/11 compatibility).

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0" xmlns:asmv3="urn:schemas-microsoft-com:asm.v3">
  <assemblyIdentity type="win32" name="Themisio" version="0.1.0.0"/>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <asmv3:application>
    <asmv3:windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
      <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </asmv3:windowsSettings>
  </asmv3:application>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/themisio.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/themisio.ico")
        .set_manifest(MANIFEST)
        .set("ProductName", "Themisio")
        .set("FileDescription", "Themisio – Make your documents glow.")
        .set("LegalCopyright", "Themisio and the Craft apps it hosts: MIT OR Apache-2.0.")
        .set("OriginalFilename", "themisio.exe")
        .set("InternalName", "themisio");
    if let Err(e) = res.compile() {
        println!("cargo:warning=themisio.exe built without icon/version resources: {e}");
    }
}
