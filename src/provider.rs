use crate::{
    chat::{Conversation, Message},
    tool::{get_available_tools, Tool, ToolCall},
};
use anyhow::Result;
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionTool,
    ChatCompletionToolArgs, ChatCompletionToolType, CreateChatCompletionRequestArgs, FunctionCall,
    FunctionObjectArgs,
};
use dotenv::dotenv;
use serde_json::json;
use std::env;

pub async fn call_openai_api(conversation: &Conversation) -> Result<Message> {
    dotenv().ok();
    let api_key =
        env::var("API_KEY").map_err(|_| anyhow::anyhow!("API_KEY not found in environment"))?;
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
                        let tool_calls: Vec<ChatCompletionMessageToolCall> = msg
                            .tool_calls
                            .iter()
                            .map(|tc| ChatCompletionMessageToolCall {
                                id: tc.id.clone(),
                                r#type: ChatCompletionToolType::Function,
                                function: FunctionCall {
                                    name: tc.name.clone(),
                                    arguments: tc.arguments.clone(),
                                },
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
            get_available_tools()
                .into_iter()
                .map(|t| to_openai_tool(&t))
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

pub fn to_openai_tool(tool: &Tool) -> ChatCompletionTool {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for p in &tool.parameters {
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

    ChatCompletionToolArgs::default()
        .r#type(ChatCompletionToolType::Function)
        .function(
            FunctionObjectArgs::default()
                .name(&tool.name)
                .description(&tool.description)
                .parameters(serde_json::Value::Object(schema))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap()
}

impl From<&ChatCompletionMessageToolCall> for ToolCall {
    fn from(value: &ChatCompletionMessageToolCall) -> Self {
        ToolCall {
            id: value.id.clone(),
            name: value.function.name.clone(),
            arguments: value.function.arguments.clone(),
        }
    }
}
