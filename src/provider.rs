use crate::{
    message::{Conversation, Message},
    tools::{Tool, ToolCall, ToolRegistry},
};
use anyhow::Result;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCalls,
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionTools,
    CreateChatCompletionRequestArgs, FunctionCall, FunctionObjectArgs,
};
use dotenv::dotenv;
use serde_json::json;
use std::env;

pub async fn call_openai_api(
    conversation: &Conversation,
    tool_registry: &ToolRegistry,
) -> Result<Message> {
    dotenv().ok();
    let api_key = env::var("RODE_API_KEY")
        .map_err(|_| anyhow::anyhow!("RODE_API_KEY not found in environment"))?;
    let url = env::var("URL").map_err(|_| anyhow::anyhow!("URL not found in environment"))?;
    let model = env::var("MODEL").map_err(|_| anyhow::anyhow!("MODEL not found in environment"))?;

    let config = async_openai::config::OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(url);
    let client = async_openai::Client::with_config(config);

    let openai_messages: Result<Vec<ChatCompletionRequestMessage>> = conversation
        .get_messages()
        .iter()
        .map(|msg| -> Result<ChatCompletionRequestMessage> {
            let m = match msg.role.as_str() {
                "system" => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ),
                "user" => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ),
                "assistant" => {
                    let mut args = ChatCompletionRequestAssistantMessageArgs::default();
                    args.content(msg.content.clone());
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
                "tool" => ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessageArgs::default()
                        .content(msg.content.clone())
                        .tool_call_id(msg.tool_call_id.clone().unwrap_or_default())
                        .build()?,
                ),
                _ => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ),
            };
            Ok(m)
        })
        .collect();
    let openai_messages = openai_messages?;

    let request = CreateChatCompletionRequestArgs::default()
        .model(&model)
        .messages(openai_messages)
        .tools(
            tool_registry
                .get_available_tools()
                .into_iter()
                .map(|t| to_openai_tool(t))
                .collect::<Vec<_>>(),
        )
        .build()?;

    let response = client.chat().create(request).await?;

    let choice = response
        .choices
        .first()
        .ok_or_else(|| anyhow::anyhow!("No response received"))?;

    let message = Message {
        role: "assistant".to_string(),
        content: choice.message.content.clone().unwrap_or_default(),
        tool_calls: choice
            .message
            .tool_calls
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|tc| tc.into())
            .collect(),
        tool_call_id: None,
    };

    Ok(message)
}

fn to_openai_tool(tool: &Box<dyn Tool>) -> ChatCompletionTools {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for p in tool.parameters() {
        properties.insert(
            p.name.clone(),
            json!({
                "type": p.r#type,
                "description": p.description,
            }),
        );
        if p.required {
            required.push(p.name.clone());
        }
    }
    let mut schema = serde_json::Map::new();
    schema.insert("type".to_string(), json!("object"));
    schema.insert("properties".to_string(), json!(properties));
    if !required.is_empty() {
        schema.insert("required".to_string(), json!(required));
    }

    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObjectArgs::default()
            .name(tool.name().to_owned())
            .description(tool.description().to_owned())
            .parameters(serde_json::Value::Object(schema))
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
