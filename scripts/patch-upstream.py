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

]

def main() -> int:
    failed = 0
    for rel, old, new in EDITS:
        path = ROOT / rel
        text = path.read_text(encoding="utf-8")
        if old in text:
            path.write_text(text.replace(old, new, 1), encoding="utf-8")
            print(f"patched  {rel}")
        elif new and new in text or (not new and old not in text and _applied(text, rel)):
            print(f"already  {rel}")
        else:
            print(f"MISSING  {rel}: upstream changed; update scripts/patch-upstream.py", file=sys.stderr)
            failed += 1
    return 1 if failed else 0

def _applied(text, rel):
    return True  # removals: absence of the text means it is already removed

if __name__ == "__main__":
    sys.exit(main())
