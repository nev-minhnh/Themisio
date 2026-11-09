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
        "All done" => "Đã xử lý hết",
        "Translate" => "Dịch",
        "Ask AI…" => "Hỏi AI…",
        "Check writing with AI" => "Kiểm tra văn bản bằng AI",
        "Check" => "Kiểm tra",
        "Ask" => "Hỏi",
        "Thinking" => "Suy nghĩ",
        "No model — Settings ▸ AI" => "Chưa có model — Cài đặt ▸ AI",
        "Model default" => "Mặc định của model",
        "Low" => "Thấp",
        "Medium" => "Vừa",
        "High" => "Cao",
        "Extra" => "Rất cao",
        "Max" => "Tối đa",
        "Select some text, or put the cursor in a paragraph or a cell with text." => "Hãy chọn một đoạn chữ, hoặc đặt con trỏ trong một đoạn văn hay một ô có chữ.",
        "Sign in to an AI provider in Settings ▸ AI first." => "Hãy đăng nhập một nhà cung cấp AI trong Cài đặt ▸ AI trước.",
        "Select the text to translate." => "Hãy chọn đoạn chữ cần dịch.",
        "Looks good — no suggestions." => "Ổn rồi — không có gợi ý nào.",
        "Detected language: {lang}" => "Ngôn ngữ nhận ra: {lang}",
        "Checking…" => "Đang kiểm tra…",
        "Writing…" => "Đang viết…",
        "Translating…" => "Đang dịch…",
        "Try again" => "Thử lại",
        "Selection" => "Vùng chọn",
        "Paragraph" => "Đoạn văn",
        "{n} suggestions" => "{n} gợi ý",
        "Accept all" => "Nhận tất cả",
        "Check again" => "Kiểm tra lại",
        "Applied" => "Đã áp dụng",
        "Couldn't apply — the text changed" => "Không áp dụng được — chữ đã thay đổi",
        "Accept" => "Nhận",
        "Dismiss" => "Bỏ qua",
        "What should AI do? e.g. make it more formal, shorter, fix the tone, continue this paragraph…" => "AI cần làm gì? Ví dụ: viết trang trọng hơn, ngắn gọn hơn, chỉnh giọng văn, viết tiếp đoạn này…",
        "Fix grammar" => "Sửa ngữ pháp",
        "More formal" => "Trang trọng hơn",
        "Shorter" => "Ngắn gọn hơn",
        "Clearer" => "Rõ ràng hơn",
        "Friendlier" => "Thân thiện hơn",
        "Skills" => "Skills",
        "No skills yet. Add SKILL.md folders in Settings ▸ AI." => "Chưa có skill nào. Thêm thư mục SKILL.md trong Cài đặt ▸ AI.",
        "Send" => "Gửi",
        "Replace" => "Thay thế",
        "Copy" => "Sao chép",
        "Writing suggestions" => "Gợi ý chỉnh sửa",
        "Spelling" => "Chính tả",
        "Grammar" => "Ngữ pháp",
        "Punctuation" => "Dấu câu",
        "Syntax" => "Cú pháp",
        "Clarity" => "Rõ ràng",
        "Style" => "Văn phong",
        "Vietnamese ↔ English" => "Việt ↔ Anh",
        "Vietnamese" => "Tiếng Việt",
        "Chinese (Simplified)" => "Tiếng Trung (giản thể)",
        "Chinese (Traditional)" => "Tiếng Trung (phồn thể)",
        "Japanese" => "Tiếng Nhật",
        "Korean" => "Tiếng Hàn",
        "French" => "Tiếng Pháp",
        "German" => "Tiếng Đức",
        "Spanish" => "Tiếng Tây Ban Nha",
        "Portuguese" => "Tiếng Bồ Đào Nha",
        "Italian" => "Tiếng Ý",
        "Russian" => "Tiếng Nga",
        "Thai" => "Tiếng Thái",
        "Indonesian" => "Tiếng Indonesia",
        "Arabic" => "Tiếng Ả Rập",
        "Hindi" => "Tiếng Hindi",
        "Text replaced" => "Đã thay thế đoạn chữ",
        "Couldn't replace the text: {error}" => "Không thay được đoạn chữ: {error}",
        "Copied to the clipboard" => "Đã sao chép vào bộ nhớ tạm",
        "{n} changes made — Ctrl+Z undoes them" => "Đã thực hiện {n} thay đổi — Ctrl+Z để hoàn tác",
        "{done} changes made, {failed} failed" => "Đã thực hiện {done} thay đổi, {failed} thất bại",
        "Ask AI" => "Hỏi AI",
        "CalcSheet assistant (Ctrl+Alt+A)" => "Trợ lý CalcSheet (Ctrl+Alt+A)",
        "CalcSheet assistant" => "Trợ lý CalcSheet",
        "New chat" => "Cuộc trò chuyện mới",
        "Ask about the data on this sheet — the selection, or the whole sheet when nothing is selected." => "Hỏi về dữ liệu trên sheet này — vùng đang chọn, hoặc cả sheet nếu chưa chọn gì.",
        "Summarise this data and point out anything unusual" => "Tóm tắt dữ liệu này và chỉ ra điểm bất thường",
        "What trends do you see?" => "Bạn thấy xu hướng gì?",
        "Suggest useful formulas and a chart" => "Gợi ý công thức hữu ích và một biểu đồ",
        "Check the data for errors and duplicates" => "Kiểm tra dữ liệu có lỗi và trùng lặp không",
        "Proposed changes" => "Thay đổi đề xuất",
        "Not allowed — skipped" => "Không được phép — bỏ qua",
        "Done" => "Xong",
        "Allow and run {n} changes" => "Cho phép và thực hiện {n} thay đổi",
        "Analysing…" => "Đang phân tích…",
        "Ask about your data… (Ctrl+Enter to send)" => "Hỏi về dữ liệu của bạn… (Ctrl+Enter để gửi)",
        "Nothing changes until you allow it." => "Không có gì thay đổi khi bạn chưa cho phép.",
        "General" => "Chung",
        "AI" => "AI",
        "Providers" => "Nhà cung cấp",
        "Keys are stored in Windows Credential Manager. Requests go directly from your PC to the provider. Keys you saved in Callio are used too." => "Key được lưu trong Windows Credential Manager. Yêu cầu đi thẳng từ máy bạn đến nhà cung cấp. Key đã lưu trong Callio cũng dùng được.",
        "Remove this provider" => "Xóa nhà cung cấp này",
        "Add an OpenAI-compatible provider:" => "Thêm nhà cung cấp tương thích OpenAI:",
        "Other…" => "Khác…",
        "Use Claude Code (works with a Claude Pro or Max plan, no API key)" => "Dùng Claude Code (dùng gói Claude Pro hoặc Max, không cần API key)",
        "Looking for Claude Code…" => "Đang tìm Claude Code…",
        "Claude Code isn't installed on this PC." => "Máy này chưa cài Claude Code.",
        "How to install" => "Cách cài đặt",
        "signed in with your Claude plan" => "đã đăng nhập bằng gói Claude của bạn",
        "Sign out" => "Đăng xuất",
        "not signed in" => "chưa đăng nhập",
        "Sign in" => "Đăng nhập",
        "Refresh" => "Làm mới",
        "Themisio runs Anthropic's official Claude Code with your own sign-in and never sees your password or tokens. It only reads the text you send and changes nothing on your PC. Usage counts toward your plan's limits." => "Themisio chạy Claude Code chính thức của Anthropic bằng tài khoản của bạn và không bao giờ thấy mật khẩu hay token. Nó chỉ đọc đoạn chữ bạn gửi và không thay đổi gì trên máy. Lượt dùng được tính vào giới hạn của gói.",
        "Other Claude Code model id:" => "Model Claude Code khác:",
        "Add" => "Thêm",
        "Models" => "Model",
        "Writing assistant" => "Trợ lý viết",
        "Effort" => "Mức nỗ lực",
        "Let the model think before answering (slower, more thorough)" => "Cho model suy nghĩ trước khi trả lời (chậm hơn, kỹ hơn)",
        "On in:" => "Bật trong:",
        "Suggest style, clarity and word choice as well as spelling and grammar" => "Gợi ý cả văn phong, độ rõ ràng và cách dùng từ, ngoài chính tả và ngữ pháp",
        "Check automatically after a pause in typing (each check is a request to the provider)" => "Tự kiểm tra khi ngừng gõ (mỗi lần kiểm tra là một yêu cầu tới nhà cung cấp)",
        "Pause before checking" => "Thời gian chờ trước khi kiểm tra",
        "Your instructions for every writing request" => "Hướng dẫn của bạn cho mọi yêu cầu viết",
        "e.g. Use British spelling. Keep an academic tone. Don't change citations." => "Ví dụ: Giữ giọng văn học thuật. Không sửa trích dẫn. Dùng tiếng Việt chuẩn.",
        "Shortcuts: Ctrl+Alt+G check writing, Ctrl+Alt+K ask AI, Ctrl+Alt+T translate — or right-click the text." => "Phím tắt: Ctrl+Alt+G kiểm tra, Ctrl+Alt+K hỏi AI, Ctrl+Alt+T dịch — hoặc nhấp chuột phải vào chữ.",
        "Translation" => "Dịch",
        "Translate with" => "Dịch bằng",
        "Into" => "Sang",
        "Writing skills are folders with a SKILL.md (as Claude Code uses). Active skills are added to every writing request." => "Skill viết là thư mục có tệp SKILL.md (như Claude Code dùng). Skill đang bật được thêm vào mọi yêu cầu viết.",
        "No skills found yet." => "Chưa tìm thấy skill nào.",
        "Open Themisio's skills folder" => "Mở thư mục skills của Themisio",
        "Add a skills folder…" => "Thêm thư mục skills…",
        "Use this provider" => "Dùng nhà cung cấp này",
        "Name" => "Tên",
        "API key" => "API key",
        "Saved" => "Đã lưu",
        "Change" => "Đổi",
        "Remove" => "Xóa",
        "optional" => "không bắt buộc",
        "Get a key" => "Lấy key",
        "Address" => "Địa chỉ",
        "model id" => "tên model",
        "Load the provider's models" => "Tải danh sách model",
        "Test" => "Thử",
        "Same as the writing assistant" => "Giống trợ lý viết",
        "First available" => "Model đầu tiên có sẵn",
        "Turn on a provider above, or {cc}." => "Bật một nhà cung cấp ở trên, hoặc {cc}.",
        "Save" => "Lưu",
        "Saved {name}" => "Đã lưu {name}",
        "Opened {name}" => "Đã mở {name}",
        "Save As…" => "Lưu thành…",
        "Print…" => "In…",
        "Pin tab" => "Ghim tab",
        "Unpin tab" => "Bỏ ghim tab",
        "Close tab" => "Đóng tab",
        "Close other tabs" => "Đóng các tab khác",
        "Close tabs to the right" => "Đóng các tab bên phải",
        "Couldn't do that: {error}" => "Không thực hiện được: {error}",
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
