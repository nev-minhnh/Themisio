//! What the CalcSheet assistant is told about the workbook: every cell with its address (column
//! letters across the top, row numbers down the side), values as the sheet shows them (dates as
//! dates, percentages as percentages), formulas next to their results, and a profile of every
//! column computed over all of its rows — so the model's formulas point at the right cells and
//! its numbers about large sheets come from the data rather than from guessing.

use serde_json::{Value, json};

/// Reads a range: `(range, formatted, formulas)` → `sheet.read`'s answer.
pub type Reader<'a> = dyn FnMut(&str, bool, bool) -> Result<Value, String> + 'a;

/// Rows shown cell by cell: all of them up to `ALL_ROWS`, else the first `HEAD` and the last `TAIL`.
const ALL_ROWS: u32 = 220;
const HEAD: u32 = 120;
const TAIL: u32 = 15;
const MAX_COLS: u32 = 40;
/// Rows the column profile reads (in chunks of `CHUNK`).
const PROFILE_ROWS: u32 = 200_000;
const CHUNK: u32 = 20_000;
const MAX_CELL: usize = 80;

/// Column 1 → "A", 27 → "AA".
pub fn col_letters(mut col: u32) -> String {
    let mut s = Vec::new();
    while col > 0 {
        let r = ((col - 1) % 26) as u8;
        s.push(b'A' + r);
        col = (col - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// "B6" → (column 2, row 6).
pub fn parse_cell(a1: &str) -> Option<(u32, u32)> {
    let a1 = a1.replace('$', "");
    let a1 = a1.rsplit('!').next().unwrap_or(&a1);
    let letters: String = a1.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits: String = a1.chars().skip_while(|c| c.is_ascii_alphabetic()).collect();
    if letters.is_empty() || letters.len() > 3 {
        return None;
    }
    let col = letters.to_ascii_uppercase().bytes().fold(0u32, |n, b| n.saturating_mul(26).saturating_add(u32::from(b - b'A' + 1)));
    let row = digits.parse().ok()?;
    (row > 0).then_some((col, row))
}

/// "B2:H52" → ((2, 2), (8, 52)).
pub fn parse_range(r: &str) -> Option<((u32, u32), (u32, u32))> {
    let mut it = r.split(':');
    let a = parse_cell(it.next()?)?;
    let b = it.next().map_or(Some(a), parse_cell)?;
    Some(((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1))))
}

fn range_a1(c0: u32, r0: u32, c1: u32, r1: u32) -> String {
    format!("{}{r0}:{}{r1}", col_letters(c0), col_letters(c1))
}

fn cell_text(v: &Value) -> String {
    let s = match v {
        Value::Null => String::new(),
        Value::String(t) => t.clone(),
        Value::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
        other => other.to_string(),
    };
    let s = s.replace(['\t', '\n', '\r'], " ");
    if s.chars().count() > MAX_CELL { format!("{}…", s.chars().take(MAX_CELL).collect::<String>()) } else { s }
}

/// One column's profile over its data rows.
#[derive(Default)]
struct Profile {
    count: usize,
    blanks: usize,
    numbers: usize,
    texts: usize,
    errors: usize,
    sum: f64,
    min: Option<(f64, u32)>,
    max: Option<(f64, u32)>,
    distinct: std::collections::HashMap<String, usize>,
    distinct_capped: bool,
}

impl Profile {
    fn add(&mut self, v: &Value, row: u32) {
        match v {
            Value::Null => self.blanks += 1,
            Value::Number(n) => {
                let x = n.as_f64().unwrap_or(0.0);
                self.count += 1;
                self.numbers += 1;
                self.sum += x;
                if self.min.is_none_or(|(m, _)| x < m) {
                    self.min = Some((x, row));
                }
                if self.max.is_none_or(|(m, _)| x > m) {
                    self.max = Some((x, row));
                }
                self.note(num(x));
            }
            Value::String(s) if s.is_empty() => self.blanks += 1,
            Value::String(s) if s.starts_with('#') && s.ends_with(['!', '?', 'A']) && s.len() <= 8 => {
                self.count += 1;
                self.errors += 1;
            }
            Value::String(s) => {
                self.count += 1;
                self.texts += 1;
                self.note(s.clone());
            }
            Value::Bool(b) => {
                self.count += 1;
                self.note(b.to_string());
            }
            _ => self.count += 1,
        }
    }

    fn note(&mut self, k: String) {
        if let Some(n) = self.distinct.get_mut(&k) {
            *n += 1;
        } else if self.distinct.len() < 5000 {
            self.distinct.insert(k, 1);
        } else {
            self.distinct_capped = true;
        }
    }
}

fn num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 { format!("{x:.0}") } else { format!("{:.4}", x).trim_end_matches('0').trim_end_matches('.').to_string() }
}

/// The description of the workbook for the assistant. `meta`: title, sheet names, the active
/// sheet and the selection.
/// `used`: the active sheet's used range ("B2:H52"), `None` when it is empty.
pub fn describe(read: &mut Reader, title: &str, sheets: &[String], active: &str, selection: &str, used: Option<String>) -> String {
    let mut out = format!("Workbook: {title}\nSheets: {}\nActive sheet: {active}\nSelection: {selection}\n", sheets.join(", "));
    let Some(((c0, r0), (c1, r1))) = used.as_deref().and_then(parse_range) else {
        out.push_str("\nThe active sheet is empty.\n");
        return out;
    };
    let c1 = c1.min(c0 + MAX_COLS - 1);
    let rows = r1 - r0 + 1;
    out.push_str(&format!(
        "\nUsed range of the active sheet: {} ({rows} rows × {} columns, columns {}–{}).\n",
        used.as_deref().unwrap_or(""),
        c1 - c0 + 1,
        col_letters(c0),
        col_letters(c1)
    ));

    // The header row: the first row with at least two cells, all text, followed by data.
    let head = read(&range_a1(c0, r0, c1, (r0 + 30).min(r1)), false, false).ok();
    let head_rows: Vec<Vec<Value>> = head
        .as_ref()
        .and_then(|v| v["values"].as_array())
        .map(|rows| rows.iter().map(|r| r.as_array().cloned().unwrap_or_default()).collect())
        .unwrap_or_default();
    let mut header: Option<u32> = None;
    for (i, row) in head_rows.iter().enumerate() {
        let filled: Vec<&Value> = row.iter().filter(|v| !v.is_null() && v.as_str() != Some("")).collect();
        if filled.len() >= 2 && filled.iter().all(|v| v.is_string()) && head_rows.get(i + 1).is_some_and(|n| n.iter().any(|v| !v.is_null())) {
            header = Some(r0 + i as u32);
            break;
        }
    }
    let names: Vec<String> = match header {
        Some(h) => head_rows.get((h - r0) as usize).map(|r| (0..=(c1 - c0)).map(|c| r.get(c as usize).map(cell_text).unwrap_or_default()).collect()).unwrap_or_default(),
        None => Vec::new(),
    };
    let data_from = header.map_or(r0, |h| h + 1);
    if let Some(h) = header {
        out.push_str(&format!("Header row: {h}; data rows {data_from}–{r1} ({} rows).\n", r1.saturating_sub(data_from) + 1));
    }

    // The profile of every column, over all data rows (or the first PROFILE_ROWS).
    let mut profiles: Vec<Profile> = (c0..=c1).map(|_| Profile::default()).collect();
    let last = r1.min(data_from + PROFILE_ROWS - 1);
    let mut r = data_from;
    while r <= last {
        let end = (r + CHUNK - 1).min(last);
        if let Ok(v) = read(&range_a1(c0, r, c1, end), false, false) {
            for (i, row) in v["values"].as_array().into_iter().flatten().enumerate() {
                for (c, cell) in row.as_array().into_iter().flatten().enumerate() {
                    if let Some(p) = profiles.get_mut(c) {
                        p.add(cell, r + i as u32);
                    }
                }
            }
        }
        r = end + 1;
    }
    let shown_of = |c: u32, row: u32, read: &mut Reader| -> String {
        let a = format!("{}{row}", col_letters(c));
        read(&a, true, false).ok().and_then(|v| v["values"][0][0].as_str().map(str::to_string)).unwrap_or_default()
    };
    out.push_str(&format!(
        "\nColumn profile (computed over {} data rows{}):\n",
        last.saturating_sub(data_from) + 1,
        if last < r1 { format!(" — the first {PROFILE_ROWS} of {}", r1 - data_from + 1) } else { String::new() }
    ));
    for (i, p) in profiles.iter().enumerate() {
        let c = c0 + i as u32;
        let letter = col_letters(c);
        if p.count == 0 {
            continue;
        }
        let name = names.get(i).filter(|n| !n.is_empty()).map(|n| format!(" \"{n}\"")).unwrap_or_default();
        let range = format!("{letter}{data_from}:{letter}{last}");
        let mut line = format!("- {letter}{name} ({range}): {} values", p.count);
        if p.blanks > 0 {
            line.push_str(&format!(", {} blank", p.blanks));
        }
        if p.errors > 0 {
            line.push_str(&format!(", {} error cells", p.errors));
        }
        if p.numbers > 0 && p.numbers >= p.texts {
            let (lo, lo_row) = p.min.unwrap_or((0.0, data_from));
            let (hi, hi_row) = p.max.unwrap_or((0.0, data_from));
            let lo_shown = shown_of(c, lo_row, read);
            let hi_shown = shown_of(c, hi_row, read);
            let formatted = lo_shown != num(lo) && !lo_shown.is_empty();
            if formatted {
                line.push_str(&format!(
                    "; numbers shown as \"{lo_shown}\" … \"{hi_shown}\" (stored values {} … {}, min at row {lo_row}, max at row {hi_row})",
                    num(lo),
                    num(hi)
                ));
            } else {
                line.push_str(&format!("; numbers, min {} (row {lo_row}), max {} (row {hi_row})", num(lo), num(hi)));
            }
            if formatted {
                line.push_str(&format!(", mean of stored values {}", num(p.sum / p.numbers as f64)));
            } else {
                line.push_str(&format!(", sum {}, mean {}", num(p.sum), num(p.sum / p.numbers as f64)));
            }
            if p.texts > 0 {
                line.push_str(&format!("; also {} text cells", p.texts));
            }
        } else if p.texts > 0 {
            line.push_str("; text");
        }
        let distinct = p.distinct.len();
        line.push_str(&format!(", {}{distinct} distinct", if p.distinct_capped { "over " } else { "" }));
        if distinct == p.count && p.count > 1 {
            line.push_str(" (all different)");
        } else if distinct > 1 && (distinct <= 40 || p.texts > p.numbers) && !(p.numbers > p.texts && p.distinct.len() > 12) {
            let mut top: Vec<(&String, &usize)> = p.distinct.iter().filter(|(_, n)| **n > 1).collect();
            top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let list: Vec<String> = top.iter().take(15).map(|(k, n)| format!("{} ×{n}", cell_text(&json!(k)))).collect();
            if !list.is_empty() {
                line.push_str(&format!("; most common: {}", list.join(", ")));
            }
        }
        out.push_str(&line);
        out.push('\n');
    }

    // The cells.
    let blocks: Vec<(u32, u32)> = if rows <= ALL_ROWS { vec![(r0, r1)] } else { vec![(r0, (r0 + HEAD - 1).min(r1)), (r1 - TAIL + 1, r1)] };
    out.push_str(&format!(
        "\nCells as displayed. First column = row number, then one column per sheet column ({}…{}). A formula cell shows its result then {{=FORMULA}}.{}\n",
        col_letters(c0),
        col_letters(c1),
        if blocks.len() > 1 { format!(" Rows {}–{} are left out here.", blocks[0].1 + 1, blocks[1].0 - 1) } else { String::new() }
    ));
    let mut header_line = String::from("row");
    for c in c0..=c1 {
        header_line.push('\t');
        header_line.push_str(&col_letters(c));
    }
    out.push_str(&header_line);
    out.push('\n');
    for (i, (a, b)) in blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("…\t(rows left out)\n");
        }
        let range = range_a1(c0, *a, c1, *b);
        let shown = read(&range, true, false).ok();
        let formulas = read(&range, false, true).ok();
        for (k, row) in shown.as_ref().and_then(|v| v["values"].as_array()).into_iter().flatten().enumerate() {
            let cells: Vec<Value> = row.as_array().cloned().unwrap_or_default();
            if cells.iter().all(|v| v.is_null() || v.as_str() == Some("")) {
                continue;
            }
            let mut line = (a + k as u32).to_string();
            for (c, v) in cells.iter().enumerate() {
                line.push('\t');
                line.push_str(&cell_text(v));
                if let Some(f) = formulas.as_ref().and_then(|f| f["values"][k][c].as_str()).filter(|f| f.starts_with('=')) {
                    line.push_str(&format!(" {{{f}}}"));
                }
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out
}

/// Error values (`#DIV/0!`, `#NAME?`, …) in the given ranges: (cell, error, formula).
pub fn errors_in(read: &mut Reader, ranges: &[String]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for r in ranges {
        let Some(((c0, r0), (c1, r1))) = parse_range(r) else { continue };
        if (c1 - c0 + 1) as u64 * (r1 - r0 + 1) as u64 > 20_000 {
            continue;
        }
        let range = range_a1(c0, r0, c1, r1);
        let (Ok(shown), Ok(formulas)) = (read(&range, true, false), read(&range, false, true)) else { continue };
        for (i, row) in shown["values"].as_array().into_iter().flatten().enumerate() {
            for (j, v) in row.as_array().into_iter().flatten().enumerate() {
                if let Some(s) = v.as_str()
                    && s.starts_with('#')
                    && ["#DIV/0!", "#NAME?", "#REF!", "#VALUE!", "#N/A", "#NUM!", "#NULL!", "#SPILL!", "#CALC!"].contains(&s)
                {
                    let cell = format!("{}{}", col_letters(c0 + j as u32), r0 + i as u32);
                    let f = formulas["values"][i][j].as_str().unwrap_or("").to_string();
                    if !out.iter().any(|(c, _, _): &(String, String, String)| *c == cell) {
                        out.push((cell, s.to_string(), f));
                    }
                }
            }
        }
    }
    out
}

/// The ranges an action writes to, for checking them afterwards.
pub fn written_ranges(command: &str, params: &Value) -> Vec<String> {
    let mut out = Vec::new();
    match command {
        "cell.set" => {
            if let Some(c) = params["cell"].as_str() {
                out.push(c.to_string());
            }
        }
        "range.setValues" | "range.fill" | "home.autoSum" => {
            for k in ["range", "to", "target", "destination"] {
                if let Some(r) = params[k].as_str() {
                    // A start cell with a block of values: cover the block.
                    if !r.contains(':')
                        && let (Some((c, row)), Some(rows)) = (parse_cell(r), params["values"].as_array())
                    {
                        let w = rows.iter().filter_map(Value::as_array).map(Vec::len).max().unwrap_or(1) as u32;
                        out.push(range_a1(c, row, c + w.max(1) - 1, row + rows.len().max(1) as u32 - 1));
                    } else {
                        out.push(r.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(parse_cell("B6"), Some((2, 6)));
        assert_eq!(parse_cell("$AA$10"), Some((27, 10)));
        assert_eq!(parse_cell("Sheet1!C3"), Some((3, 3)));
        assert_eq!(parse_range("H52:B2"), Some(((2, 2), (8, 52))));
        assert_eq!(col_letters(28), "AB");
        assert_eq!(written_ranges("range.setValues", &json!({"range": "J6", "values": [["a", "b", "c"], ["d"]]})), vec!["J6:L7"]);
    }

    /// The sample workbook a user shared, when it is on this machine.
    #[test]
    fn describes_a_workbook_with_addresses_and_profiles() {
        let path = std::env::var("THEMISIO_SAMPLE_XLSX").unwrap_or_default();
        let Ok(bytes) = std::fs::read(&path) else { return };
        let mut s = gridcraft_engine::Session::new();
        s.execute("file.open", json!({"name": "pm.xlsx", "base64": gridcraft_engine::io::base64_encode(&bytes)})).unwrap();
        let mut read = |range: &str, formatted: bool, formulas: bool| -> Result<Value, String> {
            let mut p = json!({"formatted": formatted, "formulas": formulas});
            if !range.is_empty() {
                p["range"] = json!(range);
            }
            s.execute("sheet.read", p).map_err(|e| e.to_string())
        };
        let used = read("", false, false).ok().and_then(|v| v["range"].as_str().map(str::to_string));
        let text = describe(&mut read, "pm.xlsx", &["Data".into()], "Data", "A1", used);
        println!("{text}");
        assert!(text.contains("Header row: 6"));
        assert!(text.contains("- B \"Project Name\" (B7:B52)"));
        assert!(text.contains("1/1/2024"));
    }
}
