/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

pub use clap::CommandFactory;
pub use clap::Parser;

static LONG_ABOUT: &str = "
wbproto-beautifier formats and beautifies Webots PROTO code.

This beautifier is quite opinionated and does not offer options.";

#[derive(Debug, Parser)]
#[command(author, version, about = LONG_ABOUT)]
pub struct Arguments {
    /// File(s) to beautify. If more than one file is passed, inline is implied. If no file is given, reads from stdin.
    #[arg(global = true)]
    pub files: Vec<String>,

    /// Whether files should be formatted inplace instead of printing to stdout.
    #[arg(global = true, long = "inplace")]
    pub inplace: bool,
}
