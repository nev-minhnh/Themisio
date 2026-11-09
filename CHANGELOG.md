# Themisio changelog

Lịch sử: Themisio có tên **Ostium** ở 0.1.0 và 0.2.0.

## 0.4.0 — 2026-11-09

**AI (Cài đặt ▸ AI)** — đăng nhập một lần, dùng cho Documents, Presentation và CalcSheet
- Nhà cung cấp: Claude (Anthropic), ChatGPT (OpenAI), Gemini (Google AI Studio), Grok (xAI),
  Perplexity, llama.cpp, và bất kỳ API tương thích OpenAI nào (DeepSeek, MiniMax, OpenRouter,
  Mistral, Groq, Together… hoặc tự nhập). Key lưu trong Windows Credential Manager; key đã lưu
  trong Callio được dùng luôn. Yêu cầu đi thẳng từ máy đến nhà cung cấp.
- Claude Code: dùng gói Claude Pro/Max đã đăng nhập (không cần key) — trạng thái, đăng nhập,
  đăng xuất ngay trong Cài đặt.
- Chọn model, mức nỗ lực (effort) và suy nghĩ (thinking) — trong Cài đặt và đổi nhanh ngay trên
  popup.
- **Trợ lý viết kiểu Grammarly**: chuột phải ▸ *Kiểm tra văn bản bằng AI* (Ctrl+Alt+G) — popup
  cạnh con trỏ liệt kê lỗi chính tả, ngữ pháp, dấu câu, cú pháp, văn phong, kèm giải thích; Nhận /
  Bỏ qua / Nhận tất cả (sửa thẳng vào tài liệu, Ctrl+Z hoàn tác). Bật/tắt riêng cho từng ứng dụng.
  Kiểm tra tự động khi ngừng gõ (mặc định tắt) hiện huy hiệu số gợi ý.
- **Hỏi AI** (Ctrl+Alt+K): ô nhập yêu cầu (viết trang trọng hơn, ngắn gọn hơn, viết tiếp…), các yêu
  cầu nhanh, chọn **skills** viết (thư mục SKILL.md như Claude Code; thư mục skills riêng của
  Themisio, của Claude Code, và thư mục bạn thêm), hướng dẫn chung cho mọi yêu cầu.
- **Dịch** (chuột phải ▸ Dịch, Ctrl+Alt+T): mặc định Google Translate; hoặc dịch bằng bất kỳ model
  AI nào; chọn ngôn ngữ đích (mặc định Việt ↔ Anh); Thay thế / Sao chép.
- **Trợ lý CalcSheet** (nút *Hỏi AI*, Ctrl+Alt+A): chat phân tích số liệu (vùng chọn hoặc cả
  sheet), nhận xét, xu hướng, gợi ý bước tiếp theo; đề xuất công thức, bảng, sắp xếp, định dạng,
  biểu đồ — chỉ thực hiện khi bạn bấm *Cho phép*, bằng chính lệnh của CalcSheet (hoàn tác được),
  và chỉ trong danh sách lệnh cho phép.

**Tab**
- Chuột phải vào tab: Lưu, Lưu thành, In; Ghim tab (tab ghim nằm đầu, gọn, không có nút đóng);
  Đóng tab, Đóng các tab khác, Đóng các tab bên phải (từng tài liệu vẫn hỏi về thay đổi chưa lưu).

**Thông báo**
- Popup thông báo thành công/thất bại thống nhất cho mọi thao tác ở cả bốn ứng dụng: lưu, mở,
  xuất, chuyển đổi, in, lỗi… (gom từ thông báo của từng ứng dụng, và tự nhận ra khi tài liệu
  được lưu hoặc mở xong). Rê chuột để giữ, bấm để đóng.

## 0.3.2 — 2026-11-09
- Khởi động lần đầu sau khi bật máy nhanh hơn. Ngay sau khi Windows khởi động, toàn bộ tệp exe
  phải được đọc từ ổ đĩa và được phần mềm diệt virus quét trước khi Themisio kịp chạy, nên thời
  gian tăng theo kích thước tệp. Bản phát hành giờ dùng profile `dist`: tối ưu theo kích thước,
  chỉ giữ tối ưu tốc độ cho các phần nặng (vẽ trang, giải mã ảnh và PDF, dàn trang, phông chữ,
  OCR, đọc tệp), nên tệp exe nhỏ hơn nhiều.
- Nhật ký ghi thời gian khởi động (`%LOCALAPPDATA%\Themisio\logs\themisio.log`): từ lúc Windows
  tạo tiến trình đến `main`, và đến khung hình đầu tiên.

## 0.3.1 — 2026-11-08
- Documents: tên người dùng mặc định (ký hiệu "WU" ở nút tài khoản, tác giả của chú thích và
  theo dõi thay đổi) lấy từ tên người dùng Office trên máy, nếu không có thì từ tên tài khoản
  Windows, thay cho "WordCraft User". Tên đã đặt trong Tệp ▸ Tùy chọn được giữ nguyên.

## 0.3.0 — 2026-11-08

**Tên và nhận diện**
- Đổi tên thành **Themisio – Make your documents glow.**; logo mới (thước vàng, bốn dải màu).
- Thanh ứng dụng bên trái theo màu logo: nền navy chuyển sắc, điểm nhấn vàng; màu của bốn ứng
  dụng lấy từ bốn dải màu của logo.

**Giao diện**
- Hộp thoại Cài đặt viết lại: nút đóng tự vẽ (trước đây ký tự ✕ không có trong phông của ứng
  dụng nên hiện thành ô vuông), các lựa chọn dạng nút phân đoạn, cỡ chữ gọn hơn, hàng thẳng cột.
  Cài đặt, Giới thiệu và hộp "Mở bằng" dùng chung một kiểu, giống nhau trên mọi ứng dụng.
- Biểu tượng vẽ lại và căn giữa: trang chủ, bốn ứng dụng, bánh răng; nút ghim thay bằng biểu
  tượng thanh bên (cột trái tô đặc = luôn hiện, viền = tự ẩn).
- Khi thanh ứng dụng tự ẩn, một đường vàng mảnh ở mép trái báo chỗ đưa chuột vào (sáng hơn khi
  chuột tới gần).
- Nhãn trên thanh ứng dụng, thanh tab, thông báo dùng một phông giao diện riêng của Themisio.

**Chuyển động**
- Dải băng (ribbon) của Documents, Presentation, CalcSheet và bảng công cụ của PDF mờ dần và
  trượt nhẹ vào khi đổi tab.
- Nút, menu, hộp chọn đổi màu khi rê chuột mượt (thay vì đổi ngay một khung hình).
- Menu, danh sách thả xuống và chú thích trượt nhẹ vào chỗ khi mở.
- Tất cả theo Cài đặt ▸ Hiệu ứng chuyển động (mặc định theo Windows).

**Phát hành**
- Mã nguồn trên GitHub (nev-minhnh/Themisio), build Windows bằng GitHub Actions.
- Giấy phép MIT OR Apache-2.0 (LICENSE-MIT, LICENSE-APACHE), NOTICE.

## 0.2.0 — 2026-11-08

**Bộ nhớ (RAM)**
- Nguyên nhân chính của ~500 MB ở 0.1.0: wgpu mặc định dùng gợi ý bộ nhớ "Performance", cấp sẵn
  khối 128–256 MB cho GPU và 64–128 MB cho CPU ngay lần cấp phát đầu tiên. Trên GPU tích hợp
  (Radeon trong Ryzen AI) đó là RAM thường. 0.2.0 dùng "MemoryUsage" (khối 8–64 MB).
- Khi cửa sổ ở nền > 20 giây hoặc thu nhỏ, Themisio trả bớt bộ nhớ cho Windows (EmptyWorkingSet,
  cách Edge làm với tab nền); cũng làm vậy khi đóng một ứng dụng.
- Tùy chọn kết xuất OpenGL (Cài đặt ▸ Kết xuất) cho máy cần ít bộ nhớ nhất.
- Bản phát hành được strip ký hiệu.

**Giao diện**
- Đổi tên: Documents, Presentation, CalcSheet, PDF (cả trong giao diện của từng ứng dụng).
- Thanh tab kiểu Edge cho Documents, Presentation, CalcSheet: mỗi tệp đang mở là một tab
  (bấm để chuyển, × hoặc chuột giữa để đóng, + để tạo mới). PDF dùng thanh tab sẵn có của nó.
- Sửa lỗi: mở tệp thứ hai trong cùng ứng dụng bị ẩn (trước đây mở thành cửa sổ riêng phía sau).
- Thanh ứng dụng bên trái tự ẩn, trượt ra khi chuột chạm mép trái; nút ghim để giữ cố định.
- Đồng bộ giao diện Sáng/Tối cho cả bốn ứng dụng (Cài đặt ▸ Giao diện: Theo Windows/Sáng/Tối).
- Bỏ các phần không cốt lõi: nút/liên kết Discord, thẻ cộng đồng, "Check for updates",
  liên kết trang web ứng dụng.
- Phiên bản hiển thị trong About, tiêu đề cửa sổ và tên tệp phát hành.

## 0.1.0 — 2026-11-08
- Bản đầu tiên.
