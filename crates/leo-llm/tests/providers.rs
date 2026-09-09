use leo_llm::{ChatRequest, Client, ProviderId};

#[test]
fn parses_provider_aliases() {
    assert_eq!(ProviderId::parse("xai").unwrap(), ProviderId::Grok);
    assert_eq!(ProviderId::parse("openai").unwrap(), ProviderId::Gpt);
    assert_eq!(ProviderId::parse("anthropic").unwrap(), ProviderId::Claude);
    assert!(ProviderId::parse("gemini").is_err());
}

#[test]
fn grok_is_default_xai_endpoint() {
    assert_eq!(ProviderId::Grok.default_base_url(), "https://api.x.ai/v1");
    assert_eq!(ProviderId::Grok.default_model(), "grok-4.6");
    assert_eq!(ProviderId::Grok.env_key(), Some("XAI_API_KEY"));
}

#[test]
fn ollama_from_env_does_not_need_a_key() {
    let client = Client::from_env(ProviderId::Ollama).unwrap();
    assert_eq!(client.provider(), ProviderId::Ollama);
    assert_eq!(ProviderId::Ollama.default_base_url(), "http://127.0.0.1:11434");
}

#[test]
fn parses_ollama_tags() {
    let names = leo_llm::parse_ollama_tags(
        r#"{"models":[{"name":"gemma3:latest"},{"name":"gemma3:4b"}]}"#,
    )
    .unwrap();
    assert_eq!(names, vec!["gemma3:latest", "gemma3:4b"]);
}

#[test]
fn ollama_chat_uses_native_api() {
    let req = ChatRequest::user("hola").with_system("sé breve");
    let body = leo_llm::protocol::ollama_body("gemma3:latest", &req).unwrap();
    assert_eq!(body["model"], "gemma3:latest");
    assert_eq!(body["stream"], false);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "hola");
}

#[test]
fn parses_ollama_chat() {
    let raw = r#"{
        "model": "gemma3:latest",
        "message": {"role": "assistant", "content": "hey"},
        "done": true
    }"#;
    let resp = leo_llm::protocol::parse_ollama("gemma3:latest", raw).unwrap();
    assert_eq!(resp.text, "hey");
    assert_eq!(resp.provider, ProviderId::Ollama);
}

#[test]
fn extracts_nested_http_error_message() {
    let msg = leo_llm::extract_error_message(
        r#"{"error":{"message":"model 'llama3.2' not found","type":"not_found_error"}}"#,
    )
    .unwrap();
    assert_eq!(msg, "model 'llama3.2' not found");
    let plain = leo_llm::extract_error_message(r#"{"error":"not found"}"#).unwrap();
    assert_eq!(plain, "not found");
}

#[test]
fn connect_uses_stored_url() {
    let client = Client::connect(
        ProviderId::Ollama,
        None,
        Some("http://localhost:11434".into()),
    )
    .unwrap();
    assert_eq!(client.provider(), ProviderId::Ollama);
}

#[test]
fn openai_body_includes_messages() {
    let req = ChatRequest::user("hola").with_system("sé breve");
    let body = leo_llm::protocol::openai_body("grok-4.6", &req).unwrap();
    assert_eq!(body["model"], "grok-4.6");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "hola");
}

#[test]
fn claude_lifts_system_out_of_messages() {
    let req = ChatRequest::user("hola").with_system("sé breve");
    let body = leo_llm::protocol::claude_body("claude-sonnet-5", &req).unwrap();
    assert_eq!(body["system"], "sé breve");
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["max_tokens"], 1024);
}

#[test]
fn parses_openai_choice() {
    let raw = r#"{
        "model": "grok-4.6",
        "choices": [{"message": {"role": "assistant", "content": "hey"}}]
    }"#;
    let resp = leo_llm::protocol::parse_openai(ProviderId::Grok, "grok-4.6", raw).unwrap();
    assert_eq!(resp.text, "hey");
    assert_eq!(resp.provider, ProviderId::Grok);
}

#[test]
fn parses_claude_blocks() {
    let raw = r#"{
        "model": "claude-sonnet-5",
        "content": [{"type": "text", "text": "hola"}]
    }"#;
    let resp = leo_llm::protocol::parse_claude("claude-sonnet-5", raw).unwrap();
    assert_eq!(resp.text, "hola");
    assert_eq!(resp.provider, ProviderId::Claude);
}
