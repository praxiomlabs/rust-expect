//! Common dialog patterns.

use super::definition::{Dialog, DialogBuilder, DialogStep};
use std::time::Duration;

/// Create a login dialog.
#[must_use]
pub fn login_dialog(username: &str, password: &str) -> Dialog {
    DialogBuilder::named("login")
        .expect_send("username", "login:", format!("{username}\n"))
        .expect_send("password", "assword:", format!("{password}\n"))
        .build()
}

/// Create an SSH dialog with host key acceptance.
#[must_use]
pub fn ssh_dialog(host: &str, username: &str, password: &str) -> Dialog {
    Dialog::named("ssh")
        .variable("HOST", host)
        .variable("USER", username)
        .variable("PASS", password)
        .step(
            DialogStep::new("hostkey")
                .with_expect("(yes/no")
                .with_send("yes\n")
                .then("password"),
        )
        .step(
            DialogStep::new("password")
                .with_expect("assword:")
                .with_send("${PASS}\n"),
        )
}

/// Create a sudo dialog.
#[must_use]
pub fn sudo_dialog(password: &str) -> Dialog {
    DialogBuilder::named("sudo")
        .expect_send("password", "[sudo] password", format!("{password}\n"))
        .build()
}

/// Create a yes/no confirmation dialog.
#[must_use]
pub fn confirm_dialog(answer: bool) -> Dialog {
    let response = if answer { "yes\n" } else { "no\n" };
    DialogBuilder::named("confirm")
        .expect_send("confirm", "[y/n]", response)
        .build()
}

/// Create a menu selection dialog.
#[must_use]
pub fn menu_dialog(selection: &str) -> Dialog {
    DialogBuilder::named("menu")
        .expect_send("select", "choice:", format!("{selection}\n"))
        .build()
}

/// Create a FTP login dialog.
#[must_use]
pub fn ftp_dialog(username: &str, password: &str) -> Dialog {
    Dialog::named("ftp")
        .step(
            DialogStep::new("user")
                .with_expect("Name")
                .with_send(format!("{username}\n")),
        )
        .step(
            DialogStep::new("pass")
                .with_expect("Password")
                .with_send(format!("{password}\n")),
        )
}

/// Create a telnet login dialog.
#[must_use]
pub fn telnet_dialog(username: &str, password: &str) -> Dialog {
    Dialog::named("telnet")
        .step(
            DialogStep::new("login")
                .with_expect("login:")
                .with_send(format!("{username}\n")),
        )
        .step(
            DialogStep::new("password")
                .with_expect("Password:")
                .with_send(format!("{password}\n")),
        )
}

/// Create a git credential dialog.
#[must_use]
pub fn git_credential_dialog(username: &str, password: &str) -> Dialog {
    Dialog::named("git")
        .step(
            DialogStep::new("user")
                .with_expect("Username")
                .with_send(format!("{username}\n")),
        )
        .step(
            DialogStep::new("pass")
                .with_expect("Password")
                .with_send(format!("{password}\n")),
        )
}

/// Create a prompt continuation dialog.
#[must_use]
pub fn shell_prompt_dialog(prompt: &str) -> Dialog {
    Dialog::named("shell").step(
        DialogStep::new("prompt")
            .with_expect(prompt)
            .timeout(Duration::from_secs(5)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_dialog_creation() {
        let dialog = login_dialog("user", "pass");
        assert_eq!(dialog.name, "login");
        assert_eq!(dialog.steps.len(), 2);
    }

    #[test]
    fn ssh_dialog_has_variables() {
        let dialog = ssh_dialog("host", "user", "pass");
        assert_eq!(dialog.variables.get("HOST"), Some(&"host".to_string()));
    }
}
