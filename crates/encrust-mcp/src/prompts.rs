//! The ready requests a client offers the user, served as the bundle's manifest lists them, so
//! the extension and the server never tell two lists (ADR 0455).

use rmcp::ErrorData;
use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};
use serde::Deserialize;

const MANIFEST: &str = include_str!("../bundle/manifest.json");

#[derive(Deserialize)]
struct Manifest {
    prompts: Vec<Listed>,
}

/// One prompt as the manifest lists it: its text names each argument as `${arguments.NAME}`.
#[derive(Deserialize)]
struct Listed {
    name: String,
    description: String,
    #[serde(default)]
    arguments: Vec<String>,
    text: String,
}

fn listed() -> Vec<Listed> {
    serde_json::from_str::<Manifest>(MANIFEST).map_or_else(|_| Vec::new(), |read| read.prompts)
}

/// Every prompt, each argument required.
pub fn list() -> Vec<Prompt> {
    listed()
        .into_iter()
        .map(|listed| {
            let arguments = listed
                .arguments
                .into_iter()
                .map(|name| PromptArgument::new(name).with_required(true))
                .collect();
            Prompt::new(listed.name, Some(listed.description), Some(arguments))
        })
        .collect()
}

/// The prompt `name` with `arguments` set into its text, as one message from the user.
pub fn get(name: &str, arguments: Option<&JsonObject>) -> Result<GetPromptResult, ErrorData> {
    let listed = listed()
        .into_iter()
        .find(|listed| listed.name == name)
        .ok_or_else(|| ErrorData::invalid_params(format!("no prompt is named {name}"), None))?;
    let mut text = listed.text;
    for argument in &listed.arguments {
        let value = arguments
            .and_then(|given| given.get(argument)?.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ErrorData::invalid_params(format!("{name} needs its {argument}"), None)
            })?;
        text = text.replace(&format!("${{arguments.{argument}}}"), value);
    }
    Ok(
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
            .with_description(listed.description),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(result: &GetPromptResult) -> String {
        let message = serde_json::to_value(&result.messages[0]).expect("a message serialises");
        message["content"]["text"]
            .as_str()
            .expect("a text message")
            .to_owned()
    }

    #[test]
    fn the_manifest_lists_prompts_whose_arguments_its_text_names() {
        let listed = listed();
        assert!(!listed.is_empty(), "the manifest lists no prompt");
        for prompt in listed {
            for argument in &prompt.arguments {
                let named = format!("${{arguments.{argument}}}");
                assert!(
                    prompt.text.contains(&named),
                    "{} lacks {named}",
                    prompt.name
                );
            }
        }
    }

    #[test]
    fn a_prompt_sets_its_arguments_into_its_text() {
        let mut given = JsonObject::new();
        given.insert("model".into(), "/models/knight.stl".into());
        let result = get("prepare_miniature", Some(&given)).expect("a listed prompt");
        let text = text_of(&result);
        assert!(text.contains("/models/knight.stl"), "{text}");
        assert!(!text.contains("${"), "{text}");
    }

    #[test]
    fn a_missing_argument_or_an_unknown_prompt_is_refused() {
        assert!(get("prepare_miniature", None).is_err());
        assert!(get("no_such_prompt", None).is_err());
    }
}
