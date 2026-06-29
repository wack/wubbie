use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct Version;

impl Version {
    pub fn dispatch(self) -> miette::Result<()> {
        println!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        Ok(())
    }
}
