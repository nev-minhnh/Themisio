# Themisio changelog

Lịch sử: Themisio có tên **Ostium** ở 0.1.0 và 0.2.0.

## 0.5.1 — 2026-11-09
- Popup AI (Tra nghĩa, Dịch, Hỏi AI) tự đóng khi nhấn **Esc** hoặc khi đưa chuột ra xa — không cần
  bấm dấu ×. Popup Hỏi AI không tự đóng khi bạn đang gõ yêu cầu (Esc vẫn đóng), và không đóng khi
  đang mở menu chọn model hay đang kéo popup.

## 0.5.0 — 2026-11-09

**Trợ lý viết kiểu Grammarly** (Documents, Presentation, CalcSheet)
- Gạch chân ngay trong văn bản theo 4 tiêu chí: **Chính xác** (đỏ — chính tả, ngữ pháp, dấu câu),
  **Rõ ràng** (xanh dương), **Cuốn hút** (xanh lá — từ lặp, từ nhạt), **Truyền đạt** (tím — giọng
  văn, mức trang trọng). Rê chuột vào chỗ gạch chân: thẻ giải thích ngắn với **Nhận** / **Bỏ qua** /
  **Thêm vào từ điển**.
- Nút tròn ở góc hiện số gợi ý; bảng trợ lý viết **ghim bên phải** (tài liệu tự thu hẹp, không bị
  che): **Điểm tổng 0–100**, ô đếm cho từng tiêu chí (bấm để lọc), nhận xét giọng văn so với mục tiêu,
  và các thẻ:
  - *Gợi ý*: danh sách gợi ý, bấm để xem chi tiết và nhảy tới chỗ đó trong tài liệu.
  - *Mục tiêu*: lĩnh vực (Học thuật, Kinh doanh, Email, Kỹ thuật, Sáng tạo, Thân mật…), mức trang
    trọng, người đọc, mục đích, biến thể tiếng Anh (Mỹ, Anh, Canada, Úc, Ấn Độ); **từ điển cá nhân**.
  - *Viết*: tạo nội dung từ yêu cầu, viết lại trang trọng hơn / thân thiện hơn / ngắn gọn / mở rộng /
    đơn giản hóa, lên ý tưởng, tóm tắt — chèn tại con trỏ hoặc thay phần đang chọn.
  - *Báo cáo*: số từ, câu, độ dài câu, thời gian đọc, từ khác nhau, từ dùng nhiều; tiến bộ theo tuần.
  - *Đạo văn*: tìm trên web các đoạn trùng (Claude, Claude Code hoặc Perplexity).
- **Kiểm tra khi gõ** hoặc dán (bật trong Cài đặt), hoặc bấm *Kiểm tra ngay* / Ctrl+Alt+G. Chỉ
  đoạn đã đổi được gửi lại; nhận một gợi ý không làm mất các gợi ý khác trong đoạn.
- **Tra nghĩa**: nhấp đúp vào một từ (hoặc Ctrl+Alt+D, chuột phải › Tra nghĩa).

**Trợ lý CalcSheet** — làm lại
- Đọc dữ liệu chính xác: mỗi ô kèm địa chỉ (chữ cột, số hàng), giá trị như đang hiển thị (ngày ra
  ngày, phần trăm ra phần trăm), công thức cạnh kết quả, và **thống kê từng cột trên toàn bộ hàng**
  (kể cả tệp CSV/TSV lớn — trước đây báo "trang tính trống").
- **Trả lời theo từng token** (streaming), hiển thị Markdown đúng: tiêu đề, in đậm/nghiêng, danh
  sách, bảng, code.
- **Enter để gửi**, Shift+Enter xuống dòng; nút Gửi/Dừng.
- Sau khi chạy các thay đổi, Themisio **kiểm tra ô vừa ghi**: ô báo lỗi (#DIV/0!, #NAME?, #REF!…)
  được liệt kê, kèm nút **Nhờ AI sửa**.
- Khung chat ghim bên phải (bảng tính tự thu hẹp), giao diện mới: tin nhắn của bạn màu xanh logo,
  thẻ "Thay đổi đề xuất" với ô chọn, Cho phép / Bỏ qua, Chi tiết.

**Tốc độ**
- Cuộn mượt với CSV/TSV và Excel lớn: phạm vi đã dùng của trang tính được tính một lần cho mỗi khối
  hàng thay vì duyệt mọi hàng ở mỗi khung hình (300.000 hàng: ~35 ms → dưới 4 ms mỗi khung).
- Thống kê vùng chọn trên thanh trạng thái (Tổng, Trung bình, Đếm) chỉ tính lại khi dữ liệu hoặc
  vùng chọn đổi — chọn cả cột của bảng lớn không còn làm cuộn chậm.

**Tab**
- **Kéo thả để đổi thứ tự tab** (trong nhóm ghim và nhóm thường), tab bên cạnh trượt nhường chỗ.
- Đóng tài liệu cuối cùng thì về Trang chủ; mở lại ứng dụng đó sẽ thấy màn hình bắt đầu (mới, mẫu,
  gần đây) thay vì một vùng trống.

**Cài đặt** — thiết kế lại
- Thanh bên trái: Chung, Hiệu năng, Tệp, Giới thiệu; Nhà cung cấp AI, Tính năng AI, Dịch, Skills.
- Mỗi trang là các thẻ với tên + giải thích bên trái và điều khiển bên phải; công tắc bật/tắt; nút
  chính màu logo (xanh navy trên nền sáng, vàng trên nền tối).
- Nhà cung cấp AI là các thẻ gọn: trạng thái (Sẵn sàng · n model / Cần API key / Tắt), bấm để mở
  key, địa chỉ, model (chip có nút xóa), Tải danh sách model, Thử.

**Giao diện**
- Cỡ chữ cân đối hơn: tiêu đề hộp thoại của các ứng dụng không còn to quá cỡ (ví dụ hộp "Lưu thay
  đổi" của Documents); dấu "›" thay "▸" (không còn ô vuông lỗi phông).
- Thông báo nằm cạnh khung AI, không che khung chat.

## 0.4.0 — 2026-11-09

**AI (Cài đặt › AI)** — đăng nhập một lần, dùng cho Documents, Presentation và CalcSheet
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
