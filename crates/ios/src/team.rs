//! The Apple developer team that signs the runner for a physical device.

use std::io::Write;
use std::process::{Command, Stdio};

use phonedriver_core::error::{Error, UserAction};

/// The SHA-1 of each valid "Apple Development" identity in the output of `security find-identity -v -p codesigning`.
fn identities(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(") ")?.1.split_once(' ')?;
            name.starts_with("\"Apple Development: ").then_some(hash)
        })
        .collect()
}

/// Why macOS will not sign with an "Apple Development" identity this Mac holds. `security find-identity -p
/// codesigning` lists every identity before the valid ones, and writes the reason after the name of each one it
/// refuses: `1) <sha1> "Apple Development: A Person (ABCDE12345)" (CSSMERR_TP_NOT_TRUSTED)`. Issue #69: with Apple's
/// intermediate certificate missing from the keychain, a certificate that was there counted as none.
fn refused(listing: &str) -> Option<&str> {
    listing.lines().find_map(|line| {
        let (_, name) = line.trim().split_once(") ")?.1.split_once(' ')?;
        let (_, reason) = name
            .strip_prefix("\"Apple Development: ")?
            .rsplit_once("\" (")?;
        reason.strip_suffix(')')
    })
}

/// Each certificate in the output of `security find-certificate -a -Z -p`, as its SHA-1 and its PEM text.
fn certificates(dump: &str) -> Vec<(&str, String)> {
    let mut found = Vec::new();
    let mut hash = None;
    let mut pem: Option<String> = None;
    for line in dump.lines() {
        if let Some(h) = line.strip_prefix("SHA-1 hash: ") {
            hash = Some(h.trim());
        } else if line.starts_with("-----BEGIN CERTIFICATE") {
            pem = Some(format!("{line}\n"));
        } else if let Some(text) = pem.as_mut() {
            text.push_str(line);
            text.push('\n');
            if line.starts_with("-----END CERTIFICATE")
                && let (Some(h), Some(text)) = (hash.take(), pem.take())
            {
                found.push((h, text));
            }
        }
    }
    found
}

/// The team of a certificate subject in RFC 2253 form. The identity's name ends in parentheses that hold the
/// developer's own ID, which is not the team: the team is the organizational unit.
pub fn team_in(subject: &str) -> Option<&str> {
    subject
        .trim()
        .trim_start_matches("subject=")
        .split(',')
        .find_map(|part| part.trim().strip_prefix("OU="))
}

fn subject_of(pem: &str) -> Option<String> {
    let mut openssl = Command::new("/usr/bin/openssl")
        .args(["x509", "-noout", "-subject", "-nameopt", "RFC2253"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    openssl.stdin.take()?.write_all(pem.as_bytes()).ok()?;
    let out = openssl.wait_with_output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The teams this Mac holds a valid Apple Development certificate for.
pub fn certificate_teams() -> Vec<String> {
    let listing = output(
        "/usr/bin/security",
        &["find-identity", "-v", "-p", "codesigning"],
    );
    let valid = identities(&listing);
    if valid.is_empty() {
        return Vec::new();
    }
    let dump = output(
        "/usr/bin/security",
        &[
            "find-certificate",
            "-a",
            "-c",
            "Apple Development",
            "-Z",
            "-p",
        ],
    );
    let mut teams: Vec<String> = certificates(&dump)
        .into_iter()
        .filter(|(hash, _)| valid.contains(hash))
        .filter_map(|(_, pem)| subject_of(&pem))
        .filter_map(|subject| team_in(&subject).map(str::to_string))
        .collect();
    teams.sort();
    teams.dedup();
    teams
}

/// The one team among `teams`, or what the user must do when there is none or more than one.
pub fn choose(mut teams: Vec<String>) -> Result<String, Error> {
    match teams.len() {
        0 => Err(Error::NeedsUser {
            action: UserAction::SetSigningTeam,
        }),
        1 => Ok(teams.remove(0)),
        _ => Err(Error::NeedsUser {
            action: UserAction::ChooseSigningTeam { teams },
        }),
    }
}

/// The team that signs the runner: `PHONEDRIVER_TEAM`, else the team of this Mac's Apple Development certificate.
pub fn signing_team() -> Result<String, Error> {
    match std::env::var("PHONEDRIVER_TEAM")
        .ok()
        .filter(|t| !t.is_empty())
    {
        Some(team) => Ok(team),
        None => {
            let teams = certificate_teams();
            if teams.is_empty()
                && let Some(reason) = refused(&output(
                    "/usr/bin/security",
                    &["find-identity", "-p", "codesigning"],
                ))
            {
                return Err(Error::NeedsUser {
                    action: UserAction::FixSigningCertificate {
                        reason: reason.to_string(),
                    },
                });
            }
            choose(teams)
        }
    }
}

/// The bundle id a device build of the runner is signed with: `asked`, else one that holds `team`. Apple gives an
/// app id to one team, and a free account signs only ids registered to it, so an id that is the same for everyone
/// works for one team (issue #69, a free account on Xcode 27.2: "The app identifier
/// "dev.phonedriver.runner.xctrunner" cannot be registered to your development team because it is not available").
pub fn bundle_id_for(team: &str, asked: Option<&str>) -> String {
    match asked.filter(|id| !id.is_empty()) {
        Some(id) => id.to_string(),
        None => format!("dev.phonedriver.runner.{team}"),
    }
}

/// `bundle_id_for` with `PHONEDRIVER_BUNDLE_ID` as what was asked.
pub fn bundle_id(team: &str) -> String {
    bundle_id_for(team, std::env::var("PHONEDRIVER_BUNDLE_ID").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_valid_apple_development_identities_count() {
        let listing = "  1) AAAA1111 \"Apple Development: A Person (ABCDE12345)\"\n  2) BBBB2222 \"Developer ID Application: A Company (TEAM123456)\"\n  3) CCCC3333 \"Apple Distribution: A Company (TEAM123456)\"\n     3 valid identities found\n";
        assert_eq!(identities(listing), vec!["AAAA1111"]);
        assert!(identities("     0 valid identities found\n").is_empty());
    }

    #[test]
    fn an_apple_development_identity_macos_refuses_gives_its_reason_and_a_valid_one_gives_none() {
        let untrusted = "\nPolicy: Code Signing\n  Matching identities\n  1) BBBB2222 \"Developer ID Application: A Company (TEAM123456)\" (CSSMERR_TP_CERT_EXPIRED)\n  2) AAAA1111 \"Apple Development: A Person (ABCDE12345)\" (CSSMERR_TP_NOT_TRUSTED)\n     2 identities found\n\n  Valid identities only\n     0 valid identities found\n";
        assert_eq!(refused(untrusted), Some("CSSMERR_TP_NOT_TRUSTED"));
        let valid = "\nPolicy: Code Signing\n  Matching identities\n  1) AAAA1111 \"Apple Development: A Person (ABCDE12345)\"\n     1 identities found\n\n  Valid identities only\n  1) AAAA1111 \"Apple Development: A Person (ABCDE12345)\"\n     1 valid identities found\n";
        assert_eq!(refused(valid), None);
        assert_eq!(refused("     0 identities found\n"), None);
    }

    #[test]
    fn each_certificate_keeps_its_hash_and_its_pem() {
        let dump = "SHA-256 hash: FF\nSHA-1 hash: AAAA1111\n-----BEGIN CERTIFICATE-----\nMIIB\nxyz=\n-----END CERTIFICATE-----\nSHA-256 hash: EE\nSHA-1 hash: BBBB2222\n-----BEGIN CERTIFICATE-----\nMIIC\n-----END CERTIFICATE-----\n";
        let found = certificates(dump);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "AAAA1111");
        assert_eq!(
            found[0].1,
            "-----BEGIN CERTIFICATE-----\nMIIB\nxyz=\n-----END CERTIFICATE-----\n"
        );
        assert_eq!(found[1].0, "BBBB2222");
    }

    #[test]
    fn the_team_is_the_organizational_unit_not_the_id_in_the_name() {
        let subject = "subject=C=US,O=A Company\\, Inc.,OU=TEAM123456,CN=Apple Development: A Person (ABCDE12345),UID=ABCDE12345\n";
        assert_eq!(team_in(subject), Some("TEAM123456"));
        assert_eq!(team_in("subject=CN=No Team"), None);
    }

    #[test]
    fn the_runner_id_for_a_device_holds_the_team_unless_another_is_asked_for() {
        assert_eq!(
            bundle_id_for("ABCDE12345", None),
            "dev.phonedriver.runner.ABCDE12345"
        );
        assert_ne!(
            bundle_id_for("ABCDE12345", None),
            bundle_id_for("TEAM123456", None),
            "two teams never ask Apple for the same id"
        );
        assert_eq!(
            bundle_id_for("ABCDE12345", Some("com.example.phonedriver")),
            "com.example.phonedriver"
        );
        assert_eq!(
            bundle_id_for("ABCDE12345", Some("")),
            "dev.phonedriver.runner.ABCDE12345",
            "a variable set to nothing asks for nothing"
        );
    }

    #[test]
    fn one_certificate_team_is_used_and_several_are_listed() {
        let teams = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(choose(teams(&["A"])).unwrap(), "A");
        assert!(matches!(
            choose(Vec::new()),
            Err(Error::NeedsUser {
                action: UserAction::SetSigningTeam
            })
        ));
        match choose(teams(&["A", "B"])) {
            Err(Error::NeedsUser {
                action: UserAction::ChooseSigningTeam { teams },
            }) => assert_eq!(teams, vec!["A", "B"]),
            other => panic!("{other:?}"),
        }
    }
}
