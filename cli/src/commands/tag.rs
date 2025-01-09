// Copyright 2020-2024 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use jj_lib::str_util::StringPattern;

use crate::cli_util::CommandHelper;
use crate::command_error::CommandError;
use crate::commit_templater::CommitTemplateLanguage;
use crate::commit_templater::RefName;
use crate::ui::Ui;

/// Manage tags.
#[derive(clap::Subcommand, Clone, Debug)]
pub enum TagCommand {
    #[command(visible_alias("l"))]
    List(TagListArgs),
}

/// List tags.
#[derive(clap::Args, Clone, Debug)]
pub struct TagListArgs {
    /// Show tags whose local name matches
    ///
    /// By default, the specified name matches exactly. Use `glob:` prefix to
    /// select tags by wildcard pattern. For details, see
    /// https://jj-vcs.github.io/jj/latest/revsets/#string-patterns.
    #[arg(value_parser = StringPattern::parse)]
    pub names: Vec<StringPattern>,
    /// Render each tag using the given template
    ///
    /// All 0-argument methods of the `RefName` type are available as keywords.
    ///
    /// For the syntax, see https://jj-vcs.github.io/jj/latest/templates/
    #[arg(long, short = 'T')]
    template: Option<String>,
}

pub fn cmd_tag(
    ui: &mut Ui,
    command: &CommandHelper,
    subcommand: &TagCommand,
) -> Result<(), CommandError> {
    match subcommand {
        TagCommand::List(args) => cmd_tag_list(ui, command, args),
    }
}

fn cmd_tag_list(
    ui: &mut Ui,
    command: &CommandHelper,
    args: &TagListArgs,
) -> Result<(), CommandError> {
    let workspace_command = command.workspace_helper(ui)?;
    let repo = workspace_command.repo();
    let view = repo.view();

    let template = {
        let language = workspace_command.commit_template_language();
        let text = match &args.template {
            Some(value) => value.to_owned(),
            None => workspace_command.settings().get("templates.tag_list")?,
        };
        workspace_command
            .parse_template(ui, &language, &text, CommitTemplateLanguage::wrap_ref_name)?
            .labeled("tag_list")
    };

    ui.request_pager("tag");
    let mut formatter = ui.stdout_formatter();

    for (name, target) in view.tags() {
        if !args.names.is_empty() && !args.names.iter().any(|pattern| pattern.matches(name)) {
            continue;
        }
        let ref_name = RefName::local_only(name, target.clone());
        template.format(&ref_name, formatter.as_mut())?;
    }

    Ok(())
}
