#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchEvent {
    None,
    PasswordPrompt,
    HostKeyUnknown,
    HostKeyChanged,
}

#[derive(Debug, Clone)]
struct SingleMatcher {
    pattern: Vec<u8>,
    state: usize,
}

impl SingleMatcher {
    /// Creates a matcher for a literal byte pattern.
    fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.as_bytes().to_vec(),
            state: 0,
        }
    }

    /// Feeds one byte through the streaming matcher.
    fn feed_byte(&mut self, byte: u8) -> bool {
        if self.pattern.is_empty() {
            return false;
        }

        if self.pattern[self.state] == byte {
            self.state += 1;
        } else {
            self.state = 0;
            if self.pattern[self.state] == byte {
                self.state += 1;
            }
        }

        if self.state == self.pattern.len() {
            self.state = 0;
            return true;
        }

        false
    }

    /// Resets in-flight partial progress.
    fn reset(&mut self) {
        self.state = 0;
    }
}

#[derive(Debug, Clone)]
pub struct PromptMatcher {
    password: SingleMatcher,
    host_key_unknown: SingleMatcher,
    host_key_changed: SingleMatcher,
    password_matches: usize,
}

impl PromptMatcher {
    const DEFAULT_PASSWORD_PATTERN: &'static str = "assword:";
    const HOST_KEY_UNKNOWN_PATTERN: &'static str = "The authenticity of host ";
    const HOST_KEY_CHANGED_PATTERN: &'static str =
        "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED";

    /// Creates a prompt matcher with the requested password prompt pattern.
    pub fn new(password_pattern: &str) -> Self {
        let password_pattern = if password_pattern.is_empty() {
            Self::DEFAULT_PASSWORD_PATTERN
        } else {
            password_pattern
        };

        Self {
            password: SingleMatcher::new(password_pattern),
            host_key_unknown: SingleMatcher::new(Self::HOST_KEY_UNKNOWN_PATTERN),
            host_key_changed: SingleMatcher::new(Self::HOST_KEY_CHANGED_PATTERN),
            password_matches: 0,
        }
    }

    /// Feeds PTY output into all matchers and reports the first detected event.
    pub fn feed(&mut self, buffer: &[u8]) -> MatchEvent {
        for &byte in buffer {
            let password_match = self.password.feed_byte(byte);
            let unknown_match = self.host_key_unknown.feed_byte(byte);
            let changed_match = self.host_key_changed.feed_byte(byte);

            if changed_match {
                return MatchEvent::HostKeyChanged;
            }

            if unknown_match {
                return MatchEvent::HostKeyUnknown;
            }

            if password_match {
                self.password_matches += 1;
                return MatchEvent::PasswordPrompt;
            }
        }

        MatchEvent::None
    }

    /// Returns how many password prompts have been detected.
    pub fn password_match_count(&self) -> usize {
        self.password_matches
    }

    /// Clears any pending password partial match state after password injection.
    pub fn reset_password(&mut self) {
        self.password.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::{MatchEvent, PromptMatcher};

    #[test]
    fn test_password_prompt_match() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"Enter password:"), MatchEvent::PasswordPrompt);
    }

    #[test]
    fn test_split_buffer() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"Pass"), MatchEvent::None);
        assert_eq!(matcher.feed(b"word:"), MatchEvent::PasswordPrompt);
    }

    #[test]
    fn test_no_match() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"username:"), MatchEvent::None);
    }

    #[test]
    fn test_partial_match_pending() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"passw"), MatchEvent::None);
    }

    #[test]
    fn test_double_match() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"Enter password:"), MatchEvent::PasswordPrompt);
        matcher.reset_password();
        assert_eq!(matcher.feed(b"Enter password:"), MatchEvent::PasswordPrompt);
        assert_eq!(matcher.password_match_count(), 2);
    }

    #[test]
    fn test_reset_password() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"Enter password:"), MatchEvent::PasswordPrompt);
        matcher.reset_password();
        assert_eq!(matcher.feed(b"Enter password:"), MatchEvent::PasswordPrompt);
    }

    #[test]
    fn test_host_key_unknown() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(
            matcher.feed(b"The authenticity of host 'example.com' can't be established."),
            MatchEvent::HostKeyUnknown,
        );
    }

    #[test]
    fn test_host_key_changed() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(
            matcher.feed(b"WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED"),
            MatchEvent::HostKeyChanged,
        );
    }

    #[test]
    fn test_custom_pattern() {
        let mut matcher = PromptMatcher::new("secret:");

        assert_eq!(matcher.feed(b"Enter secret:"), MatchEvent::PasswordPrompt);
    }

    #[test]
    fn test_empty_buffer() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(&[]), MatchEvent::None);
    }

    #[test]
    fn test_single_byte_at_a_time() {
        let mut matcher = PromptMatcher::new("assword:");
        let mut event = MatchEvent::None;

        for byte in b"Password:" {
            event = matcher.feed(&[*byte]);
        }

        assert_eq!(event, MatchEvent::PasswordPrompt);
    }

    #[test]
    fn test_simultaneous_patterns() {
        let mut matcher = PromptMatcher::new("assword:");

        assert_eq!(matcher.feed(b"The authenticity of h"), MatchEvent::None);
        assert_eq!(
            matcher.feed(b"ost 'example.com'"),
            MatchEvent::HostKeyUnknown
        );
        assert_eq!(matcher.feed(b"Password:"), MatchEvent::PasswordPrompt);
        matcher.reset_password();
        assert_eq!(
            matcher.feed(b"WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED"),
            MatchEvent::HostKeyChanged,
        );
    }
}
