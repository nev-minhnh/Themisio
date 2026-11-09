#!/usr/bin/env python3
"""Themisio's small edits to the four apps (applied after fetching them; idempotent).

- Remove non-core extras: Discord / community links and banners, "Check for updates" entries
  (Themisio is updated as one app), app web-page links.
- WordCraft: one window that switches documents (as its web build does), so Themisio's tab bar
  can show every open document instead of each opening in a hidden native window.
"""
import pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "upstream"
EDITS = [
    # ---- WordCraft ----
    ("wordcraft/crates/ui-egui/src/windows.rs",
     "    /// One window for every document, as in the web app.\n    #[cfg(test)]\n    pub(crate) fn single(&mut self) {",
     "    /// One window for every document, as in the web app.\n    pub(crate) fn single(&mut self) {"),
    ("wordcraft/crates/ui-egui/src/windows.rs",
     "impl WordApp {\n    /// How many windows are open.",
     "impl WordApp {\n    /// One window that shows the active document (Themisio switches documents with its tab bar).\n    pub fn use_single_window(&mut self) {\n        self.windows.single();\n    }\n\n    /// How many windows are open."),
    ("wordcraft/crates/ui-egui/src/backstage.rs",
     'for (label, id) in [("About", "about"), ("Discord community", "discord")] {',
     'for (label, id) in [("About", "about")] {'),
    ("wordcraft/crates/ui-egui/src/chrome.rs",
     "                        if app.ui.show_discord {",
     "                        if false && app.ui.show_discord {"),
    ("wordcraft/crates/ui-egui/src/ribbon/review.rs",
     '        big(ui, app, "checkForUpdates", "Check for\\nUpdates…", "ui.checkForUpdates", json!({}), false);\n        big(ui, app, "discord", "Community\\nDiscord", "ui.discord", json!({}), false);\n',
     ""),
    ("wordcraft/crates/ui-egui/src/options.rs",
     '    ui.checkbox(&mut app.ui.show_discord, tl!("Show the community button in the title bar"));\n', ""),
    ("wordcraft/crates/ui-egui/src/dialogs/dispatch.rs",
     '            ui.hyperlink_to(tl!("getartcraft.com/apps/wordcraft"), "https://getartcraft.com/apps/wordcraft");\n            ui.hyperlink_to(tl!("Join us on Discord: discord.gg/artcraft"), "https://discord.gg/artcraft");\n', ""),
    # ---- DeckCraft ----
    ("deckcraft/crates/ui-egui/src/menus.rs",
     '("About DeckCraft", "app.about"), ("Check for Updates…", "app.checkUpdates"), ', '("About DeckCraft", "app.about"), '),
    ("deckcraft/crates/ui-egui/src/backstage/info.rs",
     '            ui.hyperlink_to("Join our community on Discord", deckcraft_engine::links::DISCORD);\n', ""),
    ("deckcraft/crates/ui-egui/src/backstage/files.rs",
     '    ui.add_space(30.0);\n    ui.hyperlink_to("Join the ArtCraft community on Discord", deckcraft_engine::links::DISCORD);\n', ""),
    ("deckcraft/crates/ui-egui/src/dialogs/about.rs",
     '        ui.hyperlink_to("Join our community on Discord", deckcraft_engine::links::DISCORD);\n        ui.hyperlink_to("getartcraft.com/apps/deckcraft", deckcraft_engine::links::APP_PAGE);\n', ""),
    # ---- GridCraft ----
    ("gridcraft/crates/ui-egui/src/dialogs/about.rs",
     '            ui.hyperlink_to("getartcraft.com/apps/gridcraft", "https://getartcraft.com/apps/gridcraft");\n            ui.hyperlink_to("Join the ArtCraft Discord", "https://discord.gg/artcraft");\n', ""),
    ("gridcraft/crates/ui-egui/src/ribbon/automate.rs",
     '    let __k = big_button(ui, Icon::Script, "Check for\\nUpdates…", "Look for a newer GridCraft release", false);\n    control_keytip(app, ui, &__k, "checkForUpdates");\n    if __k.clicked() {\n        app.run_or_alert("app.checkForUpdates", json!({}));\n    }\n', ""),
    # ---- PdfCraft ----
    ("pdfcraft/crates/ui-egui/src/chrome.rs",
     '                    if widgets::ghost_button_sized(ui, "messages-square", "Discord", tab_h).on_hover_text(pdfcraft_engine::links::DISCORD).clicked() {\n                        app.execute("help.discord");\n                    }\n', ""),
    ("pdfcraft/crates/ui-egui/src/menus.rs",
     '&["help.shortcuts", SEP, "help.app_page", "help.github", "help.website", "help.discord", SEP, "help.check_updates", SEP, "help.about"];',
     '&["help.shortcuts", SEP, "help.github", SEP, "help.about"];'),
    ("pdfcraft/crates/ui-egui/src/home.rs",
     "            if !app.community_card_closed {", "            if false && !app.community_card_closed {"),
    # ---- Motion: ribbons and tool panels ease in when their tab changes (egui::themisio) ----
    ("wordcraft/crates/ui-egui/src/ribbon.rs",
     "            let mut measured = 0.0;\n            let scroll = area.show(ui, |ui| {",
     "            let mut measured = 0.0;\n            let themisio_p = egui::themisio::toolbar_transition(ui, app.ui.tab.as_str());\n            let scroll = area.show(ui, |ui| {"),
    ("wordcraft/crates/ui-egui/src/ribbon.rs",
     "                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);\n                    match entry {",
     "                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);\n                    if !rtl && themisio_p < 1.0 {\n                        ui.add_space((1.0 - themisio_p) * 14.0);\n                    }\n                    match entry {"),
    ("deckcraft/crates/ui-egui/src/ribbon.rs",
     "        let scroll =\n            area.scroll_bar_visibility(",
     "        let themisio_p = egui::themisio::toolbar_transition(&mut sui, tab.as_str());\n        let scroll =\n            area.scroll_bar_visibility("),
    ("deckcraft/crates/ui-egui/src/ribbon.rs",
     "                    ui.spacing_mut().item_spacing.x = 1.0;\n                    match tab.as_str() {",
     "                    ui.spacing_mut().item_spacing.x = 1.0;\n                    if themisio_p < 1.0 {\n                        ui.add_space((1.0 - themisio_p) * 14.0);\n                    }\n                    match tab.as_str() {"),
    ("gridcraft/crates/ui-egui/src/ribbon.rs",
     "                let scroll = area.show(ui, |ui| {\n                    ui.horizontal(|ui| {\n                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);\n                        match app.ui.ribbon_tab.as_str() {",
     "                let themisio_p = egui::themisio::toolbar_transition(ui, tab.as_str());\n                let scroll = area.show(ui, |ui| {\n                    ui.horizontal(|ui| {\n                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);\n                        if themisio_p < 1.0 {\n                            ui.add_space((1.0 - themisio_p) * 14.0);\n                        }\n                        match app.ui.ribbon_tab.as_str() {"),
    ("pdfcraft/crates/ui-egui/src/panels/tools.rs",
     "        |ui| match app.left {\n",
     "        |ui| {\n            egui::themisio::toolbar_transition(ui, format!(\"{:?}\", app.left));\n            match app.left {\n"),
    ("pdfcraft/crates/ui-egui/src/panels/tools.rs",
     "            LeftPanel::CalculationOrder => crate::calc_order_ui::panel(app, ui, &t),\n        },\n    );",
     "            LeftPanel::CalculationOrder => crate::calc_order_ui::panel(app, ui, &t),\n            }\n        },\n    );"),

    # ---- Themisio's items (Translate, writing assistant) at the end of the right-click menus ----
    ("wordcraft/crates/ui-egui/src/canvas/context_menu.rs",
     "    // A selected picture, shape, text box or chart: its own items, then Wrap Text (#513).\n",
     "    egui::themisio::context_menu_extras(ui);\n    // A selected picture, shape, text box or chart: its own items, then Wrap Text (#513).\n"),
    ("deckcraft/crates/ui-egui/src/canvas/context_menu.rs",
     "        } else {\n            slide_items(app, ui);\n        }\n    });\n}",
     "        } else {\n            slide_items(app, ui);\n        }\n        egui::themisio::context_menu_extras(ui);\n    });\n}"),
    ("gridcraft/crates/ui-egui/src/grid/menus.rs",
     "                        other => app.run_or_alert(other, json!({})),\n                    }\n                }\n            }\n        });\n    });",
     "                        other => app.run_or_alert(other, json!({})),\n                    }\n                }\n            }\n            if egui::themisio::context_menu_extras(ui) {\n                close = true;\n            }\n        });\n    });"),
    ("wordcraft/crates/ui-egui/src/canvas.rs",
     "\nmod cursor;\n",
     "\npub mod cursor;\n"),
    # ---- Where any text position is on screen (Themisio's writing assistant underlines) ----
    ("wordcraft/crates/ui-egui/src/canvas/cursor.rs",
     "pub fn pos_from_screen(app: &mut WordApp, p: Pos2) -> Option<Pos> {",
     "/// Where `pos` is on screen (the top of a caret there) and the line's height.\npub fn pos_screen(app: &mut WordApp, pos: &Pos) -> Option<(Pos2, f32)> {\n    let l = app.session.layout();\n    let c = l.caret_on(pos, app.session.page_hint)?;\n    let p = page_to_screen(app, c.page, c.x, c.top)?;\n    let scale = app\n        .canvas\n        .page_rects\n        .get(c.page)\n        .and_then(|r| l.pages.get(c.page).map(|page| page_screen_scale(*r, page, app.canvas.scale)))\n        .unwrap_or(app.canvas.scale);\n    Some((p, c.height * scale))\n}\n\npub fn pos_from_screen(app: &mut WordApp, p: Pos2) -> Option<Pos> {"),
    # ---- Balanced type: the save prompt's question at the dialog's size, not a headline ----
    ("wordcraft/crates/ui-egui/src/dialogs/dispatch.rs",
     "&[(\"name\", name)])).font(semibold(15.0)));\n    ui.label(tl!(\"Your changes will be lost if you don't save them.\"));",
     "&[(\"name\", name)])).font(semibold(13.5)));\n    ui.label(tl!(\"Your changes will be lost if you don't save them.\"));"),
    # ---- Speed: CalcSheet's used range in O(bands), not O(rows) (it runs every frame for the
    # scroll bars, so large sheets scrolled slowly) ----
    ("gridcraft/crates/model/src/store.rs",
     "#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]\nstruct Band {\n    /// row → (col → cell)\n    rows: BTreeMap<u32, BTreeMap<u32, Cell>>,\n}\n",
     "#[derive(Clone, Debug, Default, Serialize, Deserialize)]\nstruct Band {\n    /// row → (col → cell)\n    rows: BTreeMap<u32, BTreeMap<u32, Cell>>,\n    /// The band's first and last column, worked out once after each change.\n    #[serde(skip)]\n    cols: std::sync::OnceLock<Option<(u32, u32)>>,\n}\n\nimpl PartialEq for Band {\n    fn eq(&self, other: &Self) -> bool {\n        self.rows == other.rows\n    }\n}\n\nimpl Band {\n    fn col_bounds(&self) -> Option<(u32, u32)> {\n        *self.cols.get_or_init(|| {\n            let mut out: Option<(u32, u32)> = None;\n            for cols in self.rows.values() {\n                if let (Some(a), Some(b)) = (cols.keys().next(), cols.keys().next_back()) {\n                    out = Some(out.map_or((*a, *b), |(x, y)| (x.min(*a), y.max(*b))));\n                }\n            }\n            out\n        })\n    }\n}\n"),
    ("gridcraft/crates/model/src/store.rs",
     "        let band = Arc::make_mut(self.bands.entry(c.row / BAND).or_default());\n        if band.rows",
     "        let band = Arc::make_mut(self.bands.entry(c.row / BAND).or_default());\n        band.cols = std::sync::OnceLock::new();\n        if band.rows"),
    ("gridcraft/crates/model/src/store.rs",
     "        let b = Arc::make_mut(band);\n        let row = b.rows.get_mut(&c.row)?;",
     "        let b = Arc::make_mut(band);\n        b.cols = std::sync::OnceLock::new();\n        let row = b.rows.get_mut(&c.row)?;"),
    ("gridcraft/crates/model/src/store.rs",
     "        for band in self.bands.values() {\n            for cols in band.rows.values() {\n                if let (Some(a), Some(b)) = (cols.keys().next(), cols.keys().next_back()) {\n                    c0 = c0.min(*a);\n                    c1 = c1.max(*b);\n                }\n            }\n        }",
     "        for band in self.bands.values() {\n            if let Some((a, b)) = band.col_bounds() {\n                c0 = c0.min(a);\n                c1 = c1.max(b);\n            }\n        }"),
    # ---- Speed: the status bar's selection statistics are worked out once per change, not on
    # every frame (a whole column of a large sheet selected made scrolling crawl) ----
    ("gridcraft/crates/ui-egui/src/tabs.rs",
     "    let on = app.session.prefs.status_bar;\n    let st = gridcraft_engine::display::stats(sh, &d.selection.ranges);\n",
     "    let on = app.session.prefs.status_bar;\n    thread_local! {\n        static THEMISIO_STATS: std::cell::RefCell<Option<(u64, Vec<String>)>> = const { std::cell::RefCell::new(None) };\n    }\n    let key = {\n        use std::hash::{Hash, Hasher};\n        let mut h = std::collections::hash_map::DefaultHasher::new();\n        (d.uid, d.revision, &sh.name, format!(\"{:?}{:?}{on:?}\", d.selection.ranges, d.selection.active)).hash(&mut h);\n        h.finish()\n    };\n    if let Some(parts) = THEMISIO_STATS.with(|c| c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, p)| p.clone())) {\n        return parts;\n    }\n    let st = gridcraft_engine::display::stats(sh, &d.selection.ranges);\n"),
    ("gridcraft/crates/ui-egui/src/tabs.rs",
     "    if on.sum && st.numeric > 0 {\n        parts.push(format!(\"Sum: {}\", fmt(st.sum)));\n    }\n    parts\n}",
     "    if on.sum && st.numeric > 0 {\n        parts.push(format!(\"Sum: {}\", fmt(st.sum)));\n    }\n    THEMISIO_STATS.with(|c| *c.borrow_mut() = Some((key, parts.clone())));\n    parts\n}"),
]

def main() -> int:
    failed = 0
    for rel, old, new in EDITS:
        path = ROOT / rel
        text = path.read_text(encoding="utf-8")
        # An edit that inserts keeps `old` inside `new`: test for `new` first, so running the
        # script again changes nothing.
        if new and new in text or (not new and old not in text and _applied(text, rel)):
            print(f"already  {rel}")
        elif old in text:
            path.write_text(text.replace(old, new, 1), encoding="utf-8")
            print(f"patched  {rel}")
        else:
            print(f"MISSING  {rel}: upstream changed; update scripts/patch-upstream.py", file=sys.stderr)
            failed += 1
    return 1 if failed else 0

def _applied(text, rel):
    return True  # removals: absence of the text means it is already removed

if __name__ == "__main__":
    sys.exit(main())
