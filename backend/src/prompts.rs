const PLAN: &str = include_str!("../prompts/plan.txt");
const QUESTIONS: &str = include_str!("../prompts/questions.txt");
const SVG: &str = include_str!("../prompts/svg.txt");

pub fn render_plan(subject: &str, language: &str, count: i64) -> String {
    PLAN.replace("{subject}", subject)
        .replace("{language}", language)
        .replace("{count}", &count.to_string())
}

pub fn render_questions(
    subject: &str,
    subtopic: &str,
    language: &str,
    min_questions: i64,
    max_questions: i64,
) -> String {
    QUESTIONS
        .replace("{subject}", subject)
        .replace("{subtopic}", subtopic)
        .replace("{language}", language)
        .replace("{min_questions}", &min_questions.to_string())
        .replace("{max_questions}", &max_questions.to_string())
}

pub fn render_svg(asset_json: &str) -> String {
    SVG.replace("{asset_json}", asset_json)
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
        assert!(!prompt.contains("{subtopic}"));
        assert!(!prompt.contains("{min_questions}"));
        assert!(!prompt.contains("{max_questions}"));
    }

    #[test]
    fn svg_prompt_embeds_asset_spec() {
        let prompt = render_svg(r#"{"id":"red_ball"}"#);
        assert!(prompt.contains(r#"{"id":"red_ball"}"#));
        assert!(!prompt.contains("{asset_json}"));
    }
}
