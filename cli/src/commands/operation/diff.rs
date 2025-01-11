// Copyright 2024 The Jujutsu Authors
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

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use clap_complete::ArgValueCandidates;
use indexmap::IndexMap;
use itertools::Itertools;
use jj_lib::backend::ChangeId;
use jj_lib::backend::CommitId;
use jj_lib::commit::Commit;
use jj_lib::dag_walk;
use jj_lib::graph::GraphEdge;
use jj_lib::graph::TopoGroupedGraphIterator;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::op_store::RefTarget;
use jj_lib::op_store::RemoteRef;
use jj_lib::op_store::RemoteRefState;
use jj_lib::refs::diff_named_ref_targets;
use jj_lib::refs::diff_named_remote_refs;
use jj_lib::repo::ReadonlyRepo;
use jj_lib::repo::Repo;
use jj_lib::revset;
use jj_lib::revset::RevsetIteratorExt as _;

use crate::cli_util::CommandHelper;
use crate::cli_util::LogContentFormat;
use crate::command_error::CommandError;
use crate::commit_templater::CommitTemplateLanguage;
use crate::complete;
use crate::diff_util::diff_formats_for_log;
use crate::diff_util::DiffFormatArgs;
use crate::diff_util::DiffRenderer;
use crate::formatter::Formatter;
use crate::graphlog::get_graphlog;
use crate::graphlog::GraphStyle;
use crate::templater::TemplateRenderer;
use crate::ui::Ui;

/// Compare changes to the repository between two operations
#[derive(clap::Args, Clone, Debug)]
pub struct OperationDiffArgs {
    /// Show repository changes in this operation, compared to its parent
    #[arg(
        long,
        visible_alias = "op",
        add = ArgValueCandidates::new(complete::operations),
    )]
    operation: Option<String>,
    /// Show repository changes from this operation
    #[arg(
        long, short,
        conflicts_with = "operation",
        add = ArgValueCandidates::new(complete::operations),
    )]
    from: Option<String>,
    /// Show repository changes to this operation
    #[arg(
        long, short,
        conflicts_with = "operation",
        add = ArgValueCandidates::new(complete::operations),
    )]
    to: Option<String>,
    /// Don't show the graph, show a flat list of modified changes
    #[arg(long)]
    no_graph: bool,
    /// Show patch of modifications to changes
    ///
    /// If the previous version has different parents, it will be temporarily
    /// rebased to the parents of the new version, so the diff is not
    /// contaminated by unrelated changes.
    #[arg(long, short = 'p')]
    patch: bool,
    #[command(flatten)]
    diff_format: DiffFormatArgs,
}

pub fn cmd_op_diff(
    ui: &mut Ui,
    command: &CommandHelper,
    args: &OperationDiffArgs,
) -> Result<(), CommandError> {
    let workspace_command = command.workspace_helper(ui)?;
    let workspace_env = workspace_command.env();
    let repo_loader = workspace_command.workspace().repo_loader();
    let settings = workspace_command.settings();
    let from_op;
    let to_op;
    if args.from.is_some() || args.to.is_some() {
        from_op = workspace_command.resolve_single_op(args.from.as_deref().unwrap_or("@"))?;
        to_op = workspace_command.resolve_single_op(args.to.as_deref().unwrap_or("@"))?;
    } else {
        to_op = workspace_command.resolve_single_op(args.operation.as_deref().unwrap_or("@"))?;
        let to_op_parents: Vec<_> = to_op.parents().try_collect()?;
        from_op = repo_loader.merge_operations(to_op_parents, None)?;
    }
    let graph_style = GraphStyle::from_settings(settings)?;
    let with_content_format = LogContentFormat::new(ui, settings)?;

    let from_repo = repo_loader.load_at(&from_op)?;
    let to_repo = repo_loader.load_at(&to_op)?;

    // Create a new transaction starting from `to_repo`.
    let mut tx = to_repo.start_transaction();
    // Merge index from `from_repo` to `to_repo`, so commits in `from_repo` are
    // accessible.
    tx.repo_mut().merge_index(&from_repo);
    let merged_repo = tx.repo();

    let diff_renderer = {
        let formats = diff_formats_for_log(settings, &args.diff_format, args.patch)?;
        let path_converter = workspace_env.path_converter();
        let conflict_marker_style = workspace_env.conflict_marker_style();
        (!formats.is_empty())
            .then(|| DiffRenderer::new(merged_repo, path_converter, conflict_marker_style, formats))
    };
    let id_prefix_context = workspace_env.new_id_prefix_context();
    let commit_summary_template = {
        let language = workspace_env.commit_template_language(merged_repo, &id_prefix_context);
        let text = settings.get_string("templates.commit_summary")?;
        workspace_env.parse_template(ui, &language, &text, CommitTemplateLanguage::wrap_commit)?
    };

    let op_summary_template = workspace_command.operation_summary_template();
    ui.request_pager();
    let mut formatter = ui.stdout_formatter();
    write!(formatter, "From operation: ")?;
    op_summary_template.format(&from_op, &mut *formatter)?;
    writeln!(formatter)?;
    write!(formatter, "  To operation: ")?;
    op_summary_template.format(&to_op, &mut *formatter)?;
    writeln!(formatter)?;

    show_op_diff(
        ui,
        formatter.as_mut(),
        merged_repo,
        &from_repo,
        &to_repo,
        &commit_summary_template,
        (!args.no_graph).then_some(graph_style),
        &with_content_format,
        diff_renderer.as_ref(),
    )
}

/// Computes and shows the differences between two operations, using the given
/// `ReadonlyRepo`s for the operations.
/// `current_repo` should contain a `Repo` with the indices of both repos merged
/// into it.
#[allow(clippy::too_many_arguments)]
pub fn show_op_diff(
    ui: &Ui,
    formatter: &mut dyn Formatter,
    current_repo: &dyn Repo,
    from_repo: &Arc<ReadonlyRepo>,
    to_repo: &Arc<ReadonlyRepo>,
    commit_summary_template: &TemplateRenderer<Commit>,
    graph_style: Option<GraphStyle>,
    with_content_format: &LogContentFormat,
    diff_renderer: Option<&DiffRenderer>,
) -> Result<(), CommandError> {
    let changes = compute_operation_commits_diff(current_repo, from_repo, to_repo)?;

    let commit_id_change_id_map: HashMap<CommitId, ChangeId> = changes
        .iter()
        .flat_map(|(change_id, modified_change)| {
            itertools::chain(
                &modified_change.added_commits,
                &modified_change.removed_commits,
            )
            .map(|commit| (commit.id().clone(), change_id.clone()))
        })
        .collect();

    let change_parents: HashMap<_, _> = changes
        .iter()
        .map(|(change_id, modified_change)| {
            let parent_change_ids = get_parent_changes(modified_change, &commit_id_change_id_map);
            (change_id.clone(), parent_change_ids)
        })
        .collect();

    // Order changes in reverse topological order.
    let ordered_change_ids = dag_walk::topo_order_reverse(
        changes.keys().cloned().collect_vec(),
        |change_id: &ChangeId| change_id.clone(),
        |change_id: &ChangeId| change_parents.get(change_id).unwrap().clone(),
    );

    if !ordered_change_ids.is_empty() {
        writeln!(formatter)?;
        with_content_format.write(formatter, |formatter| {
            writeln!(formatter, "Changed commits:")
        })?;
        if let Some(graph_style) = graph_style {
            let mut raw_output = formatter.raw()?;
            let mut graph = get_graphlog(graph_style, raw_output.as_mut());

            let graph_iter = TopoGroupedGraphIterator::new(ordered_change_ids.iter().map(
                |change_id| -> Result<_, Infallible> {
                    let parent_change_ids = change_parents.get(change_id).unwrap();
                    Ok((
                        change_id.clone(),
                        parent_change_ids
                            .iter()
                            .map(|parent_change_id| GraphEdge::direct(parent_change_id.clone()))
                            .collect_vec(),
                    ))
                },
            ));

            for node in graph_iter {
                let (change_id, edges) = node.unwrap();
                let modified_change = changes.get(&change_id).unwrap();

                let mut buffer = vec![];
                let within_graph = with_content_format.sub_width(graph.width(&change_id, &edges));
                within_graph.write(ui.new_formatter(&mut buffer).as_mut(), |formatter| {
                    write_modified_change_summary(
                        formatter,
                        commit_summary_template,
                        modified_change,
                    )
                })?;
                if !buffer.ends_with(b"\n") {
                    buffer.push(b'\n');
                }
                if let Some(diff_renderer) = &diff_renderer {
                    let mut formatter = ui.new_formatter(&mut buffer);
                    show_change_diff(
                        ui,
                        formatter.as_mut(),
                        diff_renderer,
                        modified_change,
                        within_graph.width(),
                    )?;
                }

                // TODO: customize node symbol?
                let node_symbol = "○";
                graph.add_node(
                    &change_id,
                    &edges,
                    node_symbol,
                    &String::from_utf8_lossy(&buffer),
                )?;
            }
        } else {
            for change_id in ordered_change_ids {
                let modified_change = changes.get(&change_id).unwrap();
                with_content_format.write(formatter, |formatter| {
                    write_modified_change_summary(
                        formatter,
                        commit_summary_template,
                        modified_change,
                    )
                })?;
                if let Some(diff_renderer) = &diff_renderer {
                    let width = with_content_format.width();
                    show_change_diff(ui, formatter, diff_renderer, modified_change, width)?;
                }
            }
        }
    }

    let changed_local_bookmarks = diff_named_ref_targets(
        from_repo.view().local_bookmarks(),
        to_repo.view().local_bookmarks(),
    )
    .collect_vec();
    if !changed_local_bookmarks.is_empty() {
        writeln!(formatter)?;
        with_content_format.write(formatter, |formatter| {
            writeln!(formatter, "Changed local bookmarks:")
        })?;
        for (name, (from_target, to_target)) in changed_local_bookmarks {
            with_content_format.write(formatter, |formatter| {
                writeln!(formatter, "{name}:")?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    to_target,
                    true,
                    None,
                )?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    from_target,
                    false,
                    None,
                )
            })?;
        }
    }

    let changed_tags =
        diff_named_ref_targets(from_repo.view().tags(), to_repo.view().tags()).collect_vec();
    if !changed_tags.is_empty() {
        writeln!(formatter)?;
        with_content_format.write(formatter, |formatter| writeln!(formatter, "Changed tags:"))?;
        for (name, (from_target, to_target)) in changed_tags {
            with_content_format.write(formatter, |formatter| {
                writeln!(formatter, "{name}:")?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    to_target,
                    true,
                    None,
                )?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    from_target,
                    false,
                    None,
                )
            })?;
        }
        writeln!(formatter)?;
    }

    let changed_remote_bookmarks = diff_named_remote_refs(
        from_repo.view().all_remote_bookmarks(),
        to_repo.view().all_remote_bookmarks(),
    )
    // Skip updates to the local git repo, since they should typically be covered in
    // local branches.
    .filter(|((_, remote_name), _)| !jj_lib::git::is_special_git_remote(remote_name))
    .collect_vec();
    if !changed_remote_bookmarks.is_empty() {
        writeln!(formatter)?;
        with_content_format.write(formatter, |formatter| {
            writeln!(formatter, "Changed remote bookmarks:")
        })?;
        let get_remote_ref_prefix = |remote_ref: &RemoteRef| match remote_ref.state {
            RemoteRefState::New => "untracked",
            RemoteRefState::Tracking => "tracked",
        };
        for ((name, remote_name), (from_ref, to_ref)) in changed_remote_bookmarks {
            with_content_format.write(formatter, |formatter| {
                writeln!(formatter, "{name}@{remote_name}:")?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    &to_ref.target,
                    true,
                    Some(get_remote_ref_prefix(to_ref)),
                )?;
                write_ref_target_summary(
                    formatter,
                    current_repo,
                    commit_summary_template,
                    &from_ref.target,
                    false,
                    Some(get_remote_ref_prefix(from_ref)),
                )
            })?;
        }
    }

    Ok(())
}

/// Writes a summary for the given `ModifiedChange`.
fn write_modified_change_summary(
    formatter: &mut dyn Formatter,
    commit_summary_template: &TemplateRenderer<Commit>,
    modified_change: &ModifiedChange,
) -> Result<(), std::io::Error> {
    for commit in &modified_change.added_commits {
        formatter.with_label("diff", |formatter| write!(formatter.labeled("added"), "+"))?;
        write!(formatter, " ")?;
        commit_summary_template.format(commit, formatter)?;
        writeln!(formatter)?;
    }
    for commit in &modified_change.removed_commits {
        formatter.with_label("diff", |formatter| {
            write!(formatter.labeled("removed"), "-")
        })?;
        write!(formatter, " ")?;
        commit_summary_template.format(commit, formatter)?;
        writeln!(formatter)?;
    }
    Ok(())
}

/// Writes a summary for the given `RefTarget`.
fn write_ref_target_summary(
    formatter: &mut dyn Formatter,
    repo: &dyn Repo,
    commit_summary_template: &TemplateRenderer<Commit>,
    ref_target: &RefTarget,
    added: bool,
    prefix: Option<&str>,
) -> Result<(), CommandError> {
    let write_prefix = |formatter: &mut dyn Formatter,
                        added: bool,
                        prefix: Option<&str>|
     -> Result<(), CommandError> {
        formatter.with_label("diff", |formatter| {
            write!(
                formatter.labeled(if added { "added" } else { "removed" }),
                "{}",
                if added { "+" } else { "-" }
            )
        })?;
        write!(formatter, " ")?;
        if let Some(prefix) = prefix {
            write!(formatter, "{prefix} ")?;
        }
        Ok(())
    };
    if ref_target.is_absent() {
        write_prefix(formatter, added, prefix)?;
        writeln!(formatter, "(absent)")?;
    } else if ref_target.has_conflict() {
        for commit_id in ref_target.added_ids() {
            write_prefix(formatter, added, prefix)?;
            write!(formatter, "(added) ")?;
            let commit = repo.store().get_commit(commit_id)?;
            commit_summary_template.format(&commit, formatter)?;
            writeln!(formatter)?;
        }
        for commit_id in ref_target.removed_ids() {
            write_prefix(formatter, added, prefix)?;
            write!(formatter, "(removed) ")?;
            let commit = repo.store().get_commit(commit_id)?;
            commit_summary_template.format(&commit, formatter)?;
            writeln!(formatter)?;
        }
    } else {
        write_prefix(formatter, added, prefix)?;
        let commit_id = ref_target.as_normal().unwrap();
        let commit = repo.store().get_commit(commit_id)?;
        commit_summary_template.format(&commit, formatter)?;
        writeln!(formatter)?;
    }
    Ok(())
}

/// Returns the change IDs of the parents of the given `modified_change`, which
/// are the parents of all newly added commits for the change, or the parents of
/// all removed commits if there are no added commits.
fn get_parent_changes(
    modified_change: &ModifiedChange,
    commit_id_change_id_map: &HashMap<CommitId, ChangeId>,
) -> Vec<ChangeId> {
    // TODO: how should we handle multiple added or removed commits?
    if !modified_change.added_commits.is_empty() {
        modified_change
            .added_commits
            .iter()
            .flat_map(|commit| commit.parent_ids())
            .filter_map(|parent_id| commit_id_change_id_map.get(parent_id).cloned())
            .unique()
            .collect_vec()
    } else {
        modified_change
            .removed_commits
            .iter()
            .flat_map(|commit| commit.parent_ids())
            .filter_map(|parent_id| commit_id_change_id_map.get(parent_id).cloned())
            .unique()
            .collect_vec()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModifiedChange {
    added_commits: Vec<Commit>,
    removed_commits: Vec<Commit>,
}

/// Compute the changes in commits between two operations, returned as a
/// `HashMap` from `ChangeId` to a `ModifiedChange` struct containing the added
/// and removed commits for the change ID.
fn compute_operation_commits_diff(
    repo: &dyn Repo,
    from_repo: &ReadonlyRepo,
    to_repo: &ReadonlyRepo,
) -> Result<IndexMap<ChangeId, ModifiedChange>, CommandError> {
    let mut changes: IndexMap<ChangeId, ModifiedChange> = IndexMap::new();

    let from_heads = from_repo.view().heads().iter().cloned().collect_vec();
    let to_heads = to_repo.view().heads().iter().cloned().collect_vec();

    // Find newly added commits in `to_repo` which were not present in
    // `from_repo`.
    for commit in revset::walk_revs(repo, &to_heads, &from_heads)?
        .iter()
        .commits(repo.store())
    {
        let commit = commit?;
        let modified_change = changes
            .entry(commit.change_id().clone())
            .or_insert_with(|| ModifiedChange {
                added_commits: vec![],
                removed_commits: vec![],
            });
        modified_change.added_commits.push(commit);
    }

    // Find commits which were hidden in `to_repo`.
    for commit in revset::walk_revs(repo, &from_heads, &to_heads)?
        .iter()
        .commits(repo.store())
    {
        let commit = commit?;
        let modified_change = changes
            .entry(commit.change_id().clone())
            .or_insert_with(|| ModifiedChange {
                added_commits: vec![],
                removed_commits: vec![],
            });
        modified_change.removed_commits.push(commit);
    }

    Ok(changes)
}

/// Displays the diffs of a modified change. The output differs based on the
/// commits added and removed for the change.
/// If there is a single added and removed commit, the diff is shown between the
/// removed commit and the added commit rebased onto the removed commit's
/// parents. If there is only a single added or single removed commit, the diff
/// is shown of that commit's contents.
fn show_change_diff(
    ui: &Ui,
    formatter: &mut dyn Formatter,
    diff_renderer: &DiffRenderer,
    change: &ModifiedChange,
    width: usize,
) -> Result<(), CommandError> {
    match (&*change.removed_commits, &*change.added_commits) {
        (predecessors @ ([] | [_]), [commit]) => {
            // New or modified change. If the modification involved a rebase,
            // show diffs from the rebased tree.
            diff_renderer.show_inter_diff(
                ui,
                formatter,
                predecessors,
                commit,
                &EverythingMatcher,
                width,
            )?;
        }
        ([commit], []) => {
            // TODO: Should we show a reverse diff?
            diff_renderer.show_patch(ui, formatter, commit, &EverythingMatcher, width)?;
        }
        ([_, _, ..], _) | (_, [_, _, ..]) => {}
        ([], []) => panic!("ModifiedChange should have at least one entry"),
    }
    Ok(())
}
