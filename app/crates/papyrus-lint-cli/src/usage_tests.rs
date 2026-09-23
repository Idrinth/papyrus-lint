#[test]
fn usage_embeds_checked_in_cli_examples() {
    assert!(crate::USAGE.starts_with("Usage: PapyrusLinterCLI"));
    assert!(crate::USAGE.contains("Examples:"));
    assert!(crate::USAGE.contains("PapyrusLinterCLI lint path/to/project.achlist"));
    assert!(crate::USAGE.contains("PapyrusLinterCLI lint --blob"));
    assert!(crate::USAGE.contains("PapyrusLinterCLI doctor path/to/project.achlist"));
}

#[test]
fn usage_embeds_contact_tagged_links_from_shared_yaml() {
    assert!(crate::USAGE.contains("https://discord.gg/idrinth"));
    assert!(crate::USAGE.contains("https://tally.so/r/aQL1dB"));
    assert!(
        !crate::USAGE.contains("marketplace.visualstudio.com"),
        "CLI help should only include contact-tagged links"
    );
}
