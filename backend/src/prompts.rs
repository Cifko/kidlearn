const PLAN: &str = include_str!("../prompts/plan.txt");
const QUESTIONS: &str = include_str!("../prompts/questions.txt");
const SVG: &str = include_str!("../prompts/svg.txt");
const SEARCH: &str = include_str!("../prompts/search.txt");

pub fn render_plan(subject: &str, language: &str, count: i64) -> String {
    PLAN.replace("{subject}", subject)
        .replace("{language}", language)
        .replace("{count}", &count.to_string())
}

pub fn render_questions(
    subject: &str,
    lesson: &str,
    language: &str,
    min_questions: i64,
    max_questions: i64,
) -> String {
    QUESTIONS
        .replace("{subject}", subject)
        .replace("{lesson}", lesson)
        .replace("{language}", language)
        .replace("{min_questions}", &min_questions.to_string())
        .replace("{max_questions}", &max_questions.to_string())
}

pub fn render_svg(asset_json: &str) -> String {
    SVG.replace("{asset_json}", asset_json)
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
    fn plan_prompt_fills_all_placeholders() {
        let prompt = render_plan("prvouka", "sk", 20);
        assert!(prompt.contains("prvouka"));
        assert!(prompt.contains("MUST be written in sk"));
        assert!(prompt.contains("COUNT = 20"));
        assert!(!prompt.contains("{subject}"));
        assert!(!prompt.contains("{language}"));
        assert!(!prompt.contains("{count}"));
    }

    #[test]
    fn questions_prompt_fills_all_placeholders() {
        let prompt = render_questions("math", "addition", "en", 5, 10);
        assert!(prompt.contains("addition"));
        assert!(prompt.contains("MIN_QUESTIONS = 5"));
        assert!(prompt.contains("MAX_QUESTIONS = 10"));
        assert!(!prompt.contains("{lesson}"));
        assert!(!prompt.contains("{min_questions}"));
        assert!(!prompt.contains("{max_questions}"));
    }

    #[test]
    fn alignment_rules_survive_prompt_edits() {
        let plan = render_plan("math", "en", 20);
        assert!(plan.contains("tangential"));
        assert!(plan.contains("checkable skill"));
        let questions = render_questions("math", "addition", "en", 5, 10);
        assert!(questions.contains("differ ONLY in the tested attribute"));
        assert!(questions.contains("FORBIDDEN"));
        assert!(questions.contains("\"focus\""));
        assert!(questions.contains("Which picture shows 4 apples?"));
        assert!(!questions.contains("How many apples?"));
    }

    #[test]
    fn svg_prompt_embeds_asset_spec() {
        let prompt = render_svg(r#"{"id":"red_ball"}"#);
        assert!(prompt.contains(r#"{"id":"red_ball"}"#));
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
