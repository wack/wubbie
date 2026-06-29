use clap::ValueEnum;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Default)]
pub enum EnableColors {
    Always,
    Never,
    #[default]
    Auto,
}
