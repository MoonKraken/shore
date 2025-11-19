use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use eyre::Result;
use openai_api_rs_prime::v1::{
    api::OpenAIClient,
    chat_completion::{
        self, ChatCompletionMessage, MessageRole, Tool, ToolCall, ToolCallFunction, ToolChoiceType,
        chat_completion::ChatCompletionRequest,
    },
    types::{Function, FunctionParameters},
};
use tracing::{info, warn};

use crate::{
    model::{
        chat::{ChatMessage, ChatRole},
        model::Model,
    },
    provider::provider::{GenerationResult, Provider, ProviderClient, ToolCallRequest},
};

fn chat_role_to_message_role(chat_role: &ChatRole) -> MessageRole {
    match chat_role {
        ChatRole::User => MessageRole::user,
        ChatRole::Assistant => MessageRole::assistant,
        ChatRole::ToolResult => MessageRole::tool,
    }
}

fn create_chat_request(
    model: &str,
    system_prompt: &str,
    conversation: &[ChatMessage],
    available_tools: &[&dyn crate::model::tool::Tool],
) -> Result<ChatCompletionRequest> {
    let mut messages = Vec::new();

    // Add system message if instructions are provided
    if !system_prompt.is_empty() {
        messages.push(ChatCompletionMessage {
            role: MessageRole::system,
            content: chat_completion::Content::Text(system_prompt.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Add prior conversation messages
    for chat_msg in conversation {
        // this conversion could be done by implementing From<ToolCallRequest> for ToolCall, but the orphan rule makes that difficult
        let tool_calls: Option<Vec<ToolCall>> =
            if let Some(tool_calls) = chat_msg.tool_calls.as_ref() {
                // Vec<ToolCallRequest> doesn't play nice with Sqlx for some reason, so we store it as a string and deserialize it here instead
                let tool_calls: Vec<ToolCallRequest> = serde_json::from_str(tool_calls)
                    .map_err(|e| eyre::eyre!("Failed to parse tool calls: {}", e))?;
                Some(
                    tool_calls
                        .into_iter()
                        .map(|tc| ToolCall {
                            id: tc.tool_call_id,
                            r#type: "function".to_string(),
                            function: ToolCallFunction {
                                name: tc.name,
                                arguments: tc.params,
                            },
                        })
                        .collect(),
                )
            } else {
                None
            };

        messages.push(ChatCompletionMessage {
            role: chat_role_to_message_role(&chat_msg.chat_role),
            content: chat_completion::Content::Text(chat_msg.content.clone().unwrap_or_default()), // why doesn't openai offer an optional for content? might be blank for tool calls right?
            name: chat_msg.name.clone(),
            tool_calls,
            tool_call_id: chat_msg.tool_call_id.clone(),
        });
    }

    let mut res = ChatCompletionRequest::new(model.to_string(), messages);
    if !available_tools.is_empty() {
        res = res
            .tools(
                available_tools
                    .iter()
                    .map(|t| {
                        let params: FunctionParameters =
                            serde_json::from_value(t.parameter_schema()).map_err(|e| {
                                eyre::eyre!("Failed to parse tool parameters: {}", e)
                            })?;

                        let tool = Tool {
                            r#type: chat_completion::ToolType::Function,
                            function: Function {
                                name: t.name().to_string(),
                                description: Some(t.description().to_string()),
                                parameters: params,
                            },
                        };

                        let res: Result<Tool> = Ok(tool);
                        res
                    })
                    .collect::<Result<Vec<Tool>>>()?,
            )
            .parallel_tool_calls(false)
            .tool_choice(ToolChoiceType::Auto);
    }

    Ok(res)
}

pub struct OpenAIProvider {
    provider: Provider,
    client: OpenAIClient,
}

impl OpenAIProvider {
    pub fn new(provider: Provider) -> Self {
        let client = OpenAIClient::builder().with_endpoint(&provider.base_url);
        let client = if !provider.api_key_env_var.is_empty() {
            // this indicates the provider doesn't require an api key
            // primarily use case for this is things like ollama
            client
        } else {
            let token = std::env::var(&provider.api_key_env_var)
                .expect("API key env var not set for provider! This should not happen because we should have caught it earlier");

            info!("Running inference with endpoint {}", &provider.base_url);
            client.with_api_key(token)
        };

        let client = client.build().expect("could not create OpenAI client");
        Self { provider, client }
    }
}

#[async_trait]
impl ProviderClient for OpenAIProvider {
    async fn get_models(&mut self) -> Result<HashMap<String, Model>> {
        let models = self.client.list_models().await?.data;

        Ok(models
            .into_iter()
            .filter_map(|model_response| {
                let model_id = if let Some(model_id) = model_response.id {
                    model_id
                } else {
                    warn!(
                        "no id in one of the models from models response, this is quite unexpected: {:?}",
                        model_response
                    );
                    return None;
                };

                let model = Model {
                    id: 0,
                    provider_id: self.provider.id,
                    model: model_id,
                    api_type: 0,
                    disabled: false,
                    deprecated: false,
                    created_dt: Utc::now().timestamp(),
                };

                Some((model.model.clone(), model))
            })
            .collect())
    }

    async fn run(
        &mut self,
        model: &str,
        system_prompt: &str,
        conversation: &Vec<ChatMessage>,
        available_tools: Vec<&dyn crate::model::tool::Tool>,
        remove_think_tokens: bool,
    ) -> Result<GenerationResult> {
        let request = create_chat_request(model, system_prompt, &conversation, &available_tools)?;

        info!(
            "Sending completion request with messages: {:?}",
            &request.messages
        );
        let response = self.client.chat_completion(request).await?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| eyre::eyre!("No content in response"))?;

        let tool_calls = choice
            .message
            .tool_calls
            .map(|tool_calls| {
                tool_calls
                    .into_iter()
                    .map(|tool_call| ToolCallRequest {
                        tool_call_id: tool_call.id.clone(),
                        name: tool_call.function.name,
                        params: tool_call.function.arguments.clone(),
                    })
                    .collect::<Vec<ToolCallRequest>>()
            })
            .unwrap_or(vec![]);

        let content = choice.message.content.clone();

        let content = content.map(|content| {
            if remove_think_tokens {
                if let Some((_, after_think)) = content.split_once("</think>") {
                    info!("Trimmed think tokens from LLM response!");
                    after_think.trim().to_string() // should we do this trim irrespective of whether we removed think tokens?
                } else {
                    content
                }
            } else {
                content
            }
        });

        Ok(GenerationResult {
            content,
            tool_calls,
        })
    }
}
