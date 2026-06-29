use clap::Args;
use salvo::oapi::OpenApi;
use std::io::Write;

#[derive(Debug, Clone, Args)]
pub struct ExportOpenapi;

impl ExportOpenapi {
    pub fn dispatch(self) -> miette::Result<()> {
        // Create a minimal router without database-dependent middleware
        let router = crate::controllers::schema_router();

        // Generate OpenAPI spec with the same metadata as the running server
        let mut doc = OpenApi::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        doc.info.description = Some("API service".to_string());
        doc.info.license = Some(salvo::oapi::License::new("MIT"));
        doc = doc.merge_router(&router);

        // Serialize the OpenAPI document to JSON
        let spec_json = serde_json::to_string_pretty(&doc)
            .map_err(|e| miette::miette!("Failed to serialize OpenAPI spec: {}", e))?;

        // Output to stdout
        std::io::stdout()
            .write_all(spec_json.as_bytes())
            .map_err(|e| miette::miette!("Failed to write OpenAPI spec to stdout: {}", e))?;

        Ok(())
    }
}
