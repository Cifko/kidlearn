const PLAN: &str = include_str!("../prompts/plan.txt");
const QUESTIONS: &str = include_str!("../prompts/questions.txt");
const IMAGE: &str = include_str!("../prompts/image.txt");
const SEARCH: &str = include_str!("../prompts/search.txt");

/// Audience line for the prompts. Grade 5 means high school and above.
fn audience_label(grade: Option<i64>) -> &'static str {
    match grade {
        Some(1) | Some(2) => "a young child (grade 1-2, early reader)",
        Some(3) | Some(4) => "an older child (grade 3-4)",
        Some(5) => "an older student (high school, university, or adult)",
        _ => "a general student of unknown age",
    }
}

/// Full language name for the prompts, so the model gets "Czech (cs)"
/// instead of a bare code. Unknown codes pass through untouched.
/// Keep in sync with CONTENT_LANGUAGES in app/src/languages.ts.
fn language_name(code: &str) -> String {
    let code = code.trim();
    let name = match code.to_lowercase().as_str() {
        "en" => "English",
        "sk" => "Slovak",
        "ar" => "Arabic",
        "bg" => "Bulgarian",
        "cs" => "Czech",
        "da" => "Danish",
        "de" => "German",
        "el" => "Greek",
        "es" => "Spanish",
        "fi" => "Finnish",
        "fr" => "French",
        "hi" => "Hindi",
        "hr" => "Croatian",
        "hu" => "Hungarian",
        "it" => "Italian",
        "ja" => "Japanese",
        "nl" => "Dutch",
        "no" => "Norwegian",
        "pl" => "Polish",
        "pt" => "Portuguese",
        "ro" => "Romanian",
        "ru" => "Russian",
        "sl" => "Slovenian",
        "sr" => "Serbian",
        "sv" => "Swedish",
        "tr" => "Turkish",
        "uk" => "Ukrainian",
        "zh" => "Chinese",
        _ => return code.to_string(),
    };
    format!("{name} ({code})")
}

pub fn render_plan(subject: &str, language: &str, count: i64, grade: Option<i64>) -> String {
    PLAN.replace("{subject}", subject)
        .replace("{audience}", audience_label(grade))
        .replace("{language}", &language_name(language))
        .replace("{count}", &count.to_string())
}

pub fn render_questions(
    subject: &str,
    lesson: &str,
    language: &str,
    min_questions: i64,
    max_questions: i64,
    grade: Option<i64>,
    modality: &str,
) -> String {
    let modality = match modality {
        "text_only" => "text only",
        _ => "auto",
    };
    QUESTIONS
        .replace("{subject}", subject)
        .replace("{lesson}", lesson)
        .replace("{audience}", audience_label(grade))
        .replace("{modality}", modality)
        .replace("{language}", &language_name(language))
        .replace("{min_questions}", &min_questions.to_string())
        .replace("{max_questions}", &max_questions.to_string())
}

pub fn render_image(asset_json: &str) -> String {
    IMAGE.replace("{asset_json}", asset_json)
}

pub fn render_search(query: &str, subjects_json: &str, max_results: i64) -> String {
    SEARCH
        .replace("{query}", query)
        .replace("{subjects_json}", subjects_json)
        .replace("{max_results}", &max_results.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_names_expand_known_codes() {
        assert_eq!(language_name("cs"), "Czech (cs)");
        assert_eq!(language_name("sk"), "Slovak (sk)");
        assert_eq!(language_name("xx"), "xx");
    }

    #[test]
    fn plan_prompt_fills_all_placeholders() {
        let prompt = render_plan("prvouka", "sk", 20, Some(2));
        assert!(prompt.contains("prvouka"));
        assert!(prompt.contains("MUST be written in Slovak (sk)"));
        assert!(prompt.contains("COUNT = 20"));
        assert!(prompt.contains("AUDIENCE = a young child (grade 1-2, early reader)"));
        assert!(!prompt.contains("{subject}"));
        assert!(!prompt.contains("{audience}"));
        assert!(!prompt.contains("{language}"));
        assert!(!prompt.contains("{count}"));

        let prompt = render_plan("analysis", "en", 20, Some(5));
        assert!(prompt.contains("older student (high school, university, or adult)"));
    }

    #[test]
    fn questions_prompt_fills_all_placeholders() {
        let prompt = render_questions("math", "addition", "en", 5, 10, Some(2), "auto");
        assert!(prompt.contains("addition"));
        assert!(prompt.contains("MIN_QUESTIONS = 5"));
        assert!(prompt.contains("MAX_QUESTIONS = 10"));
        assert!(prompt.contains("AUDIENCE = a young child (grade 1-2, early reader)"));
        assert!(prompt.contains("MODALITY = auto"));
        assert!(!prompt.contains("{lesson}"));
        assert!(!prompt.contains("{audience}"));
        assert!(!prompt.contains("{modality}"));
        assert!(!prompt.contains("{min_questions}"));
        assert!(!prompt.contains("{max_questions}"));

        let prompt = render_questions("math", "addition", "en", 5, 10, None, "text_only");
        assert!(prompt.contains("AUDIENCE = a general student of unknown age"));
        assert!(prompt.contains("MODALITY = text only"));
    }

    #[test]
    fn alignment_rules_survive_prompt_edits() {
        let plan = render_plan("math", "en", 20, None);
        assert!(plan.contains("tangential"));
        assert!(plan.contains("checkable skill"));
        let questions = render_questions("math", "addition", "en", 5, 10, None, "auto");
        assert!(questions.contains("differ ONLY in the tested attribute"));
        assert!(questions.contains("FORBIDDEN"));
        assert!(questions.contains("\"focus\""));
        assert!(questions.contains("Which picture shows 4 apples?"));
        assert!(!questions.contains("How many apples?"));
        assert!(questions.contains("multiple_choice_text"));
        assert!(questions.contains("ONE QUESTION, ONE KIND"));
        assert!(questions.contains("impossible to test in words"));
        assert!(questions.contains("figure_asset_id"));
        assert!(questions.contains("\\("));
        assert!(questions.contains("\"plot\""));
    }

    #[test]
    fn image_prompt_embeds_asset_spec() {
        let prompt = render_image(r#"{"id":"red_ball"}"#);
        assert!(prompt.contains(r#"{"id":"red_ball"}"#));
        assert!(prompt.contains("children's school books"));
        assert!(!prompt.contains("{asset_json}"));
    }

    #[test]
    fn search_prompt_fills_all_placeholders() {
        let prompt = render_search("prvouka", r#"[{"id":1}]"#, 10);
        assert!(prompt.contains("prvouka"));
        assert!(prompt.contains(r#"[{"id":1}]"#));
        assert!(prompt.contains("at most 10 ids"));
        assert!(!prompt.contains("{query}"));
        assert!(!prompt.contains("{subjects_json}"));
        assert!(!prompt.contains("{max_results}"));
    }
}
