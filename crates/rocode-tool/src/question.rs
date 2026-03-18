use async_trait::async_trait;
use serde::Deserialize;
use std::io::{self, BufRead, Write};

use crate::{Tool, ToolContext, ToolError, ToolResult};

pub struct QuestionTool;

impl QuestionTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for QuestionTool {
    fn id(&self) -> &str {
        "question"
    }

    fn description(&self) -> &str {
        "Ask the user clarifying questions during execution. Use to gather preferences, clarify ambiguous requests, or get decisions on implementation choices."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask"
                            },
                            "header": {
                                "type": "string",
                                "description": "Short label for the question (max 30 chars)"
                            },
                            "multiple": {
                                "type": "boolean",
                                "default": false,
                                "description": "Allow selecting multiple options"
                            },
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {"type": "string"},
                                        "description": {"type": "string"}
                                    },
                                    "required": ["label"]
                                },
                                "description": "Available choices for the user"
                            }
                        },
                        "required": ["question"]
                    }
                }
            },
            "required": ["questions"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input = parse_question_input(args)?;

        let questions = input
            .questions
            .iter()
            .map(|q| crate::QuestionDef {
                question: q.question.clone(),
                header: q.header.clone(),
                options: q
                    .options
                    .iter()
                    .map(|opt| crate::QuestionOption {
                        label: opt.label.clone(),
                        description: opt.description.clone(),
                    })
                    .collect(),
                multiple: q.multiple,
            })
            .collect::<Vec<_>>();

        let answers_by_question = match ctx.question(questions.clone()).await {
            Ok(answers) => answers,
            Err(ToolError::ExecutionError(msg))
                if msg.contains("Question callback not configured") =>
            {
                let mut manual_answers = Vec::with_capacity(questions.len());
                for q in &questions {
                    manual_answers.push(ask_question(q)?);
                }
                manual_answers
            }
            Err(e) => return Err(e),
        };

        let mut all_answers: Vec<String> = Vec::new();
        let mut display_fields = Vec::new();
        for (idx, q) in questions.iter().enumerate() {
            let answers = answers_by_question.get(idx).cloned().unwrap_or_default();
            let answer_text = answers.join(", ");
            all_answers.extend(answers);
            display_fields.push(serde_json::json!({
                "key": q.question,
                "value": answer_text,
            }));
        }

        let response = rocode_types::QuestionToolResult {
            answers: all_answers.clone(),
        };

        let output = serde_json::to_string_pretty(&response)
            .unwrap_or_else(|_| format!("{:?}", response.answers));

        // Build display hints for TUI rendering
        let mut metadata = std::collections::HashMap::new();

        metadata.insert(
            "display.fields".to_string(),
            serde_json::Value::Array(display_fields),
        );

        // display.summary
        let summary = if questions.len() == 1 {
            "1 question answered".to_string()
        } else {
            format!("{} questions answered", questions.len())
        };
        metadata.insert(
            "display.summary".to_string(),
            serde_json::Value::String(summary),
        );

        Ok(ToolResult {
            title: "User response received".to_string(),
            output,
            metadata,
            truncated: false,
        })
    }
}

fn parse_question_input(args: serde_json::Value) -> Result<rocode_types::QuestionToolInput, ToolError> {
    #[derive(Debug, Deserialize, Default)]
    struct RawQuestionInputWire {
        #[serde(default)]
        questions: Option<serde_json::Value>,
    }

    let raw: RawQuestionInputWire = serde_json::from_value(args.clone()).unwrap_or_default();
    let mut input = rocode_types::QuestionToolInput::from_value(&args);
    input.questions.retain(|q| !q.question.trim().is_empty());
    if !input.questions.is_empty() {
        return Ok(input);
    }

    match raw.questions {
        None | Some(serde_json::Value::Null) => Err(ToolError::InvalidArguments(
            "questions is required".to_string(),
        )),
        Some(serde_json::Value::Array(array)) if array.is_empty() => Err(ToolError::InvalidArguments(
            "questions must not be empty".to_string(),
        )),
        Some(serde_json::Value::Bool(_)) | Some(serde_json::Value::Number(_)) => {
            Err(ToolError::InvalidArguments(
                "questions must be an array/object or a JSON string representing them".to_string(),
            ))
        }
        _ => Err(ToolError::InvalidArguments(
            "questions must contain at least one valid question".to_string(),
        )),
    }
}

fn ask_question(q: &crate::QuestionDef) -> Result<Vec<String>, ToolError> {
    println!();

    if let Some(ref header) = q.header {
        println!("┌─ {} ─────────────────", header);
    } else {
        println!("┌─ Question ─────────────────");
    }
    println!("│");
    println!("│ {}", q.question);
    println!("│");

    if q.options.is_empty() {
        println!("└─ Type your answer: ");
        print!("> ");
        io::stdout()
            .flush()
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        let stdin = io::stdin();
        let mut answer = String::new();
        stdin
            .lock()
            .read_line(&mut answer)
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        return Ok(vec![answer.trim().to_string()]);
    }

    println!("│ Options:");
    for (i, opt) in q.options.iter().enumerate() {
        let num = i + 1;
        if let Some(ref desc) = opt.description {
            println!("│   {}. {} - {}", num, opt.label, desc);
        } else {
            println!("│   {}. {}", num, opt.label);
        }
    }
    println!("│");

    if q.multiple {
        println!("└─ Enter choices (comma-separated, e.g., 1,3): ");
    } else {
        println!("└─ Enter your choice: ");
    }

    print!("> ");
    io::stdout()
        .flush()
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    let stdin = io::stdin();
    let mut input = String::new();
    stdin
        .lock()
        .read_line(&mut input)
        .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

    let input = input.trim();
    let answers: Vec<String> = input
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }

            if let Ok(num) = s.parse::<usize>() {
                if num > 0 && num <= q.options.len() {
                    return Some(q.options[num - 1].label.clone());
                }
            }

            Some(s.to_string())
        })
        .collect();

    if answers.is_empty() && !q.options.is_empty() {
        return Ok(vec![q.options[0].label.clone()]);
    }

    Ok(answers)
}

impl Default for QuestionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_question_input, QuestionTool};
    use crate::{Tool, ToolContext};

    #[test]
    fn parse_question_input_accepts_questions_array() {
        let input = parse_question_input(serde_json::json!({
            "questions": [
                { "question": "继续吗？", "options": [{ "label": "是" }, { "label": "否" }] }
            ]
        }))
        .expect("questions array should parse");
        assert_eq!(input.questions.len(), 1);
        assert_eq!(input.questions[0].question, "继续吗？");
    }

    #[test]
    fn parse_question_input_accepts_stringified_questions_array() {
        let raw = r#"[{"question":"继续吗？","options":[{"label":"是"},{"label":"否"}]}]"#;
        let input = parse_question_input(serde_json::json!({
            "questions": raw
        }))
        .expect("stringified questions should parse");
        assert_eq!(input.questions.len(), 1);
        assert_eq!(input.questions[0].question, "继续吗？");
    }

    #[test]
    fn parse_question_input_rejects_non_collection_questions() {
        let err = parse_question_input(serde_json::json!({ "questions": 1 }))
            .expect_err("numeric questions should fail");
        let msg = err.to_string();
        assert!(msg.contains("questions must be an array/object"));
    }

    #[test]
    fn parse_question_input_still_parses_direct_shape() {
        let parsed: rocode_types::QuestionToolInput = parse_question_input(serde_json::json!({
            "questions": [{ "question": "Q1" }]
        }))
        .expect("direct shape should parse");
        assert_eq!(parsed.questions.len(), 1);
    }

    #[tokio::test]
    async fn question_tool_uses_ctx_callback_when_available() {
        let tool = QuestionTool::new();
        let ctx = ToolContext::new("s".to_string(), "m".to_string(), ".".to_string())
            .with_ask_question(|questions| async move {
                assert_eq!(questions.len(), 1);
                Ok(vec![vec!["确认计划".to_string()]])
            });

        let result = tool
            .execute(
                serde_json::json!({
                    "questions": [
                        {
                            "question": "计划已生成并通过自我审查。您希望如何继续？",
                            "options": [{ "label": "确认计划" }, { "label": "需要修改" }]
                        }
                    ]
                }),
                ctx,
            )
            .await
            .expect("question execution should succeed");

        assert!(result.output.contains("确认计划"));
        let display = rocode_types::DisplayOverrideMetadata::from_map(&result.metadata);
        assert_eq!(display.summary.as_deref(), Some("1 question answered"));
    }
}
