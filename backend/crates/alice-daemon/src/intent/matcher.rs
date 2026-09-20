//! Read the value of a list that a message names.
//!
//! A script entity offers a live list: the applications of the machine,
//! the windows of a session, the devices of a house. The list is longer
//! than a model can read as labels, so the model reads the words of the
//! value and this module reads the entry of the list those words name.
//!
//! The rules run from the surest to the loosest: the same spelling, the
//! same words, one spelling a prefix of the other, one inside the other,
//! every word of the entry in the message, and finally a small distance
//! for a value the message almost spells. A mention that matches nothing
//! returns nothing, because a command that runs an application nobody
//! asked for is worse than a turn the daemon refuses.
//!
//! A value is what the machine really runs, and the script of an entity
//! decides the spelling: the example script of the applications offers
//! the name of the program (`firefox`), and a script of the user may
//! offer a path (`/nix/store/...-firefox-116.0.3/bin/firefox`). The rules
//! therefore also read the base name of a value, so the mention of the
//! user reaches the entry it names either way.

/// The smallest length of a value that a loose rule may match.
///
/// A two letter name sits inside half the words of a message, so a short
/// value matches only when the message spells it out.
const MIN_LOOSE_LEN: usize = 3;

/// The largest distance a misspelled value may have.
const MAX_DISTANCE: usize = 2;

/// How one mention matched one value of a list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// The message spells the value, without case.
    Exact,
    /// The message spells the value, without case or punctuation.
    Normalized,
    /// One spelling starts with the other.
    Prefix,
    /// One spelling holds the other as a whole.
    Contains,
    /// Every word of the value stands in the message.
    Words,
    /// The message almost spells the value.
    Near,
}

impl MatchKind {
    /// How sure the rule is, best first. A smaller number is surer.
    fn rank(&self) -> usize {
        match self {
            Self::Exact => 0,
            Self::Normalized => 1,
            Self::Prefix => 2,
            Self::Contains => 3,
            Self::Words => 4,
            Self::Near => 5,
        }
    }

    /// The name of the rule, for a report.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Normalized => "normalized",
            Self::Prefix => "prefix",
            Self::Contains => "contains",
            Self::Words => "words",
            Self::Near => "near",
        }
    }
}

/// One value of a list and the rule that read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The value as the list spells it.
    pub value: String,
    /// The rule that read it.
    pub kind: MatchKind,
}

/// Put a text into the form the loose rules compare.
fn normalized(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The number of characters two texts share at their start.
fn shared_prefix(left: &str, right: &str) -> usize {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .count()
}

/// The distance of two texts, counted in single character steps.
///
/// Two characters the writer swapped count as one step, because a swap is
/// the typo a keyboard makes most: `discrod` is one step from `discord`.
fn distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.is_empty() || right.is_empty() {
        return left.len().max(right.len());
    }

    let mut two_back: Vec<usize> = vec![0; right.len() + 1];
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current: Vec<usize> = vec![0; right.len() + 1];
    for row in 1..=left.len() {
        current[0] = row;
        for column in 1..=right.len() {
            let step = if left[row - 1] == right[column - 1] {
                0
            } else {
                1
            };
            let mut best = (previous[column] + 1)
                .min(current[column - 1] + 1)
                .min(previous[column - 1] + step);
            let swapped = row > 1
                && column > 1
                && left[row - 1] == right[column - 2]
                && left[row - 2] == right[column - 1];
            if swapped {
                best = best.min(two_back[column - 2] + 1);
            }
            current[column] = best;
        }
        std::mem::swap(&mut two_back, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

/// Whether the message holds every word of one value, in any order.
fn holds_words(mention: &[String], value: &[String]) -> bool {
    value.iter().all(|word| mention.contains(word))
}

/// The names inside one value that name it on their own.
///
/// The base name of a path is the name of the program it runs, and the
/// words the base name is written with are the name a user says when the
/// base name joins them with a dash: `blueman-manager` is named by
/// `blueman manager`.
fn candidate_names(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let base = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let mut names: Vec<String> = Vec::new();
    if !base.is_empty() && !base.eq_ignore_ascii_case(trimmed) {
        names.push(base.to_string());
    }
    if base.contains(['-', '_']) {
        names.push(base.replace(['-', '_'], " "));
    }
    names
}

/// The rule that reads one value out of one mention, or none.
///
/// The value itself is read first, then the names inside it. A rule that
/// reads the whole value wins over the same rule on a name of it, so a
/// list that holds both `firefox` and a path of one reads the plain one.
fn match_value(mention: &str, value: &str) -> Option<MatchKind> {
    let own = match_one(mention, value);
    let mut best = own;
    for name in candidate_names(value) {
        let Some(kind) = match_one(mention, &name) else {
            continue;
        };
        if best.is_none_or(|best| kind.rank() < best.rank()) {
            best = Some(kind);
        }
    }
    best
}

/// The rule that reads one value out of one mention, or none.
fn match_one(mention: &str, value: &str) -> Option<MatchKind> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if value.trim().eq_ignore_ascii_case(mention.trim()) {
        return Some(MatchKind::Exact);
    }

    let mention_normalized = normalized(mention);
    let value_normalized = normalized(trimmed);
    if mention_normalized.is_empty() || value_normalized.is_empty() {
        return None;
    }
    if mention_normalized == value_normalized {
        return Some(MatchKind::Normalized);
    }

    let mention_words = words_of(mention);
    let value_words = words_of(trimmed);

    if value_normalized.len() >= MIN_LOOSE_LEN && mention_normalized.len() >= MIN_LOOSE_LEN {
        if value_normalized.starts_with(&mention_normalized) {
            return Some(MatchKind::Prefix);
        }
        if mention_normalized.contains(&value_normalized) {
            return Some(MatchKind::Contains);
        }
    }
    if value_words.len() > 1 {
        if holds_words(&mention_words, &value_words) {
            return Some(MatchKind::Words);
        }
    } else if value_normalized.len() >= MIN_LOOSE_LEN {
        // A one word value that stands in the message as a word of its
        // own is a hit, whether the message adds words or not.
        if mention_words.contains(&value_normalized) {
            return Some(MatchKind::Words);
        }
    }

    let allowed = (value_normalized.len() / 4).clamp(1, MAX_DISTANCE);
    if value_normalized.len() >= MIN_LOOSE_LEN
        && distance(&mention_normalized, &value_normalized) <= allowed
    {
        return Some(MatchKind::Near);
    }
    None
}

/// Read the value of a list that best fits one mention, or none.
///
/// The surest rule wins. Two values that the same rule reads are told
/// apart by the text they share with the mention, and then by the shorter
/// one, because `firefox` names the program and `firefox-esr` names one
/// build of it.
pub fn best_match(mention: &str, values: &[String]) -> Option<Found> {
    best_of(&[mention.to_string()], values)
}

/// Read the value of a list that best fits one of many mentions, or none.
///
/// The daemon reads the words of a message against a list when the engine
/// read no span for it: the user said the name of one entry, and the name
/// is all the message had to say about it.
pub fn best_mention(mentions: &[String], values: &[String]) -> Option<Found> {
    best_of(mentions, values)
}

/// The windows of one message that may name one value of a list.
///
/// The windows are one to three words long, because a name is that long.
/// A window that holds a word of `skip` is left out: those are the words
/// of the intent itself, and the value never repeats the action.
pub fn mentions_of(message: &str, skip: &[String]) -> Vec<String> {
    let words = words_of(message);
    let mut mentions: Vec<String> = Vec::new();
    for start in 0..words.len() {
        for end in start + 1..=(start + 3).min(words.len()) {
            let window = &words[start..end];
            if window.iter().any(|word| skip.contains(word)) {
                continue;
            }
            mentions.push(window.join(" "));
        }
    }
    mentions
}

/// Split a text into the lowercase words it holds.
pub fn words_of(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// The best value one of many mentions reads, or none.
///
/// Rule, then the text the value shares with the mention, then the
/// length: the surest rule wins, a longer shared text wins, and the
/// plainer name wins over a longer one.
fn best_of(mentions: &[String], values: &[String]) -> Option<Found> {
    let mut best: Option<(MatchKind, usize, usize, String)> = None;

    for mention in mentions {
        for value in values {
            let Some(kind) = match_value(mention, value) else {
                continue;
            };
            let shared = shared_prefix(&normalized(mention), &normalized(value));
            let length = normalized(value).chars().count();

            let better = match &best {
                None => true,
                Some((best_kind, best_shared, best_length, _)) => {
                    (kind.rank(), std::cmp::Reverse(shared), length)
                        < (
                            best_kind.rank(),
                            std::cmp::Reverse(*best_shared),
                            *best_length,
                        )
                }
            };
            if better {
                best = Some((kind, shared, length, value.clone()));
            }
        }
    }

    best.map(|(kind, _, _, value)| Found { value, kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The list the tests read from.
    fn applications() -> Vec<String> {
        vec![
            "firefox".to_string(),
            "obs".to_string(),
            "discord".to_string(),
            "google-chrome".to_string(),
            "code".to_string(),
            "code-oss".to_string(),
            "gnome-terminal".to_string(),
        ]
    }

    /// Read one mention out of the list of the tests.
    fn read(mention: &str) -> Option<Found> {
        best_match(mention, &applications())
    }

    #[test]
    fn a_spelled_out_value_reads_exactly() {
        let found = read("obs").expect("the list holds obs");
        assert_eq!(found.value, "obs");
        assert_eq!(found.kind, MatchKind::Exact);
    }

    #[test]
    fn the_case_of_the_message_does_not_matter() {
        let found = read("Firefox").expect("the list holds firefox");
        assert_eq!(found.value, "firefox");
        assert_eq!(found.kind, MatchKind::Exact);
    }

    #[test]
    fn the_punctuation_of_the_message_does_not_matter() {
        let values = vec!["google-chrome".to_string()];
        let spaced = best_match("google chrome", &values).expect("the list holds google-chrome");
        let dashed = best_match("google-chrome", &values).expect("the list holds google-chrome");

        assert_eq!(spaced.value, "google-chrome");
        assert_eq!(dashed.value, "google-chrome");
        // The name written with a dash reads as surely as the value does.
        assert_eq!(spaced.kind, dashed.kind);
    }

    #[test]
    fn a_shortened_name_reads_the_value() {
        let found = read("fire").expect("fire names firefox");
        assert_eq!(found.value, "firefox");
        assert_eq!(found.kind, MatchKind::Prefix);
    }

    #[test]
    fn a_name_inside_a_sentence_reads_the_value() {
        let found = read("google chrome").expect("the words name google-chrome");
        assert_eq!(found.value, "google-chrome");
    }

    #[test]
    fn a_misspelled_name_reads_the_value() {
        let found = read("discrod").expect("discrod is a step from discord");
        assert_eq!(found.value, "discord");
        assert_eq!(found.kind, MatchKind::Near);
    }

    #[test]
    fn a_mention_that_names_nothing_reads_nothing() {
        assert!(read("vscodium").is_none());
        assert!(read("").is_none());
    }

    #[test]
    fn a_short_value_only_reads_when_the_message_spells_it() {
        let values = vec!["go".to_string(), "gh".to_string()];
        assert_eq!(
            best_match("go", &values).map(|found| found.kind),
            Some(MatchKind::Exact)
        );
        assert!(best_match("going home", &values).is_none());
    }

    #[test]
    fn the_plainer_name_wins_over_the_build_of_it() {
        let found = read("code").expect("the list holds code");
        assert_eq!(found.value, "code");

        let values = vec!["code-oss".to_string(), "code".to_string()];
        let found = best_match("code", &values).expect("the list holds code");
        assert_eq!(found.value, "code");
        assert_eq!(found.kind, MatchKind::Exact);
    }

    #[test]
    fn the_surest_rule_wins_over_a_looser_one() {
        let values = vec!["terminal".to_string(), "gnome-terminal".to_string()];
        let found = best_match("gnome-terminal", &values).expect("the list holds both");
        assert_eq!(found.value, "gnome-terminal");

        // The prefix rule reads `terminal` out of the message as well, so
        // the surest rule has to win: the message spells one entry out.
        assert_eq!(found.kind, MatchKind::Exact);
    }

    #[test]
    fn a_value_of_many_words_reads_from_a_message_that_holds_them() {
        let values = vec!["visual studio code".to_string()];
        let found =
            best_match("open code visual studio", &values).expect("the words are all there");
        assert_eq!(found.kind, MatchKind::Words);
    }

    #[test]
    fn an_empty_value_never_matches() {
        let values = vec!["".to_string(), "   ".to_string()];
        assert!(best_match("firefox", &values).is_none());
    }

    #[test]
    fn the_rule_of_a_match_is_named() {
        assert_eq!(MatchKind::Near.as_str(), "near");
        assert!(MatchKind::Exact.rank() < MatchKind::Near.rank());
    }

    #[test]
    fn distance_counts_single_character_steps() {
        assert_eq!(distance("firefox", "firefox"), 0);
        assert_eq!(distance("firefox", "firefpx"), 1);
        assert_eq!(distance("", "abc"), 3);
    }

    #[test]
    fn a_path_is_named_by_its_base_name() {
        let values = vec![
            "/nix/store/ggm28k0vkw1jg801hwvchi0vfizj0c0c-firefox-116.0.3/bin/firefox".to_string(),
            "/nix/store/9307jcya74dc4a88l2n92pyxg5vjvkmm-xdg-desktop-portal-gtk/libexec/xdg-desktop-portal-gtk".to_string(),
        ];
        let found = best_match("firefox", &values).expect("the base name names the entry");

        assert_eq!(found.value, values[0]);
        assert_eq!(found.kind, MatchKind::Exact);
    }

    #[test]
    fn a_name_written_with_a_dash_reads_the_value_it_names() {
        let values = vec!["/usr/bin/blueman-manager".to_string()];
        let found = best_match("blueman manager", &values).expect("the words name the entry");

        assert_eq!(found.value, values[0]);
        assert!(matches!(
            found.kind,
            MatchKind::Exact | MatchKind::Normalized
        ));
    }

    #[test]
    fn the_whole_value_wins_over_a_name_inside_it() {
        let values = vec![
            "firefox".to_string(),
            "/nix/store/x-firefox-116/bin/firefox".to_string(),
        ];
        let found = best_match("firefox", &values).expect("the list holds both");

        assert_eq!(found.value, "firefox");
    }

    #[test]
    fn a_word_of_a_value_alone_reads_nothing() {
        let values = vec!["blueman-manager".to_string()];

        assert!(best_match("manager", &values).is_none());
    }

    #[test]
    fn the_words_of_a_message_name_a_value() {
        let values = vec!["firefox".to_string(), "obs".to_string()];
        let mentions = mentions_of("open firefox", &["open".to_string()]);
        let found = best_mention(&mentions, &values).expect("the message names firefox");

        assert_eq!(found.value, "firefox");
    }

    #[test]
    fn the_windows_of_a_message_leave_out_its_long_tail() {
        let mentions = mentions_of("open firefox please now", &[]);

        assert!(mentions.contains(&"firefox please".to_string()));
        assert!(mentions.contains(&"please now".to_string()));
        assert!(!mentions.contains(&"open firefox please now".to_string()));
    }

    #[test]
    fn a_window_of_an_intent_word_is_left_out() {
        let mentions = mentions_of("open firefox", &["open".to_string()]);

        assert_eq!(mentions, vec!["firefox"]);
    }

    #[test]
    fn distance_counts_a_swapped_pair_as_one_step() {
        assert_eq!(distance("discrod", "discord"), 1);
        assert_eq!(distance("obs", "osb"), 1);
    }
}
