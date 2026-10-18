fn balanced_block(source: &str, start: usize) -> Result<&str, String> {
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| "source block has no opening brace".to_owned())?;
    let mut depth = 0usize;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "source block has an unmatched closing brace".to_owned())?;
                if depth == 0 {
                    return Ok(&source[start..=open + offset]);
                }
            }
            _ => {}
        }
    }
    Err("source block has no closing brace".to_owned())
}

fn function_block<'a>(source: &'a str, signature: &str) -> Result<&'a str, String> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("missing function signature: {signature}"))?;
    balanced_block(source, start)
}

fn block_after<'a>(source: &'a str, marker: &str) -> Result<&'a str, String> {
    let start = source
        .find(marker)
        .ok_or_else(|| format!("missing source marker: {marker}"))?;
    // Start after the marker: an `if let` pattern may contain destructuring
    // braces before the block's opening brace.
    balanced_block(source, start + marker.len())
}

fn validate_attachment_channel_contract(
    desktop_source: &str,
    attachment_source: &str,
) -> Result<(), String> {
    let command = function_block(desktop_source, "pub async fn attachments(")?;
    let command_header_end = command
        .find('{')
        .ok_or_else(|| "attachments command has no body".to_owned())?;
    let command_header = &command[..command_header_end];
    if !command_header.contains("on_event: JavaScriptChannelId") {
        return Err("attachments command must accept JavaScriptChannelId".to_owned());
    }
    if command_header.contains("on_event: Channel<DesktopAttachmentEvent>") {
        return Err("attachments command must not own a temporary Channel".to_owned());
    }
    if !command
        .contains("attachments::handle(window, &state, &remote, &bridge, request, on_event).await")
    {
        return Err("attachments command must pass the callback ID to handle".to_owned());
    }

    let receiver = block_after(attachment_source, "pub(super) struct Receiver")?;
    if !receiver.contains("channel: Channel<DesktopAttachmentEvent>") {
        return Err("Receiver must retain the owned event Channel".to_owned());
    }

    let handle = function_block(attachment_source, "pub(super) async fn handle(")?;
    let handle_header_end = handle
        .find('{')
        .ok_or_else(|| "attachment handle has no body".to_owned())?;
    let handle_header = &handle[..handle_header_end];
    if !handle_header.contains("channel: JavaScriptChannelId") {
        return Err("attachment handle must accept JavaScriptChannelId".to_owned());
    }
    if handle.matches(".channel_on(").count() != 1 {
        return Err("attachment handle must bind exactly one owned channel".to_owned());
    }
    let bind = block_after(
        handle,
        "if let DesktopAttachmentAction::Bind { bounds, handles } = request.action",
    )?;
    if !bind.contains("let channel = channel.channel_on(window.as_ref().clone());") {
        return Err("only the Bind action may construct the owned channel".to_owned());
    }
    if !bind.contains("let receiver = Receiver {") || !bind.contains("            channel,") {
        return Err("Bind must store the channel in Receiver".to_owned());
    }
    Ok(())
}

#[test]
fn attachment_channel_contract_is_structural_and_rejects_legacy_owner() {
    let desktop_source = include_str!("../src/codex_desktop.rs");
    let attachment_source = include_str!("../src/codex_desktop/attachments.rs");

    validate_attachment_channel_contract(desktop_source, attachment_source)
        .expect("attachment callback must be an ID owned only by the Bind action");

    // Red-side proof: the contract rejects the pre-fix command signature and
    // the pre-fix per-call Channel construction without mutating product code.
    let legacy_signature = desktop_source.replace(
        "on_event: JavaScriptChannelId",
        "on_event: Channel<DesktopAttachmentEvent>",
    );
    assert!(validate_attachment_channel_contract(&legacy_signature, attachment_source).is_err());

    let legacy_handle = attachment_source
        .replace(
            "channel: JavaScriptChannelId",
            "channel: Channel<DesktopAttachmentEvent>",
        )
        .replace(
            "let channel = channel.channel_on(window.as_ref().clone());",
            "let channel = channel;",
        );
    assert!(validate_attachment_channel_contract(desktop_source, &legacy_handle).is_err());
}
