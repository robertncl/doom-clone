//! On-screen text in English, French, German, Japanese and Chinese.
//!
//! Every string the game draws comes from [`tr`] (or [`trf`] when it carries
//! a number), so adding a language is one more column in [`table`]. Latin
//! text is upper case to match the bitmap font; French and German accents are
//! drawn as marks over the base letter (see `hud.rs`). Japanese and Chinese
//! use 16×16 glyphs from GNU Unifont, embedded in `cjk_glyphs.rs` — that file
//! is generated from this one by `tools/gen_cjk_glyphs.py`, so after adding a
//! CJK string, rerun the script (a test fails until you do).

pub const LANG_EN: usize = 0;
pub const LANG_FR: usize = 1;
pub const LANG_DE: usize = 2;
pub const LANG_JA: usize = 3;
pub const LANG_ZH: usize = 4;
pub const LANG_COUNT: usize = 5;

/// ISO 639-1 codes, as `--lang` and the locale variables spell them.
pub const LANG_CODES: [&str; LANG_COUNT] = ["en", "fr", "de", "ja", "zh"];
/// Each language's name in that language, for the selector.
pub const LANG_NAMES: [&str; LANG_COUNT] = ["ENGLISH", "FRANÇAIS", "DEUTSCH", "日本語", "中文"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    Health,
    Ammo,
    Weapon,
    Pistol,
    Shotgun,
    Rifle,
    Level,
    Kills,
    Score,
    Victory,
    YouDied,
    SkillLine,
    NewHighScore,
    HighScores,
    PressRestart,
    Controls,
    Move,
    Strafe,
    Turn,
    Shoot,
    Restart,
    Quit,
    PressStart,
    Difficulty,
    ChangeHint,
    LevelClear,
    Language,
    KeyArrows,
    KeySpace,
}

/// Every message, for tests that walk the whole table.
pub const ALL_MSGS: [Msg; 29] = [
    Msg::Health,
    Msg::Ammo,
    Msg::Weapon,
    Msg::Pistol,
    Msg::Shotgun,
    Msg::Rifle,
    Msg::Level,
    Msg::Kills,
    Msg::Score,
    Msg::Victory,
    Msg::YouDied,
    Msg::SkillLine,
    Msg::NewHighScore,
    Msg::HighScores,
    Msg::PressRestart,
    Msg::Controls,
    Msg::Move,
    Msg::Strafe,
    Msg::Turn,
    Msg::Shoot,
    Msg::Restart,
    Msg::Quit,
    Msg::PressStart,
    Msg::Difficulty,
    Msg::ChangeHint,
    Msg::LevelClear,
    Msg::Language,
    Msg::KeyArrows,
    Msg::KeySpace,
];

/// `[en, fr, de, ja, zh]` for one message. `{}` marks where a number goes.
fn table(m: Msg) -> [&'static str; LANG_COUNT] {
    match m {
        Msg::Health => ["HEALTH", "SANTÉ", "GESUNDHEIT", "体力", "生命"],
        Msg::Ammo => ["AMMO", "MUNITIONS", "MUNITION", "弾薬", "弹药"],
        Msg::Weapon => ["WEAPON", "ARME", "WAFFE", "武器", "武器"],
        Msg::Pistol => ["PISTOL", "PISTOLET", "PISTOLE", "ピストル", "手枪"],
        Msg::Shotgun => ["SHOTGUN", "FUSIL À POMPE", "SCHROTFLINTE", "ショットガン", "霰弹枪"],
        Msg::Rifle => ["RIFLE", "FUSIL", "GEWEHR", "ライフル", "步枪"],
        Msg::Level => ["LEVEL {}", "NIVEAU {}", "LEVEL {}", "レベル {}", "第{}关"],
        Msg::Kills => ["KILLS {}/{}", "ÉLIMINÉS {}/{}", "ABSCHÜSSE {}/{}", "撃破 {}/{}", "击杀 {}/{}"],
        Msg::Score => ["SCORE {}", "SCORE {}", "PUNKTE {}", "スコア {}", "得分 {}"],
        Msg::Victory => ["VICTORY", "VICTOIRE", "SIEG", "勝利", "胜利"],
        Msg::YouDied => ["YOU DIED", "VOUS ÊTES MORT", "DU BIST TOT", "死亡", "你死了"],
        Msg::SkillLine => ["SKILL {}", "DIFFICULTÉ {}", "SCHWIERIGKEIT {}", "難易度 {}", "难度 {}"],
        Msg::NewHighScore => [
            "NEW HIGH SCORE RANK {}",
            "NOUVEAU RECORD RANG {}",
            "NEUER REKORD PLATZ {}",
            "新記録 {}位",
            "新纪录 第{}名",
        ],
        Msg::HighScores => ["HIGH SCORES", "MEILLEURS SCORES", "BESTENLISTE", "ハイスコア", "最高分"],
        Msg::PressRestart => [
            "PRESS R TO RESTART",
            "R POUR REJOUER",
            "R FÜR NEUSTART",
            "Rキーで再開",
            "按R键重新开始",
        ],
        Msg::Controls => ["CONTROLS", "COMMANDES", "STEUERUNG", "操作", "操作"],
        Msg::Move => ["MOVE", "AVANCER", "BEWEGEN", "移動", "移动"],
        Msg::Strafe => ["STRAFE", "PAS LATÉRAL", "SEITWÄRTS", "横移動", "平移"],
        Msg::Turn => ["TURN", "TOURNER", "DREHEN", "旋回", "转向"],
        Msg::Shoot => ["SHOOT", "TIRER", "SCHIESSEN", "射撃", "射击"],
        Msg::Restart => ["RESTART", "REJOUER", "NEUSTART", "再開", "重新开始"],
        Msg::Quit => ["QUIT", "QUITTER", "BEENDEN", "終了", "退出"],
        Msg::PressStart => [
            "PRESS SPACE TO START",
            "ESPACE POUR COMMENCER",
            "LEERTASTE ZUM STARTEN",
            "スペースキーで開始",
            "按空格键开始",
        ],
        Msg::Difficulty => ["DIFFICULTY", "DIFFICULTÉ", "SCHWIERIGKEIT", "難易度", "难度"],
        Msg::ChangeHint => [
            "LEFT RIGHT TO CHANGE",
            "GAUCHE DROITE POUR CHANGER",
            "LINKS RECHTS ZUM ÄNDERN",
            "左右キーで変更",
            "按左右键调整",
        ],
        Msg::LevelClear => ["LEVEL CLEAR", "NIVEAU TERMINÉ", "LEVEL GESCHAFFT", "レベルクリア", "关卡完成"],
        Msg::Language => ["LANGUAGE", "LANGUE", "SPRACHE", "言語", "语言"],
        Msg::KeyArrows => ["ARROWS", "FLÈCHES", "PFEILE", "矢印キー", "方向键"],
        Msg::KeySpace => ["SPACE", "ESPACE", "LEERTASTE", "スペース", "空格"],
    }
}

/// Difficulty names per language, indexed `[lang][SKILL_*]`.
const SKILLS: [[&str; 5]; LANG_COUNT] = [
    ["BABY", "EASY", "NORMAL", "HARD", "NIGHTMARE"],
    ["BÉBÉ", "FACILE", "NORMAL", "DIFFICILE", "CAUCHEMAR"],
    ["BABY", "LEICHT", "NORMAL", "SCHWER", "ALBTRAUM"],
    ["超簡単", "簡単", "普通", "難しい", "悪夢"],
    ["极易", "简单", "普通", "困难", "噩梦"],
];

/// `m` in language `lang` (out-of-range languages fall back to the last).
pub fn tr(lang: usize, m: Msg) -> &'static str {
    table(m)[lang.min(LANG_COUNT - 1)]
}

/// `m` with each `{}` replaced by the next of `args`.
pub fn trf(lang: usize, m: Msg, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut rest = tr(lang, m);
    for a in args {
        match rest.find("{}") {
            Some(i) => {
                out.push_str(&rest[..i]);
                out.push_str(&a.to_string());
                rest = &rest[i + 2..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// Name of difficulty `skill` in `lang`.
pub fn skill_name(lang: usize, skill: usize) -> &'static str {
    SKILLS[lang.min(LANG_COUNT - 1)][skill.min(4)]
}

/// Every string in the tables, for tests and the glyph generator's check.
pub fn all_strings() -> Vec<&'static str> {
    let mut v: Vec<&str> = ALL_MSGS.iter().flat_map(|&m| table(m)).collect();
    v.extend(SKILLS.iter().flatten());
    v.extend(LANG_NAMES);
    v
}

/// Read a language from a flag value or locale string: `fr`, `de_DE.UTF-8`,
/// `ja-JP`, `zh_CN`, or a name such as `french`/`français`. `C` and `POSIX`
/// mean English.
pub fn parse_lang(s: &str) -> Option<usize> {
    let s = s.trim().to_lowercase();
    if s == "c" || s == "posix" {
        return Some(LANG_EN);
    }
    let code: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    if let Some(i) = LANG_CODES.iter().position(|&c| c == code) {
        return Some(i);
    }
    let names = [
        ("english", LANG_EN),
        ("french", LANG_FR),
        ("français", LANG_FR),
        ("francais", LANG_FR),
        ("german", LANG_DE),
        ("deutsch", LANG_DE),
        ("japanese", LANG_JA),
        ("日本語", LANG_JA),
        ("chinese", LANG_ZH),
        ("中文", LANG_ZH),
    ];
    names.iter().find(|(n, _)| *n == s).map(|&(_, l)| l)
}

/// Language from the environment: `DOOM_LANG` first, then the usual locale
/// variables in POSIX priority order. `None` if none is set or recognised.
pub fn lang_from_env() -> Option<usize> {
    ["DOOM_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .and_then(|v| parse_lang(&v))
}
