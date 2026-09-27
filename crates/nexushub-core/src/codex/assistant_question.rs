//! Conservative, local classification of a final reply requesting user feedback.

pub fn assistant_feedback_request(text: &str) -> Option<String> {
    let mut text = text.replace("&lt;", "<").replace("&gt;", ">");
    for tag in [
        "oai-mem-citation",
        "citation_entries",
        "rollout_ids",
        "memory_citation",
    ] {
        loop {
            let lower = text.to_ascii_lowercase();
            let Some(start) = lower.find(&format!("<{tag}>")) else {
                break;
            };
            let end = lower[start..]
                .find(&format!("</{tag}>"))
                .map(|offset| start + offset + tag.len() + 3)
                .unwrap_or(text.len());
            text.replace_range(start..end, "");
        }
    }
    let mut visible = Vec::new();
    let mut fence: Option<char> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = trimmed.chars().next().unwrap();
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
            visible.push(String::new());
            continue;
        }
        if fence.is_some()
            || trimmed.starts_with('>')
            || line.starts_with("    ")
            || line.starts_with('\t')
            || trimmed
                .trim_start_matches(['"', '\''])
                .starts_with("memory_citation")
        {
            continue;
        }
        let mut inline = String::new();
        let mut code = false;
        for part in line.split_inclusive('`') {
            if !code {
                inline.push_str(part.trim_end_matches('`'));
            }
            if part.ends_with('`') {
                code = !code;
            }
        }
        visible.push(inline);
    }
    let visible = visible.join("\n");
    let paragraph = visible
        .rsplit("\n\n")
        .map(str::trim)
        .find(|p| !p.is_empty())?;
    let mut unquoted = String::new();
    let mut quote_end = None;
    for ch in paragraph.chars() {
        if quote_end == Some(ch) {
            quote_end = None;
            continue;
        }
        if quote_end.is_some() {
            continue;
        }
        match ch {
            '“' => quote_end = Some('”'),
            '「' => quote_end = Some('」'),
            '"' => quote_end = Some('"'),
            _ => unquoted.push(ch),
        }
    }
    let lower = unquoted.to_lowercase();
    if [
        "例如",
        "示例",
        "引用",
        "用户说",
        "提示词",
        "example:",
        "for example",
        "the user said",
        "不需要你确认",
        "无需确认",
        "不用确认",
        "不必回复",
        "no need to reply",
        "no confirmation needed",
        "如果需要我可以",
        "如果你需要，我可以",
        "if you'd like, i can",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return None;
    }
    let explicit = [
        "请确认",
        "请你确认",
        "请帮我确认",
        "请核对",
        "请回复",
        "请告诉我",
        "请提供",
        "请补充",
        "请告知",
        "请反馈",
        "请选择",
        "请选一个",
        "麻烦确认",
        "麻烦你确认",
        "麻烦你提供",
        "操作后反馈",
        "告诉我结果",
        "告诉我是否",
        "告诉我现在",
        "回复确认",
        "等待你的",
        "等你确认",
        "需要你确认",
        "需要你提供",
        "please confirm",
        "please check",
        "please choose",
        "please select",
        "please provide",
        "please reply",
        "please test",
        "please try",
        "let me know",
        "could you confirm",
        "could you provide",
        "could you test",
        "can you confirm",
        "can you check",
        "can you test",
        "which option do you prefer",
        "would you like me to",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let direct_question = [
        "现在表现如何",
        "现在是否正常",
        "是否收到",
        "你收到了吗",
        "你希望选择",
        "你想选择",
        "你选哪个",
        "你是否同意",
        "是否继续",
        "是否执行",
        "可以继续吗",
        "可以发布吗",
        "需要我继续吗",
        "does it work now",
        "did you receive",
        "is it working now",
        "shall i proceed",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if !explicit && !direct_question {
        return None;
    }
    // An instruction to the user is not inferred from an assistant's own next step.
    if ["我会", "我将", "我正在", "i will", "i'll", "i am going to"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
        && !lower.contains('？')
        && !lower.contains('?')
    {
        return None;
    }
    Some(paragraph.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifies_explicit_requests_only() {
        for text in [
            "修复已安装。请试一次左键和右键；现在表现如何？",
            "请确认是否采用方案 A。",
            "请选择需要保留的配置",
            "请提供复现步骤",
            "Please test the new build and let me know the result.",
            "Did you receive the notification?",
        ] {
            assert!(assistant_feedback_request(text).is_some(), "{text}");
        }
        for text in [
            "为什么失败？原因是文件不存在。",
            "问题是在哪里？我们已经找到了答案。",
            "已完成。",
            "我会检查并反馈结果。",
            "示例：请确认是否继续？",
            "> 请确认是否继续？",
            "```text\n请确认是否继续？\n```",
            "`请确认是否继续？`",
            "如果你需要，我可以继续完善。",
            "需要多久？通常两分钟。",
            "不需要你确认，已自动完成。",
            "程序会显示“请确认是否继续”。",
            "请把配置保存到文件。",
            "你希望的行为已恢复。",
        ] {
            assert!(assistant_feedback_request(text).is_none(), "{text}");
        }
    }
    #[test]
    fn ignores_metadata_and_earlier_questions() {
        assert_eq!(assistant_feedback_request("已修复。\n\n请确认结果。<oai-mem-citation><citation_entries>internal</citation_entries></oai-mem-citation>"), Some("请确认结果。".into()));
        assert!(assistant_feedback_request("请确认结果。\n\n后续已验证通过，任务完成。").is_none());
        assert!(assistant_feedback_request(
            "Done.<citation_entries>请确认结果。</citation_entries>"
        )
        .is_none());
    }

    #[test]
    fn terminal_feedback_requires_complete_latest_turn_and_settled_tools() {
        use serde_json::json;
        let root = std::env::temp_dir().join(format!("nexushub-feedback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("rollout.jsonl");
        let timestamp = chrono::Utc::now().to_rfc3339();
        let start =
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-a"}});
        let final_reply = json!({"type":"response_item","payload":{"type":"message","role":"assistant","phase":"final","content":[{"type":"output_text","text":"请确认测试结果。"}]}});
        let terminal = json!({"type":"event_msg","timestamp":timestamp,"payload":{"type":"task_complete","turn_id":"turn-a","last_agent_message":"请确认测试结果。"}});
        let write = |events: &[serde_json::Value], partial: &str| {
            let text = events
                .iter()
                .map(|event| format!("{event}\n"))
                .collect::<String>()
                + partial;
            std::fs::write(&path, text).unwrap();
            super::super::rollout_pending_feedback(&path).unwrap()
        };
        assert!(write(&[start.clone(), final_reply.clone()], "").is_none());
        assert_eq!(
            write(&[start.clone(), final_reply.clone(), terminal.clone()], "")
                .unwrap()
                .turn_id,
            "turn-a"
        );
        assert!(write(&[start.clone(), terminal.clone()], "{\"type\":").is_none());
        let tool = json!({"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call-a","arguments":"{}"}});
        assert!(write(&[start.clone(), tool.clone(), terminal.clone()], "").is_none());
        let output = json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call-a","output":"ok"}});
        assert!(write(&[start.clone(), tool, output, terminal.clone()], "").is_some());
        for after in [
            json!({"type":"event_msg","payload":{"type":"agent_message","phase":"commentary","message":"继续验证"}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"已确认"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-b"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","phase":"final","content":[{"type":"output_text","text":"已自行验证完成。"}]}}),
        ] {
            assert!(write(&[start.clone(), terminal.clone(), after], "").is_none());
        }
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
