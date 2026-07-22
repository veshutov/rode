use crate::{
    agent::session::{Message, Role},
    tools::{ToolCall, ToolInfo, ToolRegistry},
};
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionStreamOptions, ChatCompletionTool,
    ChatCompletionTools, CreateChatCompletionRequest, CreateChatCompletionRequestArgs,
    FunctionCall, FunctionObjectArgs,
};
use chrono::Utc;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct LLMProviderConfig {
    pub key: String,
    pub url: String,
}

#[derive(Clone)]
pub struct LLMProvider {
    client: async_openai::Client<async_openai::config::OpenAIConfig>,
    tool_registry: ToolRegistry,
}

impl LLMProvider {
    pub fn new(config: LLMProviderConfig, tool_registry: ToolRegistry) -> Self {
        let api_key = config.key.clone();
        let base_url = config.url.clone();
        Self {
            client: async_openai::Client::with_config(
                async_openai::config::OpenAIConfig::new()
                    .with_api_key(api_key)
                    .with_api_base(base_url),
            ),
            tool_registry,
        }
    }

    pub async fn stream_openai_api(
        &self,
        messages: &[Message],
        model: &str,
        mut on_token: impl FnMut(&str),
        cancelled: Arc<AtomicBool>,
    ) -> Result<Message> {
        let request = build_request(messages, &self.tool_registry, model)?;

        let mut stream = self.client.chat().create_stream(request).await?;
        let mut content = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        let mut total_tokens: Option<u32> = None;

        while let Some(result) = stream.next().await {
            if cancelled.load(Ordering::SeqCst) {
                break;
            }
            let response = result?;

            if let Some(usage) = &response.usage {
                total_tokens = Some(usage.total_tokens);
            }

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
            id: Uuid::now_v7(),
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
            used_tokens: total_tokens,
            timestamp: Utc::now(),
        })
    }
}

fn build_request(
    messages: &[Message],
    tool_registry: &ToolRegistry,
    model: &str,
) -> Result<CreateChatCompletionRequest> {
    let openai_messages: Result<Vec<ChatCompletionRequestMessage>> = messages
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
        .model(model)
        .stream(true)
        .stream_options(ChatCompletionStreamOptions {
            include_usage: Some(true),
            include_obfuscation: None,
        })
        .messages(openai_messages)
        .tools(
            tool_registry
                .available_tools()
                .iter()
                .map(|tool| tool.into())
                .collect::<Vec<_>>(),
        )
        .build()?)
}

impl From<&ToolInfo> for ChatCompletionTools {
    fn from(tool: &ToolInfo) -> ChatCompletionTools {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for param in &tool.parameters {
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
                .name(tool.name.clone())
                .description(tool.description.clone())
                .parameters(serde_json::Value::Object(parameters))
                .build()
                .unwrap(),
        })
    }
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
