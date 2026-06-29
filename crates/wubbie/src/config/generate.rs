//! Arguments for `wubbie generate`.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::Args;

/// The positional-prompt sentinel that means "read the prompt from stdin".
const STDIN_SENTINEL: &str = "-";

/// `wubbie generate`: generate text from a trained model.
///
/// Scaffold arguments — sampling is implemented in a later ticket (T3), which
/// inherits this layout. `--weights` points at a safetensors checkpoint; the
/// prompt is a required positional (pass `-` to read it from stdin).
#[derive(Debug, Args, Clone)]
pub struct GenerateSubcommand {
    /// The prompt to condition generation on. Pass `-` to read it from stdin.
    #[arg(value_name = "PROMPT")]
    prompt: String,

    /// Path to the safetensors weights to load.
    #[arg(long, value_name = "FILE")]
    weights: Option<PathBuf>,
}

impl GenerateSubcommand {
    /// The raw prompt argument as supplied on the command line.
    ///
    /// A value of `-` denotes stdin; use [`GenerateSubcommand::read_prompt`] to
    /// resolve it to the actual text.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Path to the weights file, if one was supplied.
    pub fn weights(&self) -> Option<&Path> {
        self.weights.as_deref()
    }

    /// Resolve the prompt to its text, reading all of stdin when the argument
    /// is the `-` sentinel.
    pub fn read_prompt(&self) -> io::Result<String> {
        if self.prompt == STDIN_SENTINEL {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        } else {
            Ok(self.prompt.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    /// `Args` structs can't be parsed standalone; flatten into a tiny `Parser`.
    #[derive(Debug, Parser)]
    struct Harness {
        #[command(flatten)]
        args: GenerateSubcommand,
    }

    fn parse(argv: &[&str]) -> Result<GenerateSubcommand, clap::Error> {
        Harness::try_parse_from(argv).map(|h| h.args)
    }

    #[test]
    fn prompt_is_required() {
        assert!(parse(&["generate"]).is_err());
    }

    #[test]
    fn literal_prompt_resolves_to_itself() {
        let args = parse(&["generate", "hello there"]).expect("parses");
        assert_eq!(args.prompt(), "hello there");
        assert_eq!(args.read_prompt().expect("read"), "hello there");
    }

    #[test]
    fn dash_is_recognized_as_the_stdin_sentinel() {
        let args = parse(&["generate", "-"]).expect("parses");
        assert_eq!(args.prompt(), STDIN_SENTINEL);
    }
}
