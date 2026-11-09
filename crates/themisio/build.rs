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

/// Paths under draw.io's `src/main/webapp` the editor doesn't load in Themisio: sources the
/// minified bundles already contain, source maps, the server and online-only pieces.
fn drawio_skip(rel: &str) -> bool {
    const PREFIXES: [&str; 12] = [
        "WEB-INF/",
        "META-INF/",
        "shapes/",
        "js/diagramly/",
        "js/grapheditor/",
        "mxgraph/src/",
        "connect/",
        "workbox-",
        "service-worker.js",
        "teams.html",
        "onedrive3.html",
        "monday-app-association.json",
    ];
    const FILES: [&str; 4] = ["mxgraph/mxClient.js", "js/integrate.min.js", "js/viewer-static.min.js", "js/embed.dev.js"];
    PREFIXES.iter().any(|p| rel.starts_with(p))
        || FILES.contains(&rel)
        || rel.ends_with(".map")
        || (rel.starts_with("stencils/") && rel.ends_with(".xml"))
}

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// draw.io's web app (upstream/drawio, see upstream.lock) as one zip that themisio.exe carries
/// and serves to its WebView2. Without upstream/drawio the zip is empty and the draw.io page
/// says the editor isn't part of this build.
fn bundle_drawio() {
    use std::io::Write;
    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let webapp = manifest.join("../../upstream/drawio/src/main/webapp");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("drawio.zip");
    println!("cargo:rerun-if-changed={}", webapp.display());
    println!("cargo:rerun-if-changed={}", webapp.join("js").display());
    let Ok(file) = std::fs::File::create(&out) else { return };
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let mut files = Vec::new();
    walk(&webapp, &mut files);
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(6));
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut count = 0usize;
    for path in files {
        let Ok(rel) = path.strip_prefix(&webapp) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if drawio_skip(&rel) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let packed = matches!(path.extension().and_then(|e| e.to_str()), Some("png" | "jpg" | "jpeg" | "gif" | "woff" | "woff2" | "zip"));
        if zip.start_file(rel, if packed { stored } else { deflate }).is_ok() && zip.write_all(&bytes).is_ok() {
            count += 1;
        }
    }
    let _ = zip.finish();
    if count == 0 {
        println!("cargo:warning=draw.io not bundled: upstream/drawio is missing (scripts/fetch-upstream)");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/themisio.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    bundle_drawio();
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
