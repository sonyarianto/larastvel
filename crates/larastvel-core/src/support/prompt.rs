use dialoguer::{Confirm, FuzzySelect, Input, MultiSelect, Password, Select};

pub use dialoguer::Error as PromptError;

pub struct Prompt;

type Result<T> = std::result::Result<T, PromptError>;

impl Prompt {
    pub fn ask(question: &str, default: Option<&str>) -> Result<String> {
        let mut input = Input::<String>::new().with_prompt(question);
        if let Some(default) = default {
            input = input.default(default.to_string());
        }
        input.interact_text()
    }

    pub fn confirm(question: &str, default: Option<bool>) -> Result<bool> {
        let mut confirm = Confirm::new().with_prompt(question);
        if let Some(default) = default {
            confirm = confirm.default(default);
        }
        confirm.interact()
    }

    pub fn secret(question: &str) -> Result<String> {
        Password::new().with_prompt(question).interact()
    }

    pub fn choice(question: &str, options: &[&str], default: Option<usize>) -> Result<String> {
        let mut select = Select::new().with_prompt(question).items(options);
        if let Some(default) = default {
            select = select.default(default);
        }
        let index = select.interact()?;
        Ok(options[index].to_string())
    }

    pub fn autocomplete(
        question: &str,
        choices: &[&str],
        default: Option<usize>,
    ) -> Result<String> {
        let mut select = FuzzySelect::new().with_prompt(question).items(choices);
        if let Some(default) = default {
            select = select.default(default);
        }
        let index = select.interact()?;
        Ok(choices[index].to_string())
    }

    pub fn multiselect(
        question: &str,
        options: &[&str],
        defaults: Option<&[bool]>,
    ) -> Result<Vec<String>> {
        let mut multi = MultiSelect::new().with_prompt(question).items(options);
        if let Some(defaults) = defaults {
            multi = multi.defaults(defaults);
        }
        let indices = multi.interact()?;
        Ok(indices
            .into_iter()
            .map(|i| options[i].to_string())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_exists() {
        assert_eq!(std::mem::size_of::<Prompt>(), 0);
    }
}
