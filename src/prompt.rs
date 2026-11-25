//! Rules for which pinentry prompts may be answered from the credential store.
//! Pure string logic, shared by every platform.

/// Assuan data lines must escape `%`, CR and LF.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            c => out.push(c),
        }
    }
    out
}

/// What gpg-agent told the pinentry so far in this session.
#[derive(Default)]
pub struct Prompt {
    pub desc: String,
    pub prompt: String,
    pub had_error: bool,
    pub answered: bool,
}

impl Prompt {
    /// The card serial, if this is the first try at a card's user PIN. scdaemon's description looks like
    /// `Please unlock the card%0A%0ANumber: 32 898 543%0AHolder: ...`; it adds "Remaining attempts" once a
    /// try failed, and that must never burn a second attempt with the same stored PIN.
    pub fn card_serial(&self) -> Option<String> {
        if self.answered || self.had_error {
            return None;
        }
        let (desc, prompt) = (self.desc.as_str(), self.prompt.as_str());
        if prompt.contains("Admin")
            || ["Admin", "New", "Reset", "Remaining attempts"]
                .iter()
                .any(|w| desc.contains(w))
        {
            return None;
        }
        if !desc.contains("unlock the card") {
            return None;
        }
        let (_, rest) = desc.split_once("Number: ")?;
        let number = rest.split("%0A").next().unwrap_or_default();
        let serial: String = number.chars().filter(char::is_ascii_digit).collect();
        (!serial.is_empty()).then_some(serial)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNLOCK: &str = "Please unlock the card%0A%0ANumber: 32 898 543%0AHolder: Some One%0ACounter: 7";

    fn prompt(desc: &str) -> Prompt {
        Prompt {
            desc: desc.into(),
            prompt: "PIN".into(),
            ..Prompt::default()
        }
    }

    #[test]
    fn first_user_pin_prompt_gives_serial() {
        assert_eq!(prompt(UNLOCK).card_serial().as_deref(), Some("32898543"));
    }

    #[test]
    fn never_answers_a_retry() {
        let retry = format!("{UNLOCK}%0A%0ARemaining attempts: 2");
        assert_eq!(prompt(&retry).card_serial(), None);

        let mut after_error = prompt(UNLOCK);
        after_error.had_error = true;
        assert_eq!(after_error.card_serial(), None);

        let mut second = prompt(UNLOCK);
        second.answered = true;
        assert_eq!(second.card_serial(), None);
    }

    #[test]
    fn never_answers_admin_or_new_pin() {
        let mut admin = prompt(UNLOCK);
        admin.prompt = "Admin PIN".into();
        assert_eq!(admin.card_serial(), None);
        assert_eq!(prompt(&UNLOCK.replace("Please", "New")).card_serial(), None);
        assert_eq!(prompt(&format!("Reset Code%0A{UNLOCK}")).card_serial(), None);
    }

    #[test]
    fn ignores_passphrase_prompts() {
        let key = "Please enter the passphrase to unlock the OpenPGP secret key";
        assert_eq!(prompt(key).card_serial(), None);
    }

    #[test]
    fn encodes_assuan_specials() {
        assert_eq!(encode("1%2\r\n3"), "1%252%0D%0A3");
    }
}
