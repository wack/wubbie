//! Ergonomic HTTP test client wrapping Salvo's test client.
//!
//! `send` eagerly captures the status and full body so callers (and the BDD
//! `TestWorld`) can inspect the response without `&mut` plumbing.

#![allow(dead_code)]

use salvo::http::StatusCode;
use salvo::prelude::*;
use salvo::test::{ResponseExt, TestClient as SalvoTestClient};
use serde::Serialize;

/// A small wrapper around Salvo's test client.
#[derive(Debug)]
pub struct TestClient {
    base_url: String,
}

impl Default for TestClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TestClient {
    /// Creates a client with the default in-process base URL.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_url: "http://127.0.0.1:5800".to_string(),
        }
    }

    /// GET request builder for `path`.
    #[must_use]
    pub fn get(&self, path: &str) -> RequestBuilder {
        RequestBuilder::new(Method::Get, self.build_url(path))
    }

    /// POST request builder for `path`.
    #[must_use]
    pub fn post(&self, path: &str) -> RequestBuilder {
        RequestBuilder::new(Method::Post, self.build_url(path))
    }

    /// PUT request builder for `path`.
    #[must_use]
    pub fn put(&self, path: &str) -> RequestBuilder {
        RequestBuilder::new(Method::Put, self.build_url(path))
    }

    /// PATCH request builder for `path`.
    #[must_use]
    pub fn patch(&self, path: &str) -> RequestBuilder {
        RequestBuilder::new(Method::Patch, self.build_url(path))
    }

    /// DELETE request builder for `path`.
    #[must_use]
    pub fn delete(&self, path: &str) -> RequestBuilder {
        RequestBuilder::new(Method::Delete, self.build_url(path))
    }

    fn build_url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }
}

/// HTTP methods supported by the test client.
#[derive(Debug, Clone, Copy)]
enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// Builder for a single test request.
#[derive(Debug)]
pub struct RequestBuilder {
    method: Method,
    url: String,
    body: Option<Vec<u8>>,
    content_type: Option<String>,
}

impl RequestBuilder {
    fn new(method: Method, url: String) -> Self {
        Self {
            method,
            url,
            body: None,
            content_type: None,
        }
    }

    /// Sets a JSON request body.
    #[must_use]
    pub fn json<T: Serialize>(mut self, body: &T) -> Self {
        self.body = Some(serde_json::to_vec(body).expect("failed to serialize JSON body"));
        self.content_type = Some("application/json".to_string());
        self
    }

    /// Sends the request to `service` and captures the response.
    pub async fn send(self, service: &Service) -> TestResponse {
        let mut client = match self.method {
            Method::Get => SalvoTestClient::get(&self.url),
            Method::Post => SalvoTestClient::post(&self.url),
            Method::Put => SalvoTestClient::put(&self.url),
            Method::Patch => SalvoTestClient::patch(&self.url),
            Method::Delete => SalvoTestClient::delete(&self.url),
        };

        if let Some(content_type) = self.content_type {
            client = client.add_header(salvo::http::header::CONTENT_TYPE, content_type, true);
        }
        if let Some(body) = self.body {
            client = client.bytes(body);
        }

        let mut response = client.send(service).await;
        let status = response
            .status_code
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = response.take_bytes(None).await.unwrap_or_default().to_vec();

        TestResponse { status, body }
    }
}

/// A captured test response (status + full body).
#[derive(Debug)]
pub struct TestResponse {
    /// HTTP status code.
    pub status: StatusCode,
    /// Full response body bytes.
    pub body: Vec<u8>,
}

impl TestResponse {
    /// True if the status is 2xx.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// True if the status is 4xx.
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        self.status.is_client_error()
    }
}
