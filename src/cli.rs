use clap::{App, AppSettings, Arg};
use indoc::indoc;

pub fn new_app<'help>(name: &str, about: &'help str) -> App<'help> {
    new_subcommand(name, about)
        .global_setting(AppSettings::HelpExpected)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .global_setting(AppSettings::DisableHelpSubcommand)
        .global_setting(AppSettings::DontCollapseArgsInUsage)
}

pub fn new_subcommand<'help>(name: &str, about: &'help str) -> App<'help> {
    App::new(name)
        // Default template contains `{bin} {version}` for some reason
        .help_template(indoc!("
            {before-help}{about}

            {usage-heading}
                {usage}

            {all-args}{after-help}\
        "))
        .about(about)
}

pub fn new_arg<'help>(name: &'help str, help: &'help str) -> Arg<'help> {
    Arg::new(name).help(help)
}