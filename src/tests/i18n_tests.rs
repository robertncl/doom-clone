//! Languages: the translation table, the glyphs behind it, and switching.

use super::*;
use crate::cjk_glyphs::{JA, ZH};
use crate::constants::*;
use crate::hud::{can_draw, has_wide, is_wide, text_width};
use crate::i18n::*;

// ---- the table ----

#[test]
fn every_message_exists_in_every_language() {
    for m in ALL_MSGS {
        let en = tr(LANG_EN, m);
        for lang in 0..LANG_COUNT {
            let s = tr(lang, m);
            assert!(!s.trim().is_empty(), "{m:?} is blank in {}", LANG_CODES[lang]);
            assert_eq!(s.matches("{}").count(), en.matches("{}").count(), "{m:?} in {} takes a different number of values", LANG_CODES[lang]);
        }
    }
    for lang in 0..LANG_COUNT {
        for skill in 0..5 {
            assert!(!skill_name(lang, skill).is_empty());
        }
    }
}

#[test]
fn every_character_can_be_drawn_in_its_own_language() {
    for lang in 0..LANG_COUNT {
        let mut strings: Vec<&str> = ALL_MSGS.iter().map(|&m| tr(lang, m)).collect();
        strings.extend((0..5).map(|s| skill_name(lang, s)));
        strings.push(LANG_NAMES[lang]);
        for s in strings {
            for c in s.chars().filter(|&c| !"{}".contains(c)) {
                assert!(can_draw(c, lang), "{c:?} in {s:?} ({}) has no glyph", LANG_CODES[lang]);
            }
        }
    }
}

#[test]
fn the_glyph_table_matches_the_translations() {
    // Stale-table guard: rerun tools/gen_cjk_glyphs.py after editing CJK text.
    let mut wanted: Vec<char> = all_strings().iter().flat_map(|s| s.chars()).filter(|&c| is_wide(c)).collect();
    wanted.sort_unstable();
    wanted.dedup();
    for (name, table) in [("JA", &JA[..]), ("ZH", &ZH[..])] {
        let have: Vec<char> = table.iter().map(|e| e.0).collect();
        assert!(have.windows(2).all(|w| w[0] < w[1]), "{name} must be sorted for binary search");
        for c in &wanted {
            assert!(have.contains(c), "{name} is missing {c:?}: rerun tools/gen_cjk_glyphs.py");
        }
        assert!(table.iter().all(|e| e.1.iter().any(|&r| r != 0)), "{name} has a blank glyph");
    }
}

#[test]
fn japanese_and_chinese_draw_some_characters_differently() {
    // The point of carrying two tables: shared ideographs differ in form.
    let differ = JA.iter().zip(ZH.iter()).filter(|(j, z)| j.0 == z.0 && j.1 != z.1).count();
    assert!(differ > 0);
}

#[test]
fn values_are_filled_in_order() {
    assert_eq!(trf(LANG_EN, Msg::Kills, &[&3, &7]), "KILLS 3/7");
    assert_eq!(trf(LANG_ZH, Msg::Level, &[&4]), "第4关");
    assert_eq!(trf(LANG_JA, Msg::NewHighScore, &[&2]), "新記録 2位");
    assert_eq!(trf(LANG_EN, Msg::Health, &[&1]), "HEALTH", "extra values are ignored");
    assert_eq!(trf(LANG_EN, Msg::Score, &[]), "SCORE {}", "missing values leave the marker");
}

#[test]
fn an_out_of_range_language_falls_back_to_the_last() {
    assert_eq!(tr(99, Msg::Victory), tr(LANG_COUNT - 1, Msg::Victory));
    assert_eq!(skill_name(99, 99), skill_name(LANG_COUNT - 1, 4));
}

#[test]
fn languages_parse_from_codes_locales_and_names() {
    for (s, want) in [
        ("fr", LANG_FR),
        ("de_DE.UTF-8", LANG_DE),
        ("ja-JP", LANG_JA),
        ("zh_CN.GB2312", LANG_ZH),
        ("en_US.UTF-8", LANG_EN),
        ("C", LANG_EN),
        ("POSIX", LANG_EN),
        (" Français ", LANG_FR),
        ("GERMAN", LANG_DE),
        ("日本語", LANG_JA),
        ("中文", LANG_ZH),
    ] {
        assert_eq!(parse_lang(s), Some(want), "{s:?}");
    }
    for bad in ["", "xx", "klingon", "e"] {
        assert_eq!(parse_lang(bad), None, "{bad:?}");
    }
}

// ---- layout ----

#[test]
fn text_width_counts_cjk_as_two_cells() {
    assert_eq!(text_width("中文"), 32);
    assert_eq!(text_width("É"), 8, "an accented capital is one cell");
    assert_eq!(text_width("R键"), 24);
    assert!(has_wide("按R键") && !has_wide("FRANÇAIS"));
}

#[test]
fn hud_labels_fit_their_slots_in_every_language() {
    for lang in 0..LANG_COUNT {
        // Health sits left of the first divider (x 20..110); ammo runs from
        // 80px before the right edge.
        assert!(text_width(tr(lang, Msg::Health)) <= 88, "{} health label", LANG_CODES[lang]);
        assert!(text_width(tr(lang, Msg::Ammo)) <= 76, "{} ammo label", LANG_CODES[lang]);
        // Intro and game-over lines stay inside the 500px panel.
        for m in [Msg::PressStart, Msg::ChangeHint, Msg::PressRestart, Msg::HighScores] {
            assert!(text_width(tr(lang, m)) <= 460, "{} {m:?}", LANG_CODES[lang]);
        }
        assert!(text_width(&trf(lang, Msg::NewHighScore, &[&5])) <= 460);
        // Key names fit the 160px column before the action names.
        for m in [Msg::KeyArrows, Msg::KeySpace] {
            assert!(text_width(tr(lang, m)) < 150, "{} {m:?}", LANG_CODES[lang]);
        }
    }
}

// ---- drawing ----

fn ink(g: &Game) -> usize {
    g.pixels.iter().filter(|&&p| p != 0).count()
}

#[test]
fn accents_add_a_mark_to_the_base_letter() {
    let mut g = open_room();
    g.pixels.fill(0);
    g.draw_text("E", 40, 40, 0xFFFFFF);
    let plain = ink(&g);
    for accented in ["É", "È", "Ê", "Ë", "Ç"] {
        let mut g = open_room();
        g.pixels.fill(0);
        g.draw_text(accented, 40, 40, 0xFFFFFF);
        assert!(ink(&g) > if accented == "Ç" { 0 } else { plain }, "{accented} drew no mark");
    }
}

#[test]
fn cjk_text_draws_glyphs() {
    for lang in [LANG_JA, LANG_ZH] {
        let mut g = open_room();
        g.lang = lang;
        g.pixels.fill(0);
        g.draw_text(tr(lang, Msg::Victory), 100, 100, 0xFFFFFF);
        assert!(ink(&g) > 40, "{} victory text is empty", LANG_CODES[lang]);
    }
    let mut g = open_room();
    g.pixels.fill(0);
    g.draw_text("\u{0416}", 10, 10, 0xFFFFFF); // Cyrillic: not in the font
    assert_eq!(ink(&g), 0, "unknown characters are skipped, not drawn as garbage");
}

#[test]
fn every_screen_renders_in_every_language() {
    let frame = |lang: usize, intro: bool, dead: bool| {
        let mut g = open_room();
        g.lang = lang;
        g.show_intro = intro;
        if dead {
            g.player.health = 0;
            g.score_saved = true;
            g.final_rank = 1;
        }
        g.render_frame();
        g.pixels.clone()
    };
    for (intro, dead) in [(true, false), (false, false), (false, true)] {
        let en = frame(LANG_EN, intro, dead);
        for lang in 1..LANG_COUNT {
            assert_ne!(frame(lang, intro, dead), en, "{} looks like English (intro {intro}, dead {dead})", LANG_CODES[lang]);
        }
    }
}

// ---- switching ----

#[test]
fn l_cycles_the_language_without_starting_the_game() {
    let mut g = open_room();
    g.show_intro = true;
    for want in [LANG_FR, LANG_DE, LANG_JA, LANG_ZH, LANG_EN] {
        g.key_edge[K_LANG] = true;
        g.update_game(1.0 / 60.0);
        assert_eq!(g.lang, want);
        assert!(g.show_intro, "changing language must not dismiss the title screen");
        assert!(!g.key_edge[K_LANG], "the press is consumed");
    }
}

#[test]
fn the_language_can_change_mid_game_and_survives_a_restart() {
    let mut g = open_room();
    g.key_edge[K_LANG] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.lang, LANG_FR);
    g.reset_game();
    assert_eq!(g.lang, LANG_FR);
}
