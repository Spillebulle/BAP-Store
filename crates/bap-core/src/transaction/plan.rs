//! Operations to steps. Each source says how its own operations are carried
//! out; this module orders those steps so that one helper run covers as many
//! of them as possible, and joins consecutive package-manager calls into one
//! (`pacman -S a`, `pacman -S b` becomes `pacman -S a b`), which is both one
//! password and one dependency resolution instead of three.
//!
//! The order is refresh steps, then root steps grouped by source, then
//! session steps. Within a source the order the source gave is kept. A source
//! whose steps must alternate (download as the user, then `pacman -U` as
//! root) is not split across lanes; its steps stay together at the end, and
//! the runner asks for the password once more for them. The alternative,
//! hoisting that `-U` into the shared root run, would install a file that
//! has not been downloaded yet.

use crate::model::*;
use crate::{Result, Store};

/// The sentence the Updates page shows beside a partial pacman update.
pub const PARTIAL_UPGRADE_NOTICE: &str = "Arch does not support partial upgrades. Updating only some packages can break others; Update all is the safe choice.";

/// Build the plan for `ops`: ask each source for its steps, order them into
/// lanes, batch what can be batched.
pub fn build(store: &Store, ops: &[Op]) -> Result<Plan> {
    let mut gathered = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        let kind = source_of(op);
        let Some(source) = store.source(kind) else {
            return Err(crate::Error::new(format!(
                "{} is not a source on this machine.",
                kind.label()
            )));
        };
        for step in source.plan(op)? {
            gathered.push(Gathered {
                step,
                op: index,
                refresh: matches!(op, Op::Refresh { .. }),
            });
        }
    }
    let ordered = order(gathered, ops);
    let steps = batch(ordered, ops);
    Ok(Plan {
        id: new_id(),
        ops: ops.to_vec(),
        steps,
    })
}

/// What the page should say beside a plan before it runs. Today that is one
/// sentence: a pacman update that leaves other pending pacman updates out is
/// a partial upgrade, which Arch does not support. The sentence is shown
/// whenever the plan updates some pacman packages but not all of them, and
/// also when the pending list cannot be read, because then nobody can say
/// the update is complete.
pub fn notices(store: &Store, ops: &[Op]) -> Vec<String> {
    let mut notices = Vec::new();
    let updates_all = ops.iter().any(|op| {
        matches!(
            op,
            Op::UpdateAll {
                source: SourceKind::Pacman
            }
        )
    });
    let chosen: Vec<&str> = ops
        .iter()
        .filter_map(|op| match op {
            Op::Update { package } if package.source == SourceKind::Pacman => {
                Some(package.id.as_str())
            }
            _ => None,
        })
        .collect();
    if !chosen.is_empty() && !updates_all {
        let pending = store
            .source(SourceKind::Pacman)
            .and_then(|s| s.updates().ok());
        let complete = pending.as_ref().is_some_and(|p| {
            !p.is_empty() && p.iter().all(|u| chosen.contains(&u.package.id.as_str()))
        });
        if !complete {
            notices.push(PARTIAL_UPGRADE_NOTICE.to_string());
        }
    }
    notices
}

pub fn new_id() -> String {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("plan-{t}")
}

fn source_of(op: &Op) -> SourceKind {
    match op {
        Op::Install { package } | Op::Remove { package } | Op::Update { package } => package.source,
        Op::UpdateAll { source } | Op::Refresh { source } => *source,
    }
}

struct Gathered {
    step: Step,
    /// Index into the plan's ops, so a batched title can say what the batch
    /// does and a source's steps can be told apart from another's.
    op: usize,
    refresh: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Lane {
    Refresh,
    Root,
    Session,
}

/// Stable ordering by (lane, source rank, sequence). The rank of a source is
/// where it first appears in `ops`, so the user's order still shows through
/// where nothing forces another.
fn order(gathered: Vec<Gathered>, ops: &[Op]) -> Vec<Gathered> {
    let ranks: Vec<SourceKind> = ops
        .iter()
        .map(source_of)
        .fold(Vec::new(), |mut ranks, kind| {
            if !ranks.contains(&kind) {
                ranks.push(kind);
            }
            ranks
        });
    let bound: Vec<SourceKind> = ranks
        .iter()
        .copied()
        .filter(|kind| order_bound(gathered.iter().filter(|g| g.step.source == *kind)))
        .collect();
    let mut indexed: Vec<(Lane, usize, usize, Gathered)> = gathered
        .into_iter()
        .enumerate()
        .map(|(sequence, g)| {
            let lane = if g.refresh {
                Lane::Refresh
            } else if bound.contains(&g.step.source) {
                Lane::Session
            } else if g.step.needs_root {
                Lane::Root
            } else {
                Lane::Session
            };
            let rank = ranks
                .iter()
                .position(|k| *k == g.step.source)
                .unwrap_or(ranks.len());
            (lane, rank, sequence, g)
        })
        .collect();
    indexed.sort_by_key(|(lane, rank, sequence, _)| (*lane, *rank, *sequence));
    indexed.into_iter().map(|(_, _, _, g)| g).collect()
}

/// A source whose non-refresh steps put a root step after a session step
/// depends on that order (a build, then an install of what was built).
fn order_bound<'a>(steps: impl Iterator<Item = &'a Gathered>) -> bool {
    let mut seen_session = false;
    for g in steps.filter(|g| !g.refresh) {
        if !g.step.needs_root {
            seen_session = true;
        } else if seen_session {
            return true;
        }
    }
    false
}

/// One batchable shape: `program verb` where the arguments after the verb are
/// options, then `head_positionals` fixed arguments, then package names.
/// Two consecutive steps with the same program, verb, options, fixed
/// arguments, environment, working directory and privilege become one step
/// with the names joined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchRule {
    pub program: &'static str,
    pub verb: &'static str,
    /// Positional arguments that are part of the head rather than names:
    /// the remote in `flatpak install flathub a b`.
    pub head_positionals: usize,
}

/// Every command shape the planner may batch. A program not in this table is
/// never joined with another step, whatever its arguments.
pub const BATCHABLE: &[BatchRule] = &[
    BatchRule {
        program: "pacman",
        verb: "-S",
        head_positionals: 0,
    },
    BatchRule {
        program: "pacman",
        verb: "-Sy",
        head_positionals: 0,
    },
    BatchRule {
        program: "pacman",
        verb: "-Syu",
        head_positionals: 0,
    },
    BatchRule {
        program: "pacman",
        verb: "-R",
        head_positionals: 0,
    },
    BatchRule {
        program: "pacman",
        verb: "-Rs",
        head_positionals: 0,
    },
    BatchRule {
        program: "pacman",
        verb: "-Rns",
        head_positionals: 0,
    },
    BatchRule {
        program: "apt-get",
        verb: "install",
        head_positionals: 0,
    },
    BatchRule {
        program: "apt-get",
        verb: "remove",
        head_positionals: 0,
    },
    BatchRule {
        program: "dnf",
        verb: "install",
        head_positionals: 0,
    },
    BatchRule {
        program: "dnf",
        verb: "remove",
        head_positionals: 0,
    },
    BatchRule {
        program: "snap",
        verb: "install",
        head_positionals: 0,
    },
    BatchRule {
        program: "snap",
        verb: "remove",
        head_positionals: 0,
    },
    BatchRule {
        program: "flatpak",
        verb: "install",
        head_positionals: 1,
    },
    BatchRule {
        program: "flatpak",
        verb: "uninstall",
        head_positionals: 0,
    },
    BatchRule {
        program: "flatpak",
        verb: "update",
        head_positionals: 0,
    },
];

/// A command split into the part that must match for batching and the names
/// that are joined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchKey {
    pub program: String,
    pub head: Vec<String>,
    pub names: Vec<String>,
}

/// How `command` would batch, or `None` when it is not a batchable shape.
/// The verb is the first argument that is not an option; options before or
/// after it are part of the head; once a name has been seen, every later
/// argument must be a name too, otherwise the command is left alone rather
/// than reordered.
pub fn batch_key(command: &Command) -> Option<BatchKey> {
    let mut head = Vec::new();
    let mut names = Vec::new();
    let mut rule: Option<&BatchRule> = None;
    let mut fixed_left = 0;
    for arg in &command.args {
        match rule {
            None => {
                if arg.starts_with('-') && command.program != "pacman" {
                    head.push(arg.clone());
                    continue;
                }
                let found = BATCHABLE
                    .iter()
                    .find(|r| r.program == command.program && r.verb == arg)?;
                rule = Some(found);
                fixed_left = found.head_positionals;
                head.push(arg.clone());
            }
            Some(_) => {
                if arg.starts_with('-') {
                    if !names.is_empty() {
                        return None;
                    }
                    head.push(arg.clone());
                } else if fixed_left > 0 {
                    fixed_left -= 1;
                    head.push(arg.clone());
                } else {
                    names.push(arg.clone());
                }
            }
        }
    }
    rule?;
    if fixed_left > 0 {
        return None;
    }
    Some(BatchKey {
        program: command.program.clone(),
        head,
        names,
    })
}

fn batch(ordered: Vec<Gathered>, ops: &[Op]) -> Vec<Step> {
    struct Pending {
        step: Step,
        key: Option<BatchKey>,
        ops: Vec<usize>,
    }
    let mut out: Vec<Pending> = Vec::new();
    for g in ordered {
        let key = batch_key(&g.step.command);
        let joins = match (out.last(), key.as_ref()) {
            (Some(last), Some(key)) => last
                .key
                .as_ref()
                .is_some_and(|last_key| joinable(&last.step, &g.step, last_key, key)),
            _ => false,
        };
        if joins {
            let last = out.last_mut().expect("checked above");
            let key = key.expect("checked above");
            let joined = last.key.as_mut().expect("checked above");
            for name in key.names {
                if !joined.names.contains(&name) {
                    joined.names.push(name);
                }
            }
            last.step.weight = last.step.weight.saturating_add(g.step.weight);
            last.ops.push(g.op);
            last.step.command.args = joined
                .head
                .iter()
                .chain(joined.names.iter())
                .cloned()
                .collect();
            if joined.names.len() > 1 {
                last.step.title = batched_title(&last.ops, ops, joined.names.len());
            }
            continue;
        }
        out.push(Pending {
            step: g.step,
            key,
            ops: vec![g.op],
        });
    }
    out.into_iter().map(|p| p.step).collect()
}

fn joinable(a: &Step, b: &Step, ka: &BatchKey, kb: &BatchKey) -> bool {
    a.source == b.source
        && a.needs_root == b.needs_root
        && a.command.env == b.command.env
        && a.command.cwd == b.command.cwd
        && ka.program == kb.program
        && ka.head == kb.head
}

/// "Installing 3 packages", from what the ops that fed the batch asked for.
/// Mixed batches (an install and an update through the same `pacman -S`)
/// say "Installing", which is what pacman does in both cases.
fn batched_title(op_indexes: &[usize], ops: &[Op], count: usize) -> String {
    let mut removes = 0;
    let mut updates = 0;
    let mut refreshes = 0;
    for i in op_indexes {
        match ops.get(*i) {
            Some(Op::Remove { .. }) => removes += 1,
            Some(Op::Update { .. } | Op::UpdateAll { .. }) => updates += 1,
            Some(Op::Refresh { .. }) => refreshes += 1,
            _ => {}
        }
    }
    let n = op_indexes.len();
    let verb = if removes == n {
        "Removing"
    } else if updates == n {
        "Updating"
    } else if refreshes == n {
        "Refreshing"
    } else {
        "Installing"
    };
    format!("{verb} {count} packages")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Query, Source};

    /// A source that answers `plan` from a table, so the planner is tested
    /// without pacman or the network.
    struct Fake {
        kind: SourceKind,
        steps: fn(&Op) -> Vec<Step>,
        pending: Vec<&'static str>,
    }

    impl Source for Fake {
        fn kind(&self) -> SourceKind {
            self.kind
        }
        fn status(&self) -> SourceStatus {
            SourceStatus {
                kind: self.kind,
                available: true,
                reason: None,
                detail: None,
            }
        }
        fn search(&self, _: &Query) -> Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn installed(&self) -> Result<Vec<Package>> {
            Ok(Vec::new())
        }
        fn updates(&self) -> Result<Vec<Update>> {
            Ok(self
                .pending
                .iter()
                .map(|id| Update {
                    package: PackageRef {
                        source: self.kind,
                        id: id.to_string(),
                    },
                    name: id.to_string(),
                    kind: PackageKind::Package,
                    summary: None,
                    icon: None,
                    from: None,
                    to: "2".to_string(),
                    download_size: None,
                    published: None,
                    is_self: false,
                })
                .collect())
        }
        fn details(&self, id: &str) -> Result<Package> {
            Err(crate::Error::new(format!("{id} is not known.")))
        }
        fn plan(&self, op: &Op) -> Result<Vec<Step>> {
            Ok((self.steps)(op))
        }
    }

    fn step(source: SourceKind, title: &str, program: &str, args: &[&str], root: bool) -> Step {
        Step {
            source,
            title: title.to_string(),
            command: Command {
                program: program.to_string(),
                args: args.iter().map(|a| a.to_string()).collect(),
                env: Vec::new(),
                cwd: None,
            },
            needs_root: root,
            weight: 2,
        }
    }

    fn id_of(op: &Op) -> String {
        match op {
            Op::Install { package } | Op::Remove { package } | Op::Update { package } => {
                package.id.clone()
            }
            Op::UpdateAll { .. } => "all".to_string(),
            Op::Refresh { .. } => "refresh".to_string(),
        }
    }

    fn pacman_steps(op: &Op) -> Vec<Step> {
        let id = id_of(op);
        match op {
            Op::Install { .. } | Op::Update { .. } => vec![step(
                SourceKind::Pacman,
                &format!("Installing {id}"),
                "pacman",
                &["-S", "--noconfirm", "--needed", &id],
                true,
            )],
            Op::Remove { .. } => vec![step(
                SourceKind::Pacman,
                &format!("Removing {id}"),
                "pacman",
                &["-Rs", "--noconfirm", &id],
                true,
            )],
            Op::UpdateAll { .. } => vec![step(
                SourceKind::Pacman,
                "Updating everything",
                "pacman",
                &["-Syu", "--noconfirm"],
                true,
            )],
            Op::Refresh { .. } => vec![step(
                SourceKind::Pacman,
                "Refreshing",
                "pacman",
                &["-Sy"],
                true,
            )],
        }
    }

    fn flatpak_steps(op: &Op) -> Vec<Step> {
        let id = id_of(op);
        let remote = if id.ends_with("beta") {
            "flathub-beta"
        } else {
            "flathub"
        };
        vec![step(
            SourceKind::Flatpak,
            &format!("Installing {id}"),
            "flatpak",
            &["install", "--system", "-y", remote, &id],
            true,
        )]
    }

    fn aur_steps(op: &Op) -> Vec<Step> {
        let id = id_of(op);
        vec![
            step(
                SourceKind::Aur,
                "Installing build dependencies",
                "pacman",
                &["-S", "--needed", "--asdeps", "cmake"],
                true,
            ),
            step(
                SourceKind::Aur,
                &format!("Building {id}"),
                "makepkg",
                &["-si"],
                false,
            ),
        ]
    }

    fn github_steps(op: &Op) -> Vec<Step> {
        let id = id_of(op);
        vec![
            step(
                SourceKind::Github,
                &format!("Downloading {id}"),
                "curl",
                &["-o", "x.pkg.tar.zst"],
                false,
            ),
            step(
                SourceKind::Github,
                &format!("Installing {id}"),
                "pacman",
                &["-U", "/tmp/x.pkg.tar.zst"],
                true,
            ),
        ]
    }

    fn store(sources: Vec<Box<dyn Source>>) -> Store {
        Store {
            system: crate::system::from_os_release("ID=arch\n"),
            sources,
        }
    }

    fn fake(kind: SourceKind, steps: fn(&Op) -> Vec<Step>) -> Box<dyn Source> {
        Box::new(Fake {
            kind,
            steps,
            pending: Vec::new(),
        })
    }

    fn install(kind: SourceKind, id: &str) -> Op {
        Op::Install {
            package: PackageRef {
                source: kind,
                id: id.to_string(),
            },
        }
    }

    fn update(kind: SourceKind, id: &str) -> Op {
        Op::Update {
            package: PackageRef {
                source: kind,
                id: id.to_string(),
            },
        }
    }

    fn remove(kind: SourceKind, id: &str) -> Op {
        Op::Remove {
            package: PackageRef {
                source: kind,
                id: id.to_string(),
            },
        }
    }

    fn args(step: &Step) -> Vec<&str> {
        step.command.args.iter().map(String::as_str).collect()
    }

    #[test]
    fn three_pacman_installs_become_one_step() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let ops = [
            install(SourceKind::Pacman, "steam"),
            install(SourceKind::Pacman, "lutris"),
            install(SourceKind::Pacman, "wine"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(plan.steps.len(), 1);
        let s = &plan.steps[0];
        assert_eq!(
            args(s),
            ["-S", "--noconfirm", "--needed", "steam", "lutris", "wine"]
        );
        assert_eq!(s.title, "Installing 3 packages");
        assert!(s.needs_root);
        assert_eq!(s.weight, 6, "weights add up");
        assert_eq!(plan.ops.len(), 3);
        assert!(plan.id.starts_with("plan-"));
    }

    #[test]
    fn a_single_step_keeps_its_own_title() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let plan = build(&store, &[install(SourceKind::Pacman, "steam")]).unwrap();
        assert_eq!(plan.steps[0].title, "Installing steam");
    }

    #[test]
    fn installs_and_removes_do_not_mix_and_removes_say_so() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let ops = [
            install(SourceKind::Pacman, "a"),
            remove(SourceKind::Pacman, "b"),
            remove(SourceKind::Pacman, "c"),
            install(SourceKind::Pacman, "d"),
        ];
        let plan = build(&store, &ops).unwrap();
        let titles: Vec<&str> = plan.steps.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Installing a", "Removing 2 packages", "Installing d"]
        );
        assert_eq!(args(&plan.steps[1]), ["-Rs", "--noconfirm", "b", "c"]);
    }

    #[test]
    fn updates_through_pacman_s_are_titled_updating() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let ops = [
            update(SourceKind::Pacman, "a"),
            update(SourceKind::Pacman, "b"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(plan.steps[0].title, "Updating 2 packages");
    }

    #[test]
    fn a_repeated_name_is_not_listed_twice() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let ops = [
            install(SourceKind::Pacman, "a"),
            install(SourceKind::Pacman, "a"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(args(&plan.steps[0]), ["-S", "--noconfirm", "--needed", "a"]);
        assert_eq!(plan.steps[0].title, "Installing a");
    }

    #[test]
    fn flatpak_batches_within_a_remote_only() {
        let store = store(vec![fake(SourceKind::Flatpak, flatpak_steps)]);
        let ops = [
            install(SourceKind::Flatpak, "org.gimp.GIMP"),
            install(SourceKind::Flatpak, "org.inkscape.Inkscape"),
            install(SourceKind::Flatpak, "org.gimp.GIMP.beta"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(
            args(&plan.steps[0]),
            [
                "install",
                "--system",
                "-y",
                "flathub",
                "org.gimp.GIMP",
                "org.inkscape.Inkscape"
            ]
        );
        assert_eq!(plan.steps[0].title, "Installing 2 packages");
        assert_eq!(
            args(&plan.steps[1]),
            [
                "install",
                "--system",
                "-y",
                "flathub-beta",
                "org.gimp.GIMP.beta"
            ]
        );
    }

    #[test]
    fn sources_are_grouped_so_interleaved_ops_still_batch() {
        let store = store(vec![
            fake(SourceKind::Pacman, pacman_steps),
            fake(SourceKind::Flatpak, flatpak_steps),
        ]);
        let ops = [
            install(SourceKind::Flatpak, "org.gimp.GIMP"),
            install(SourceKind::Pacman, "a"),
            install(SourceKind::Flatpak, "org.inkscape.Inkscape"),
            install(SourceKind::Pacman, "b"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(
            plan.steps[0].source,
            SourceKind::Flatpak,
            "first appearance wins"
        );
        assert_eq!(plan.steps[1].source, SourceKind::Pacman);
        assert_eq!(
            args(&plan.steps[1]),
            ["-S", "--noconfirm", "--needed", "a", "b"]
        );
    }

    #[test]
    fn refresh_first_then_root_then_session() {
        let store = store(vec![
            fake(SourceKind::Pacman, pacman_steps),
            fake(SourceKind::Aur, aur_steps),
        ]);
        let ops = [
            install(SourceKind::Aur, "paru"),
            install(SourceKind::Pacman, "a"),
            Op::Refresh {
                source: SourceKind::Pacman,
            },
        ];
        let plan = build(&store, &ops).unwrap();
        let shape: Vec<(SourceKind, &str, bool)> = plan
            .steps
            .iter()
            .map(|s| (s.source, s.command.program.as_str(), s.needs_root))
            .collect();
        assert_eq!(
            shape,
            [
                (SourceKind::Pacman, "pacman", true),
                (SourceKind::Aur, "pacman", true),
                (SourceKind::Pacman, "pacman", true),
                (SourceKind::Aur, "makepkg", false),
            ]
        );
        assert_eq!(args(&plan.steps[0]), ["-Sy"]);
        assert_eq!(plan.steps[1].title, "Installing build dependencies");
    }

    #[test]
    fn a_download_then_install_source_is_not_split_across_lanes() {
        let store = store(vec![
            fake(SourceKind::Pacman, pacman_steps),
            fake(SourceKind::Github, github_steps),
        ]);
        let ops = [
            install(SourceKind::Github, "bap-store"),
            install(SourceKind::Pacman, "a"),
        ];
        let plan = build(&store, &ops).unwrap();
        let shape: Vec<(&str, bool)> = plan
            .steps
            .iter()
            .map(|s| (s.command.program.as_str(), s.needs_root))
            .collect();
        assert_eq!(shape, [("pacman", true), ("curl", false), ("pacman", true)]);
        assert_eq!(
            args(&plan.steps[2]),
            ["-U", "/tmp/x.pkg.tar.zst"],
            "the -U is never batched"
        );
    }

    #[test]
    fn different_environments_do_not_batch() {
        fn steps(op: &Op) -> Vec<Step> {
            let id = id_of(op);
            let mut s = step(
                SourceKind::Apt,
                &format!("Installing {id}"),
                "apt-get",
                &["install", "-y", &id],
                true,
            );
            if id == "b" {
                s.command
                    .env
                    .push(("DEBIAN_FRONTEND".to_string(), "noninteractive".to_string()));
            }
            vec![s]
        }
        let store = store(vec![fake(SourceKind::Apt, steps)]);
        let ops = [
            install(SourceKind::Apt, "a"),
            install(SourceKind::Apt, "b"),
            install(SourceKind::Apt, "c"),
        ];
        let plan = build(&store, &ops).unwrap();
        assert_eq!(plan.steps.len(), 3);
    }

    #[test]
    fn an_unknown_source_is_an_error_sentence() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        let err = build(&store, &[install(SourceKind::Snap, "code")]).unwrap_err();
        assert_eq!(err.message, "Snap is not a source on this machine.");
    }

    #[test]
    fn the_batch_table_reads_commands_as_expected() {
        let cmd = |program: &str, args: &[&str]| Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            env: Vec::new(),
            cwd: None,
        };
        let key = batch_key(&cmd("pacman", &["-S", "--noconfirm", "--needed", "a", "b"])).unwrap();
        assert_eq!(key.head, ["-S", "--noconfirm", "--needed"]);
        assert_eq!(key.names, ["a", "b"]);

        let key = batch_key(&cmd(
            "flatpak",
            &["--system", "install", "-y", "flathub", "org.x"],
        ))
        .unwrap();
        assert_eq!(key.head, ["--system", "install", "-y", "flathub"]);
        assert_eq!(key.names, ["org.x"]);

        let key = batch_key(&cmd("apt-get", &["-y", "install", "a"])).unwrap();
        assert_eq!(key.head, ["-y", "install"]);

        assert_eq!(
            batch_key(&cmd("pacman", &["-U", "/x.pkg.tar.zst"])),
            None,
            "-U is not in the table"
        );
        assert_eq!(
            batch_key(&cmd("pacman", &["-S", "a", "--needed"])),
            None,
            "an option after a name is left alone"
        );
        assert_eq!(
            batch_key(&cmd("flatpak", &["install", "-y"])),
            None,
            "no remote, nothing to key on"
        );
        assert_eq!(batch_key(&cmd("makepkg", &["-si"])), None);
        assert_eq!(
            batch_key(&cmd("pacman", &["--noconfirm", "-S", "a"])),
            None,
            "pacman's verb comes first"
        );
        assert!(
            BATCHABLE
                .iter()
                .all(|r| !r.program.is_empty() && !r.verb.is_empty())
        );
    }

    #[test]
    fn a_partial_pacman_update_carries_the_notice() {
        let pacman = Box::new(Fake {
            kind: SourceKind::Pacman,
            steps: pacman_steps,
            pending: vec!["a", "b", "c"],
        });
        let store = store(vec![pacman]);
        assert_eq!(
            notices(&store, &[update(SourceKind::Pacman, "a")]),
            [PARTIAL_UPGRADE_NOTICE.to_string()]
        );
        assert_eq!(
            notices(
                &store,
                &[
                    update(SourceKind::Pacman, "a"),
                    update(SourceKind::Pacman, "b"),
                    update(SourceKind::Pacman, "c")
                ]
            ),
            Vec::<String>::new(),
            "every pending update ticked is not partial"
        );
        assert_eq!(
            notices(
                &store,
                &[
                    update(SourceKind::Pacman, "a"),
                    Op::UpdateAll {
                        source: SourceKind::Pacman
                    }
                ]
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            notices(&store, &[install(SourceKind::Pacman, "a")]),
            Vec::<String>::new()
        );
        assert_eq!(
            notices(&store, &[update(SourceKind::Flatpak, "org.x")]),
            Vec::<String>::new()
        );
        assert!(!PARTIAL_UPGRADE_NOTICE.contains('\u{2014}'));
    }

    #[test]
    fn an_unreadable_pending_list_still_warns() {
        let store = store(vec![fake(SourceKind::Pacman, pacman_steps)]);
        assert_eq!(notices(&store, &[update(SourceKind::Pacman, "a")]).len(), 1);
    }
}
