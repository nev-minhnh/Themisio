//! Themisio's own interface strings in English and Vietnamese, and the language setting that the
//! shell hands on to the four apps (WordCraft and PdfCraft through craft-i18n, GridCraft through
//! its built-in catalog, DeckCraft through `deck_i18n`).

use serde::{Deserialize, Serialize};

/// The language the interface is shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    En,
    Vi,
}

impl Lang {
    /// BCP 47 code, as the apps' language settings take it.
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
        }
    }
}

/// The language setting: follow Windows, or one of the two.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LangPref {
    #[default]
    System,
    En,
    Vi,
}

impl LangPref {
    pub fn effective(self) -> Lang {
        match self {
            Self::En => Lang::En,
            Self::Vi => Lang::Vi,
            Self::System => system_lang(),
        }
    }
}

/// Vietnamese when the desktop's preferred language is Vietnamese, else English.
pub fn system_lang() -> Lang {
    match sys_language() {
        Some(tag) if tag.to_ascii_lowercase().starts_with("vi") => Lang::Vi,
        _ => Lang::En,
    }
}

#[cfg(windows)]
fn sys_language() -> Option<String> {
    // The user's preferred UI languages, most preferred first (safe WinRT call).
    let langs = windows::System::UserProfile::GlobalizationPreferences::Languages().ok()?;
    let first = langs.GetAt(0).ok()?;
    Some(first.to_string())
}

#[cfg(not(windows))]
fn sys_language() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty() && v != "C" && v != "POSIX"))
}

/// The interface text for `en` in `lang`. Unknown strings stay English.
pub fn t(lang: Lang, en: &'static str) -> &'static str {
    if lang == Lang::En {
        return en;
    }
    vi(en).unwrap_or(en)
}

/// `t`, then `{name}` placeholders filled in.
pub fn tf(lang: Lang, en: &'static str, args: &[(&str, &str)]) -> String {
    let mut s = t(lang, en).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

fn vi(en: &str) -> Option<&'static str> {
    Some(match en {
        "Home" => "Trang chủ",
        "Docs" => "Văn bản",
        "Slides" => "Trình chiếu",
        "Sheets" => "Bảng tính",
        "PDF" => "PDF",
        "Documents" => "Tài liệu",
        "Presentations" => "Bản trình bày",
        "Spreadsheets" => "Bảng tính",
        "PDF documents" => "Tài liệu PDF",
        "Make your documents glow." => "Làm tài liệu của bạn tỏa sáng.",
        "Good morning" => "Chào buổi sáng",
        "Good afternoon" => "Chào buổi chiều",
        "Good evening" => "Chào buổi tối",
        "New" => "Tạo mới",
        "Blank document" => "Tài liệu trống",
        "Blank presentation" => "Bản trình bày trống",
        "Blank workbook" => "Sổ làm việc trống",
        "Blank PDF" => "PDF trống",
        "Open…" => "Mở…",
        "Open a file" => "Mở tệp",
        "Open file" => "Mở tệp",
        "Recent" => "Gần đây",
        "No recent files yet. Open or create something to get started." => {
            "Chưa có tệp nào gần đây. Hãy mở hoặc tạo một tệp để bắt đầu."
        }
        "Drop files anywhere to open them." => "Kéo thả tệp vào bất kỳ đâu để mở.",
        "Remove from list" => "Xóa khỏi danh sách",
        "Open file location" => "Mở thư mục chứa tệp",
        "Clear list" => "Xóa danh sách",
        "Settings" => "Cài đặt",
        "Language" => "Ngôn ngữ",
        "Same as Windows" => "Theo Windows",
        "English" => "English",
        "Tiếng Việt" => "Tiếng Việt",
        "Smooth scrolling" => "Cuộn mượt",
        "Off" => "Tắt",
        "Standard" => "Tiêu chuẩn",
        "Fluid (like Microsoft Edge)" => "Mượt (như Microsoft Edge)",
        "Animations" => "Hiệu ứng chuyển động",
        "Same as Windows (Animation effects)" => "Theo Windows (Hiệu ứng hoạt ảnh)",
        "On" => "Bật",
        "Interface size" => "Cỡ giao diện",
        "File types" => "Loại tệp",
        "Open documents, presentations, workbooks and PDFs with Themisio" => {
            "Mở tài liệu, bản trình bày, sổ làm việc và PDF bằng Themisio"
        }
        "Register file types" => "Đăng ký loại tệp",
        "Choose default apps…" => "Chọn ứng dụng mặc định…",
        "File types registered. Pick Themisio in Default apps to make it the default." => {
            "Đã đăng ký loại tệp. Chọn Themisio trong Ứng dụng mặc định để đặt làm mặc định."
        }
        "Couldn't register file types: {error}" => "Không thể đăng ký loại tệp: {error}",
        "About Themisio" => "Giới thiệu Themisio",
        "Documents, presentations, spreadsheets and PDFs in one window." => "Văn bản, trình chiếu, bảng tính và PDF trong một cửa sổ.",
        "Open-source licences" => "Giấy phép mã nguồn mở",
        "Themisio is built on open-source engines by the ArtCraft team (MIT OR Apache-2.0):" => {
            "Themisio được xây dựng trên các bộ máy mã nguồn mở của nhóm ArtCraft (MIT OR Apache-2.0):"
        }
        "Version" => "Phiên bản",
        "App bar" => "Thanh ứng dụng",
        "Auto-hide" => "Tự ẩn",
        "Always show" => "Luôn hiện",
        "Auto-hide the app bar" => "Tự ẩn thanh ứng dụng",
        "Keep the app bar open" => "Ghim thanh ứng dụng",
        "GPU (Direct3D 12)" => "GPU (Direct3D 12)",
        "OpenGL (least memory)" => "OpenGL (ít bộ nhớ nhất)",
        "Close" => "Đóng",
        "Cancel" => "Hủy",
        "Open with" => "Mở bằng",
        "How do you want to open {name}?" => "Bạn muốn mở {name} bằng gì?",
        "Edit as a document" => "Chỉnh sửa như văn bản",
        "Import as slides (outline)" => "Nhập thành trang chiếu (dàn ý)",
        "Open as data in a workbook" => "Mở dữ liệu trong sổ làm việc",
        "Convert to PDF" => "Chuyển thành PDF",
        "Start the slide show" => "Bắt đầu trình chiếu",
        "Edit the presentation" => "Chỉnh sửa bản trình bày",
        "Open" => "Mở",
        "Always do this for .{ext} files" => "Luôn làm vậy với tệp .{ext}",
        "Themisio can't open {name}." => "Themisio không mở được {name}.",
        "Opening {app}…" => "Đang mở {app}…",
        "Closing Themisio…" => "Đang đóng Themisio…",
        "{app} is asking about unsaved changes." => "{app} đang hỏi về các thay đổi chưa lưu.",
        "Don't close" => "Không đóng",
        "Close {app}" => "Đóng {app}",
        "Running" => "Đang chạy",
        "Go to {app}" => "Chuyển sang {app}",
        "Create, open and edit Word documents" => "Tạo, mở và chỉnh sửa tài liệu Word",
        "Build and present slide shows" => "Thiết kế và trình chiếu bài thuyết trình",
        "Calculate, chart and analyse data" => "Tính toán, vẽ biểu đồ và phân tích dữ liệu",
        "Read, annotate, sign and edit PDFs" => "Đọc, chú thích, ký và chỉnh sửa PDF",
        "Restart Themisio to apply this." => "Khởi động lại Themisio để áp dụng.",
        "Rendering" => "Kết xuất",
        "Low-latency (higher power use)" => "Độ trễ thấp (tốn điện hơn)",
        "Balanced (VSync)" => "Cân bằng (VSync)",
        "just now" => "vừa xong",
        "{n} min ago" => "{n} phút trước",
        "{n} h ago" => "{n} giờ trước",
        "yesterday" => "hôm qua",
        "{n} days ago" => "{n} ngày trước",
        "The file no longer exists." => "Tệp không còn tồn tại.",
        "Theme" => "Giao diện",
        "Fluid" => "Mượt",
        "Frame pacing" => "Nhịp khung hình",
        "Low latency" => "Độ trễ thấp",
        "Light" => "Sáng",
        "Dark" => "Tối",
        "Keyboard: Ctrl+Alt+1…5 switch apps, Ctrl+Alt+O opens a file." => {
            "Phím tắt: Ctrl+Alt+1…5 chuyển ứng dụng, Ctrl+Alt+O mở tệp."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_returned_as_is_and_vietnamese_is_translated() {
        assert_eq!(t(Lang::En, "Home"), "Home");
        assert_eq!(t(Lang::Vi, "Home"), "Trang chủ");
        assert_eq!(t(Lang::Vi, "not a known string"), "not a known string");
        assert_eq!(tf(Lang::Vi, "Opening {app}…", &[("app", "Word")]), "Đang mở Word…");
    }
}
