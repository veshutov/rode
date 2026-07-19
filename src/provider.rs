use crate::{
    message::{Conversation, Message, Role},
    tools::{Tool, ToolCall, ToolRegistry},
};
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionTools,
    CreateChatCompletionRequest, CreateChatCompletionRequestArgs, FunctionCall, FunctionObjectArgs,
};
use futures::StreamExt;
use serde_json::json;
use std::env;

/// Stream response tokens. Returns (full_content, tool_calls) when complete.
pub async fn stream_openai_api(
    conversation: &Conversation,
    tool_registry: &ToolRegistry,
    mut on_token: impl FnMut(&str),
) -> Result<Message> {
    let client = build_client()?;
    let request = build_request(conversation, tool_registry)?;

    let mut stream = client.chat().create_stream(request).await?;
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let _current_tool_call: Option<ToolCall> = None;

    while let Some(result) = stream.next().await {
        let response = result?;
        if let Some(choice) = response.choices.first() {
            if let Some(delta) = &choice.delta.content {
                content.push_str(delta);
                on_token(delta);
            }
            if let Some(tcs) = &choice.delta.tool_calls {
                for tc in tcs {
                    let idx = tc.index as usize;
                    while tool_calls.len() <= idx {
                        tool_calls.push(ToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });
                    }
                    if let Some(id) = &tc.id {
                        tool_calls[idx].id = id.clone();
                    }
                    if let Some(function) = &tc.function {
                        if let Some(name) = &function.name {
                            tool_calls[idx].name = name.clone();
                        }
                        if let Some(args) = &function.arguments {
                            tool_calls[idx].arguments.push_str(args);
                        }
                    }
                }
            }
        }
    }

    // Filter out empty tool calls
    tool_calls.retain(|tc| !tc.id.is_empty());

    Ok(Message {
        role: Role::Assistant,
        content,
        tool_calls,
        tool_call_id: None,
    })
}

fn build_client() -> Result<async_openai::Client<async_openai::config::OpenAIConfig>> {
    let api_key = env::var("RODE_API_KEY")
        .map_err(|_| anyhow::anyhow!("RODE_API_KEY not found in environment"))?;
    let url = env::var("URL").map_err(|_| anyhow::anyhow!("URL not found in environment"))?;

    let config = async_openai::config::OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(url);
    Ok(async_openai::Client::with_config(config))
}

fn build_request(
    conversation: &Conversation,
    tool_registry: &ToolRegistry,
) -> Result<CreateChatCompletionRequest> {
    let model = env::var("MODEL").map_err(|_| anyhow::anyhow!("MODEL not found in environment"))?;

    let openai_messages: Result<Vec<ChatCompletionRequestMessage>> = conversation
        .get_messages()
        .iter()
        .map(|msg| -> Result<ChatCompletionRequestMessage> {
            let m = match msg.role {
                Role::System => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ),
                Role::User => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ),
                Role::Assistant => {
                    let mut args = ChatCompletionRequestAssistantMessageArgs::default();
                    if !msg.content.is_empty() {
                        args.content(msg.content.clone());
                    }
                    if !msg.tool_calls.is_empty() {
                        let tool_calls: Vec<ChatCompletionMessageToolCalls> = msg
                            .tool_calls
                            .iter()
                            .map(|tc| {
                                ChatCompletionMessageToolCalls::Function(
                                    ChatCompletionMessageToolCall {
                                        id: tc.id.clone(),
                                        function: FunctionCall {
                                            name: tc.name.clone(),
                                            arguments: tc.arguments.clone(),
                                        },
                                    },
                                )
                            })
                            .collect();
                        args.tool_calls(tool_calls);
                    }
                    ChatCompletionRequestMessage::Assistant(args.build()?)
                }
                Role::Tool => ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessageArgs::default()
                        .content(msg.content.clone())
                        .tool_call_id(msg.tool_call_id.clone().unwrap_or_default())
                        .build()?,
                ),
            };
            Ok(m)
        })
        .collect();
    let openai_messages = openai_messages?;

    Ok(CreateChatCompletionRequestArgs::default()
        .model(&model)
        .stream(true)
        .messages(openai_messages)
        .tools(
            tool_registry
                .get_available_tools()
                .into_iter()
                .map(|t| to_openai_tool(t))
                .collect::<Vec<_>>(),
        )
        .build()?)
}

fn to_openai_tool(tool: &std::sync::Arc<dyn Tool>) -> ChatCompletionTools {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for param in tool.parameters() {
        properties.insert(
            param.name.clone(),
            json!({
                "type": param.r#type,
                "description": param.description,
            }),
        );
        if param.required {
            required.push(param.name.clone());
        }
    }
    let mut parameters = serde_json::Map::new();
    parameters.insert("type".to_string(), json!("object"));
    parameters.insert("properties".to_string(), json!(properties));
    if !required.is_empty() {
        parameters.insert("required".to_string(), json!(required));
    }

    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObjectArgs::default()
            .name(tool.name().to_owned())
            .description(tool.description().to_owned())
            .parameters(serde_json::Value::Object(parameters))
            .build()
            .unwrap(),
    })
}

impl From<&ChatCompletionMessageToolCalls> for ToolCall {
    fn from(value: &ChatCompletionMessageToolCalls) -> Self {
        match value {
            ChatCompletionMessageToolCalls::Function(tool_call) => ToolCall {
                id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                arguments: tool_call.function.arguments.clone(),
            },
            ChatCompletionMessageToolCalls::Custom(_) => {
                todo!()
            }
        }
    }
}
