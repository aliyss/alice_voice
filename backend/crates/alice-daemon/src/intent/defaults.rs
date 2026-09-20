//! The intents a fresh installation starts with.
//!
//! An installation without an intent answers every message with "I could
//! not match that to an intent", so the daemon seeds a catalog that shows
//! what an intent is and covers the things a user says to a machine: open
//! an application, move around the windows and the workspaces, change the
//! sound, take a screenshot, look something up, ask about the machine.
//!
//! Every field of an intent belongs to the user, so the catalog is a
//! starting point that the settings page edits. The commands are written
//! for a Wayland session that runs Hyprland with PipeWire and Network
//! Manager, and they are the commands of the machine this project runs
//! on. A session with other tools writes its own commands in the same
//! place; `frontend-web/README.md` lists the alternatives.
//!
//! Two rules keep the catalog small and true:
//!
//! - The catalog holds one intent less than
//!   [`crate::resolver::prompt::MAX_DECISION_OPTIONS`], because the
//!   llama.cpp resolver offers one letter per option and spends the last
//!   letter on "none of these". A longer catalog would make that engine
//!   refuse every message.
//! - A value the user says is a value the machine holds. An entity whose
//!   values come from the machine is a script entity, so the list is as
//!   live as the machine, and a fixed list is a closed entity. An intent
//!   whose value never changes — the terminal of the session — names the
//!   program in its command instead, because a value the user has to
//!   supply for a command that works without it is a question too many.
//! - A default that kills or switches something off is no default. The
//!   catalog closes a window, because a window is back in one gesture,
//!   and leaves the programs alone with `pkill`.
//!
//!   An intent that closes an application competed with the intent that
//!   opens one: both act on the same value, so the phrases of each read
//!   the message of the other, and `open firefox` reached the closing
//!   intent through the phrase `close firefox`. The catalog therefore
//!   keeps the intent a user says more often and leaves the other one to
//!   the user, who knows which programs may be stopped.

use chrono::{DateTime, Utc};

use alice_core::dto::EntityKindDto;

use crate::intent::input::{EntityInput, IntentInput};
use crate::intent::service::IntentService;
use crate::intent::SaveIntentError;

/// The dispatcher of the compositor, with the library path of the daemon
/// taken off the environment of the command.
///
/// The daemon may run with a library path of its own: the build shell of
/// this project puts an older `libstdc++` on it, and a program that needs
/// a newer one then fails to start (`hyprctl: version 'GLIBCXX_3.4.35'
/// not found`). The compositor, its lock screen, and every program of that
/// kind link `libstdc++`, so a command that runs one takes the variable
/// off. A session that starts the daemon without such a path changes
/// nothing: taking off a variable that is not there does nothing.
const HYPRCTL: &str = "env -u LD_LIBRARY_PATH hyprctl dispatch";

/// The script that lists the applications of the machine.
///
/// It reads the `Exec` line of every desktop entry and writes the name of
/// the program the entry starts, so the list holds `firefox` and not the
/// path of the store the machine keeps Firefox in, and not the whole
/// command line of a launcher. The name is what a shell resolves through
/// the path, which is what the command of the intent needs.
///
/// The script reads the directories the desktop specification names: the
/// ones the user added, the ones of a Nix profile, and the ones of the
/// system. The store of a Nix profile is reached through a link, so the
/// walk follows links. An entry that the desktop hides (`NoDisplay`,
/// `Hidden`) is a protocol handler, a portal, or a second name of a
/// program that already stands in the list, so the script skips it, and
/// an entry that starts a shell is not an application either.
///
/// A name is read without a regular expression: the last word of the path
/// of the program is the name, a leading dot of a name of the store goes,
/// and a `-wrapped` suffix goes with it, so `.yuzu-wrapped` is `yuzu`.
pub const APPLICATIONS_SCRIPT: &str = "find -L \
\"${XDG_DATA_HOME:-$HOME/.local/share}\" \
\"$HOME/.nix-profile/share\" \
\"/etc/profiles/per-user/$(id -un)/share\" \
/run/current-system/sw/share \
$(printf '%s' \"${XDG_DATA_DIRS:-/usr/local/share:/usr/share}\" | tr ':' ' ') \
-maxdepth 2 -path '*/applications/*.desktop' 2>/dev/null \
| xargs -r -d '\\n' grep -LiE '^(NoDisplay|Hidden)=true' 2>/dev/null \
| xargs -r -d '\\n' grep -lE '^Type=Application' 2>/dev/null \
| xargs -r -d '\\n' grep -h '^Exec=' 2>/dev/null \
| sed 's/^Exec=//; s/%[A-Za-z]//g' \
| awk '{ i=1; while (i<=NF && ($i==\"env\" || index($i,\"=\")>0)) i++; \
if (i<=NF) { p=$i; n=split(p, part, \"/\"); p=part[n]; \
if (substr(p,1,1)==\".\") p=substr(p,2); \
if (length(p)>8 && substr(p,length(p)-7)==\"-wrapped\") p=substr(p,1,length(p)-8); \
if (p!~/^(sh|bash|dash|zsh|env)$/) print p } }' \
| sort -u";

/// The script that lists the folders of the home directory.
///
/// The value of the entity is the name of the folder and not its path,
/// because the name is what the user says: `Downloads` is the entry of
/// `open my downloads`, and the command builds the path of the home
/// around it. A hidden directory is the configuration of a program, so
/// the script leaves it out.
///
/// The command opens the folder with the file manager of the session,
/// which is `yazi` here: a terminal program that the desktop entry of the
/// machine names as the handler of a directory, so the command starts a
/// terminal for it. `xdg-open` would find that entry and fail, because the
/// daemon runs it without a terminal. A session whose file manager is a
/// window of its own writes the command of that window instead:
/// `xdg-open "$HOME/{folder}"`.
pub const FOLDERS_SCRIPT: &str =
    "find \"$HOME\" -maxdepth 1 -mindepth 1 -type d -not -name '.*' | sed 's|.*/||' | sort";

/// The name of the intent that opens an application.
pub const APPLICATIONS_INTENT: &str = "open application";

/// The command of the intent that opens an application.
///
/// The value is a program the shell resolves through the path of the
/// session, so it sits in double quotes: a name with a space in it stays
/// one argument. The program leaves the session of the daemon, so the
/// turn answers at once instead of waiting for the program to close, and
/// its output is dropped because a window has none.
///
/// The command works on every session, and it needs no launcher of the
/// compositor. A session that launches through its compositor writes the
/// launcher of that compositor here instead:
///
/// - a Hyprland that reads its configuration in Lua:
///   `hyprctl dispatch 'hl.dsp.exec_cmd("{applications}")'`
/// - a Hyprland that reads its configuration in its own language:
///   `hyprctl dispatch exec "{applications}"`
///
/// A launcher prints the name of the program it started, so a turn that
/// uses one answers with that name instead of with the output of the
/// program.
pub const APPLICATIONS_COMMAND: &str = "setsid -f \"{applications}\" >/dev/null 2>&1";

/// Build one intent of the catalog.
fn intent(
    name: &str,
    description: &str,
    command: &str,
    entities: Vec<EntityInput>,
    examples: &[&str],
) -> IntentInput {
    IntentInput {
        name: name.to_string(),
        description: description.to_string(),
        command: command.to_string(),
        entities,
        examples: examples
            .iter()
            .map(|example| (*example).to_string())
            .collect(),
    }
}

/// Build an entity whose value the message holds.
///
/// The value is required, because none of the commands of the catalog
/// does something useful without it: a launcher without a name, a search
/// without a query, and a file manager without a folder all do nothing.
/// The daemon asks the user for a missing value.
fn open_entity(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        kind: EntityKindDto::Open,
        values: Vec::new(),
        script: None,
        required: true,
    }
}

/// Build an entity that takes one value of a fixed list.
fn closed_entity(name: &str, values: &[&str]) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        kind: EntityKindDto::Closed,
        values: values.iter().map(|value| (*value).to_string()).collect(),
        script: None,
        required: true,
    }
}

/// Build an entity whose values a script answers with.
fn script_entity(name: &str, script: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        kind: EntityKindDto::Script,
        values: Vec::new(),
        script: Some(script.to_string()),
        required: true,
    }
}

/// The intents a fresh installation starts with.
pub fn example_intents() -> Vec<IntentInput> {
    vec![
        // An application.
        intent(
            APPLICATIONS_INTENT,
            "Open a desktop application of this machine by name.",
            APPLICATIONS_COMMAND,
            vec![script_entity("applications", APPLICATIONS_SCRIPT)],
            &[
                "launch firefox",
                "open obs",
                "open the browser",
                "run spotify",
                "start discord",
            ],
        ),
        intent(
            "open terminal",
            "Open a terminal window (foot, the terminal of this session).",
            "setsid -f foot >/dev/null 2>&1",
            Vec::new(),
            &["open a terminal", "start a shell", "open foot"],
        ),
        intent(
            "open folder",
            "Open a folder of the home directory in the file manager.",
            "setsid -f foot -e yazi \"$HOME/{folder}\" >/dev/null 2>&1",
            vec![script_entity("folder", FOLDERS_SCRIPT)],
            &["open my downloads", "open the projects folder"],
        ),
        // A window and a workspace.
        intent(
            "close window",
            "Close the window that has the focus.",
            &format!("{HYPRCTL} 'hl.dsp.window.close()'"),
            Vec::new(),
            &["close the window", "close this window"],
        ),
        intent(
            "switch workspace",
            "Move to one numbered workspace of the session.",
            &format!("ws=\"{{workspace}}\"; {HYPRCTL} \"hl.dsp.focus({{ workspace = $ws }})\""),
            vec![closed_entity(
                "workspace",
                &["1", "2", "3", "4", "5", "6", "7", "8", "9", "10"],
            )],
            &["switch to workspace 3", "go to workspace 1"],
        ),
        intent(
            "next workspace",
            "Move to the next workspace of the session.",
            &format!("{HYPRCTL} 'hl.dsp.focus({{ workspace = \"e+1\" }})'"),
            Vec::new(),
            &["next workspace", "go to the next workspace"],
        ),
        intent(
            "previous workspace",
            "Move to the previous workspace of the session.",
            &format!("{HYPRCTL} 'hl.dsp.focus({{ workspace = \"e-1\" }})'"),
            Vec::new(),
            &["previous workspace", "go back a workspace"],
        ),
        // The screen and the session.
        intent(
            "take screenshot",
            "Write a screenshot of the whole screen into the pictures folder.",
            "grim \"$HOME/Pictures/Screenshots/$(date +%Y%m%d-%H%M%S).png\"",
            Vec::new(),
            &["take a screenshot", "capture the screen"],
        ),
        intent(
            "copy screenshot",
            "Take a screenshot of a chosen area and put it on the clipboard.",
            "grim -g \"$(slurp)\" - | wl-copy",
            Vec::new(),
            &[
                "copy a screenshot of an area",
                "screenshot a part of the screen",
            ],
        ),
        intent(
            "lock screen",
            "Lock the session, so the password is needed again.",
            "env -u LD_LIBRARY_PATH hyprlock --immediate-render",
            Vec::new(),
            &["lock the screen", "lock my session"],
        ),
        // The sound.
        intent(
            "raise volume",
            "Turn the volume of the default output up by five percent.",
            "wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+",
            Vec::new(),
            &["turn it up", "louder", "volume up"],
        ),
        intent(
            "lower volume",
            "Turn the volume of the default output down by five percent.",
            "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-",
            Vec::new(),
            &["turn it down", "quieter", "volume down"],
        ),
        intent(
            "mute sound",
            "Mute the default output, or unmute it when it is muted.",
            "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle",
            Vec::new(),
            &["mute the sound", "unmute the sound", "silence the audio"],
        ),
        // What the machine knows.
        intent(
            "search web",
            "Search the web for a phrase and open the results in the browser.",
            "xdg-open \"https://duckduckgo.com/?q={query}\"",
            vec![open_entity("query")],
            &["search for hyprland binds", "look up the rust book"],
        ),
        intent(
            "open website",
            "Open a website in the browser.",
            "xdg-open \"https://{site}\"",
            vec![open_entity("site")],
            &["open github.com", "go to news.ycombinator.com"],
        ),
        intent(
            "get weather",
            "Read the current weather of one city.",
            "curl -s \"wttr.in/{city}?format=3\"",
            vec![open_entity("city")],
            &[
                "what is the weather in Berlin",
                "how is the weather today",
                "is it raining",
            ],
        ),
        intent(
            "tell time",
            "Read the date and the time of the machine.",
            "date \"+%A, %d %B %Y, %H:%M\"",
            Vec::new(),
            &["what time is it", "what day is it", "tell me the date"],
        ),
        intent(
            "show disk space",
            "Read how much space is left on the filesystem of the home directory.",
            "df -h / \"$HOME\" | tail -n +2 | sort -u",
            Vec::new(),
            &[
                "how much space is left",
                "show the disk space",
                "how much room is left",
                "show the free space",
            ],
        ),
        intent(
            "show memory",
            "Read how much of the memory is free.",
            "free -h",
            Vec::new(),
            &[
                "how much memory is free",
                "show the memory",
                "how much ram is left",
                "show the ram usage",
            ],
        ),
        intent(
            "show public address",
            "Read the public address of this connection.",
            "curl -s ifconfig.me",
            Vec::new(),
            &["what is my ip", "show the public address"],
        ),
        // The desktop around the machine.
        intent(
            "dismiss notifications",
            "Dismiss every notification that is on the screen.",
            "makoctl dismiss --all",
            Vec::new(),
            &["dismiss the notifications", "clear the notifications"],
        ),
        intent(
            "empty clipboard",
            "Empty the clipboard, so nothing is pasted by accident.",
            "wl-copy --clear",
            Vec::new(),
            &["empty the clipboard", "clear the clipboard"],
        ),
        intent(
            "wifi off",
            "Turn the wireless connection off.",
            "nmcli radio wifi off",
            Vec::new(),
            &["turn the wifi off", "disable the wifi"],
        ),
        intent(
            "wifi on",
            "Turn the wireless connection on.",
            "nmcli radio wifi on",
            Vec::new(),
            &["turn the wifi on", "enable the wifi"],
        ),
    ]
}

/// Add every example intent whose name is still free.
///
/// The function returns the names it added. An example that an older
/// installation already carries keeps the configuration of the user, so
/// the daemon never writes over an intent of its own making.
pub async fn add_missing(
    intents: &IntentService,
    at: DateTime<Utc>,
) -> Result<Vec<String>, SaveIntentError> {
    let stored = intents.list_intents().await?;
    let mut added: Vec<String> = Vec::new();

    for example in example_intents() {
        let taken = stored
            .iter()
            .any(|intent| intent.name.eq_ignore_ascii_case(&example.name));
        if taken {
            continue;
        }
        let name = example.name.clone();
        match intents.create_intent(example, at).await {
            Ok(intent) => added.push(intent.name),
            Err(SaveIntentError::NameTaken { .. }) => continue,
            Err(err) => return Err(err),
        }
        tracing::info!(intent = %name, "the daemon added an example intent");
    }
    Ok(added)
}

/// Add the example intents when the catalog is empty.
///
/// A catalog with an intent of the user stays as it is: only an
/// installation that answers nothing at all gets the example.
pub async fn seed_when_empty(
    intents: &IntentService,
    at: DateTime<Utc>,
) -> Result<Vec<String>, SaveIntentError> {
    if !intents.list_intents().await?.is_empty() {
        return Ok(Vec::new());
    }
    add_missing(intents, at).await
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::intent::command::render_command;
    use crate::intent::input::IntentInput;
    use crate::resolver::prompt::MAX_DECISION_OPTIONS;

    /// Build one example and normalize it, as a save would.
    fn normalized(name: &str) -> IntentInput {
        example_intents()
            .into_iter()
            .find(|intent| intent.name == name)
            .expect("the catalog holds the example")
            .normalize()
            .expect("the example passes the intent rules")
    }

    /// The brace groups of one text.
    fn brace_groups(text: &str) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('}') else {
                break;
            };
            found.push(after[..end].trim().to_string());
            rest = &after[end + 1..];
        }
        found
    }

    /// The brace groups of one text that name a value, as a placeholder
    /// of an intent does.
    ///
    /// A group that the command carries for a language of its own holds
    /// more than a name: a Lua table holds an assignment. A group of one
    /// word is a placeholder that no entity of the intent replaced.
    fn leftovers(text: &str) -> Vec<String> {
        brace_groups(text)
            .into_iter()
            .filter(|group| {
                !group.is_empty()
                    && !group.contains([' ', '='])
                    && group
                        .chars()
                        .all(|character| character.is_alphanumeric() || character == '_')
            })
            .collect()
    }

    /// Build the value the renderer writes for every entity of one
    /// intent: the first value of a fixed list, and a name of its own for
    /// a value the message holds.
    fn values_of(example: &IntentInput) -> BTreeMap<String, String> {
        example
            .entities
            .iter()
            .map(|entity| {
                let value = entity
                    .values
                    .first()
                    .cloned()
                    .unwrap_or_else(|| format!("zz{}zz", entity.name));
                (entity.name.clone(), value)
            })
            .collect()
    }

    /// The names of the entities of one example.
    fn entity_names(example: &IntentInput) -> Vec<String> {
        example
            .entities
            .iter()
            .map(|entity| entity.name.clone())
            .collect()
    }

    #[test]
    fn every_example_passes_the_intent_rules() {
        for example in example_intents() {
            let name = example.name.clone();
            example
                .normalize()
                .unwrap_or_else(|err| panic!("{name} is not a valid intent: {err}"));
        }
    }

    #[test]
    fn every_example_has_a_name_of_its_own() {
        let mut names: Vec<String> = Vec::new();
        for example in example_intents() {
            assert!(
                !names.contains(&example.name),
                "the name {} is twice",
                example.name
            );
            names.push(example.name);
        }
    }

    #[test]
    fn the_catalog_leaves_one_letter_for_no_intent() {
        // The llama.cpp resolver offers one letter per option and states
        // "none of these" as the last one, so the catalog may take every
        // letter but one.
        let catalog = example_intents().len();

        assert!(
            catalog < MAX_DECISION_OPTIONS,
            "a catalog of {catalog} intents does not fit the {MAX_DECISION_OPTIONS} options of a decision"
        );
    }

    #[test]
    fn every_entity_of_an_example_reaches_its_command() {
        for example in example_intents() {
            let values = values_of(&example);
            let rendered = render_command(&example.command, &entity_names(&example), &values);

            assert!(
                rendered.unreadable.is_empty(),
                "the command of {} reads no value for {:?}",
                example.name,
                rendered.unreadable
            );
            for entity in &example.entities {
                let value = values.get(&entity.name).expect("every entity has a value");
                assert!(
                    rendered.command.contains(value.as_str()),
                    "the value of {} never reaches the command of {}",
                    entity.name,
                    example.name
                );
            }
        }
    }

    #[test]
    fn every_placeholder_of_an_example_is_replaced() {
        for example in example_intents() {
            let rendered = render_command(
                &example.command,
                &entity_names(&example),
                &values_of(&example),
            );
            let leftovers = leftovers(&rendered.command);

            assert!(
                leftovers.is_empty(),
                "the command of {} names {leftovers:?}, which is no entity of it",
                example.name
            );
        }
    }

    #[test]
    fn the_command_of_the_workspace_keeps_the_braces_of_its_language() {
        let intent = normalized("switch workspace");
        let rendered = render_command(
            &intent.command,
            &entity_names(&intent),
            &BTreeMap::from([("workspace".to_string(), "3".to_string())]),
        );

        assert_eq!(
            rendered.command,
            "ws=\"3\"; env -u LD_LIBRARY_PATH hyprctl dispatch \
\"hl.dsp.focus({ workspace = $ws })\""
        );
    }

    #[test]
    fn the_example_opens_an_application() {
        let intent = normalized(APPLICATIONS_INTENT);

        assert_eq!(intent.command, APPLICATIONS_COMMAND);
        // The value is a program the shell runs, so the placeholder sits
        // in double quotes and the launcher reads it as one argument.
        assert!(intent.command.contains("\"{applications}\""));
        // The program leaves the session of the daemon, so the turn
        // answers at once instead of waiting for it to close.
        assert!(intent.command.starts_with("setsid"));
        assert_eq!(intent.entities.len(), 1);
    }

    #[test]
    fn the_example_reads_its_list_from_a_script() {
        let intent = normalized(APPLICATIONS_INTENT);
        let entity = &intent.entities[0];

        assert_eq!(entity.kind, EntityKindDto::Script);
        assert_eq!(entity.script.as_deref(), Some(APPLICATIONS_SCRIPT));
        assert!(entity.values.is_empty());
    }

    #[test]
    fn the_example_names_the_phrases_a_user_says() {
        let intent = normalized(APPLICATIONS_INTENT);

        assert!(intent
            .examples
            .iter()
            .any(|phrase| phrase == "launch firefox"));
        assert!(intent.examples.len() > 1);
    }

    #[test]
    fn the_examples_run_the_tools_of_the_session() {
        // Every command of the catalog runs a program the session offers,
        // so the catalog holds no default that asks the user for a tool
        // that another session may not have.
        let tools = [
            "curl", "date", "df", "foot", "free", "grim", "hyprctl", "hyprlock", "makoctl",
            "nmcli", "setsid", "slurp", "wl-copy", "wpctl", "xdg-open",
        ];
        let missing: Vec<String> = example_intents()
            .into_iter()
            .filter(|example| !tools.iter().any(|tool| example.command.contains(tool)))
            .map(|example| example.name)
            .collect();

        assert!(
            missing.is_empty(),
            "these defaults name no tool of the session: {missing:?}"
        );
    }

    #[test]
    fn the_script_reads_the_exec_name_of_every_desktop_entry() {
        assert!(APPLICATIONS_SCRIPT.contains("^Exec="));
        assert!(APPLICATIONS_SCRIPT.contains(".desktop"));
        // The program of the entry, not the whole command line of a
        // launcher, so the field codes go with the other words.
        assert!(APPLICATIONS_SCRIPT.contains("%[A-Za-z]"));
    }

    #[test]
    fn the_script_reads_the_name_of_the_program_not_the_path_to_it() {
        // The name of a value is the last word of the path it holds.
        assert!(APPLICATIONS_SCRIPT.contains("n=split(p, part, \"/\")"));
        assert!(APPLICATIONS_SCRIPT.contains("p=part[n]"));
        // A name the store hides behind a dot and a suffix comes back as
        // the name of the program.
        assert!(APPLICATIONS_SCRIPT.contains("if (substr(p,1,1)==\".\") p=substr(p,2)"));
        assert!(APPLICATIONS_SCRIPT.contains("substr(p,length(p)-7)==\"-wrapped\""));
    }

    #[test]
    fn the_script_skips_the_entries_the_desktop_hides() {
        assert!(APPLICATIONS_SCRIPT.contains("^(NoDisplay|Hidden)=true"));
        assert!(APPLICATIONS_SCRIPT.contains("^Type=Application"));
        // An entry that starts a shell, or that only sets the environment
        // of the program, offers no application.
        assert!(APPLICATIONS_SCRIPT.contains("sh|bash|dash|zsh|env"));
        assert!(APPLICATIONS_SCRIPT.contains("$i==\"env\""));
    }

    #[test]
    fn the_script_reads_every_directory_the_desktop_specification_names() {
        // The directories of the user, of a Nix profile, and of the
        // system, plus the ones the session names.
        assert!(APPLICATIONS_SCRIPT.contains("${XDG_DATA_HOME:-$HOME/.local/share}"));
        assert!(APPLICATIONS_SCRIPT.contains("$HOME/.nix-profile/share"));
        assert!(APPLICATIONS_SCRIPT.contains("${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"));
        // The store of a Nix profile is reached through a link, so the
        // walk follows links.
        assert!(APPLICATIONS_SCRIPT.contains("find -L"));
    }

    #[test]
    fn the_script_of_the_folders_lists_the_names_of_the_directories() {
        assert!(FOLDERS_SCRIPT.contains("\"$HOME\""));
        assert!(FOLDERS_SCRIPT.contains("-type d"));
        // A hidden directory is the configuration of a program, not a
        // folder a user asks for.
        assert!(FOLDERS_SCRIPT.contains("-not -name '.*'"));
        // A user says the name of a folder, so the list carries the name
        // and not the path, and the command of the intent builds the
        // path of the home around it.
        assert!(FOLDERS_SCRIPT.contains("sed 's|.*/||'"));
        assert!(normalized("open folder")
            .command
            .contains("foot -e yazi \"$HOME/{folder}\""));
    }
}
