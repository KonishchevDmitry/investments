use clap::{Command, AppSettings, Arg};
use indoc::indoc;

pub fn new_app<'help>(name: &str, about: &'help str) -> Command<'help> {
    new_subcommand(name, about)
        .help_expected(true)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .disable_help_subcommand(true)
        .dont_collapse_args_in_usage(true)
}

pub fn new_subcommand<'help>(name: &str, about: &'help str) -> Command<'help> {
    Command::new(name)
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